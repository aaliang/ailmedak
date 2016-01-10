use std::net::{UdpSocket, SocketAddr};
use time::get_time;
use std::thread;
use std::thread::JoinHandle;
use std::sync::mpsc::{Sender, Receiver, channel};

use message_protocol::{DSocket, Message, Key, Value, ProtoMessage, NodeContact};
use api_layer::{spawn_api_thread, ClientMessage, Callback};
use utils::fmt::{as_hex_string};
use utils::networking::{ip_port_pair};
use utils::loggerator::Loggerator;
use config::Config;
use node::state::{NodeAddr, KademliaNode, ASizedNode};

const DEFAULT_TTL:i64 = 3; //timeout in seconds for a request

#[derive(Debug)]
pub struct EvictionCandidate {
    pub old: (NodeAddr, SocketAddr),
    pub new: (NodeAddr, SocketAddr)
}

#[derive(Debug)]
pub enum AsyncAction {
    Awake,
    SetEvictTimeout(EvictionCandidate),
    // the key producing these results, contact information for these results, and the nodeid from
    // the source (none if the source is the resident node)
    LookupResults(Key, Vec<NodeContact<Key>>, Option<Key>)
    //PingResp(),

}

pub enum MessageType {
    FromClient(ClientMessage),
    FromNode(Message<Key, Value>, NodeAddr, SocketAddr)
}


trait ReceiveMessage <M, A> {
    fn receive (&mut self, msg: M, src_addr: SocketAddr, a_sender: &Sender<A>, node_id: NodeAddr);
}

impl ReceiveMessage <Message<Key, Value>, AsyncAction> for KademliaNode {
    fn receive (&mut self, msg: Message<Key, Value>, src_addr: SocketAddr, a_sender: &Sender<AsyncAction>, node_id: NodeAddr) {
        match msg {
            Message::Ping => {
                let _ = self.socket.send_to(&self.ping_ack(), src_addr);
            },
            Message::FindNode(key) => {
                let kclosest = self.find_k_closest(&key);
                let response = self.find_node_resp(&kclosest, &key);
                let _ = self.socket.send_to(&response, src_addr);
            },
            Message::FindVal(key) => {
                let _ = self.socket.send_to(&(match self.data.get(&key) {
                    None => self.find_node_resp(&self.find_k_closest(&key), &key),
                    Some(data) => self.find_val_resp(&key, data)
                }), src_addr);
            },
            Message::Store(key, val) => {self.data.insert(key, val);},
            //Responses
            Message::FindNodeResp(key, node_vec) => {
                let _ = a_sender.send(AsyncAction::LookupResults(key, node_vec, Some(node_id)));
            },
            Message::PingResp => {
                //TODO: if this is an eviction candidate, do stuff such that it isn't evicted
            }
            _ => {}
        }
    }
}

pub struct AilmedakMachine;

pub trait Machine {
    fn start (&mut self, port: u16);
}

///Returns true if a lookup can be considered finished
///TODO: need to also compensate for Yellow nodes (timedout ones)
macro_rules! is_lookup_finished {
    ($k_val: expr, $cand_vec: expr) => {{
        let visited = $cand_vec.iter().take_while(|&&(_, ref color)| *color == Color::Black).count();
        if visited >= $k_val || visited >= $cand_vec.len() {
            println!("lookup is done");
            true
        } else {
            false
        }
    }}
}

///Higher level abstractions on a node. Contain worker threads that process certain types of
///messages (i.e. internal messages, messages from other nodes within the system, and messages from
///clients that wish to consume the get/set api)
impl AilmedakMachine {

    /// Spins up distinct threads (reader, state, alpha) in a CSP style channel passing model
    ///
    /// currently messages can flow either:
    ///     reader -> state -> alpha (when receiving messages from other nodes)
    ///     alpha -> state (when timing out contact information)
    ///
    /// reader spins around a UDP socket which receives datagrams sent over UDP from the outside
    /// world. Bytes are deserialized into Ailmedak Messages and passed on to the state thread
    ///
    /// state does quick processing. it will update and maintian lists sorted by how
    /// recently they were last seen. k, v lookups - the actual hash table is contained in here
    /// currently it is backed by the native Rust HashMap implementation but this is prone to
    /// change (and easily swapped out)
    ///
    /// alpha is concerned with asynchronous processing. state passes async response messages down to
    /// alpha, crucial for maintaining async state in operations such as lookup node. Additionally
    /// it also worries about timing out contact information (and updating the state lists) back up
    /// in the state thread
    pub fn start (config: Config, id_opt: Option<NodeAddr>) {
        let network_socket = match UdpSocket::bind(("0.0.0.0", config.network_port)) {
            Ok(a) => a,
            _ => panic!("unable to bind")
        };

        let state = KademliaNode::new(
            id_opt.unwrap_or_else(||KademliaNode::gen_new_id()),
            config.k_val.clone(),
            network_socket.try_clone().unwrap());

        for i_neighbor in config.initial_neighbors.iter() {
            let as_ref:&str = i_neighbor.as_ref();
            state.send_msg(&state.ping_msg(), as_ref);
        }

        let logger = Loggerator::new(state.id());
        logger.log(&format!("NODE BIND CLUSTER PORT {}", config.network_port));

        let ap = AlphaProcessor {id: state.id().clone(), k_val: state.k_val.clone()};

        let (m_tx, m_rx) = channel();
        let (a_tx, a_rx) = channel();

        let _ = Self::spawn_proto_thread(network_socket.try_clone().unwrap(), m_tx.clone());
        let cb_tx = match config {
            Config {api_port: Some(port_val), ..} => {
                let (_, s) = spawn_api_thread(port_val, m_tx.clone());
                s //this is a channel that might do something with a callback
            },
            _ => {
                let (c, _) = channel();
                c //this is a channel that does nothing
            }
        };

        let _ = Self::spawn_state_thread(state, m_rx, cb_tx, a_tx.clone());

        //alpha processor processes events that may be waiting on a future condition. performance
        //requirements are less stringent within this thread
        let _ = Self::spawn_alpha_thread(ap, a_rx, a_tx.clone(), network_socket.try_clone().unwrap());

        loop {
            thread::sleep_ms(config.async_poll_interval);
            let _ = a_tx.send(AsyncAction::Awake);
        }
    }

    /// state thread manages the k-lists staying mostly true to Kademlia's description
    fn spawn_state_thread (mut state: KademliaNode,  rx: Receiver<MessageType>,  to_api: Sender<Callback>, to_async: Sender<AsyncAction>) -> JoinHandle<()> {

        thread::spawn(move|| {
            let logger = Loggerator::new(state.id());
            loop {
                match rx.recv().unwrap() {
                    MessageType::FromClient(message) => {
                        match message {
                            ClientMessage::Get(key) => {
                                match state.data.get(&key) {
                                    None => {
                                        println!("need to node_lookup");
                                    },
                                    Some(data) => {
                                        println!("f");
                                        //if this node has an client api side, it will send the resolved key back to the api layer.
                                        //otherwise it will send a message down the channel that will just be discarded
                                        let _ = to_api.send(Callback::Resolve(key, data.clone()));
                                    }
                                }
                            },
                            ClientMessage::Set(key, val) => {
                                //TODO: remove me and use nodelookup
                                state.data.insert(key, val);
                            }
                        };
                    }
                    MessageType::FromNode(message, node_id, ip_addr) => {
                        let diff = state.distance_to(&node_id);
                        let k_index = KademliaNode::k_bucket_index(&diff);
                        let e_cand = state.update_k_bucket(k_index, (node_id, ip_addr));
                        match e_cand {
                            None => (),
                            Some(e_c) => {
                                let _ = (&to_async).send(AsyncAction::SetEvictTimeout(e_c));
                                state.send_msg(&state.ping_msg(), ip_addr);
                            }
                        };
                        
                        logger.logs(&message, &node_id);

                        let action = state.receive(message, ip_addr, &to_async, node_id);
                        println!("action: {:?}", action);
                    }

                }

            }
        })
    }

    /// alpha thread attempts to asynchronous responses from other nodes and timeouts
    // TODO: make alpha_sock a Sender
    fn spawn_alpha_thread (ap: AlphaProcessor, a_rx: Receiver<AsyncAction>, a_tx_self: Sender<AsyncAction>, alpha_sock: UdpSocket) -> JoinHandle<()> {
        const ALPHA_FACTOR:usize = 4;
        thread::spawn(move|| {
            let mut timeoutbuf:Vec<(EvictionCandidate, i64)> = Vec::new();
            //It would probably be better to use a HashMap for highly concurrent api requests
            //but for now focus on lower latency in small batches. maybe make this configurable
            let mut lookup_qi: Vec<(Key, Vec<(NodeContact<Key>, Color)>)> = Vec::new();
            let mut find_out:Vec<(NodeContact<Key>, i64)> = Vec::new();
            loop {
                match a_rx.recv().unwrap() {
                    AsyncAction::Awake => {
                        //these are orthogonal. can be handled in an isolated thread
                        if !timeoutbuf.is_empty() {
                            let now_secs = get_time().sec;
                            let (exp, rem):(Vec<_>, Vec<_>) = timeoutbuf.into_iter().partition(|&(ref ec, ref expire)| expire >= &now_secs);
                            timeoutbuf = rem;
                            //TODO: this needs to signal back to the k-buckets owner to update
                            println!("UNHANDLED");
                        }
                        /*for f in find_out.iter() {
                            
                        }*/

                    },
                    AsyncAction::SetEvictTimeout(ec) => {
                        let expire_at = get_time().sec + DEFAULT_TTL;
                        timeoutbuf.push((ec, expire_at));
                    },
                    AsyncAction::LookupResults(key, mut close_nodes, from_id) => {
                        let is_new = {
                            let f_res = lookup_qi.iter_mut().find(|&&mut(k, _)| k == key);
                            match f_res {
                                None => true,
                                Some(&mut (ref a, ref mut key_vec)) => {
                                    Self::merge_into(key_vec, &mut close_nodes, &key);
                                    //unoptimized... set the from_id to black (visited)
                                    if let Some(fid) = from_id {
                                        if let Some(&mut( _, ref mut color)) = key_vec.iter_mut().find(|&&mut(NodeContact{id: key,..}, _)| key == fid) {
                                            *color = Color::Black;
                                        } // probably should have gone with a HM
                                        find_out.retain(|&(NodeContact{id: fe, ..}, _)| fe != fid);
                                    }
                                    match is_lookup_finished!(ap.k_val, key_vec) {
                                        false => {
                                            Self::color(key_vec, ALPHA_FACTOR - find_out.len(), |&mut find_entry| {
                                                let NodeContact{ref ip, ref port, ..} = find_entry;
                                                let _ = alpha_sock.send_to(&ap.find_node_msg(&key), ip_port_pair(ip, port));
                                                find_out.push((find_entry, get_time().sec+1));
                                            });
                                        },
                                        true => {
                                            println!("DONE");
                                            let _ = a_tx_self.send(AsyncAction::Awake);
                                            //signal that we're done
                                        }
                                    };
                                    false
                                }
                            }
                        };
                        if is_new { //assumption here is that new entries cannot be in a finished state
                            let mut new_entry = Vec::new();
                            Self::merge_into(&mut new_entry, &mut close_nodes, &key);
                            Self::color(&mut new_entry, ALPHA_FACTOR - find_out.len(), |&mut find_entry| {
                                let NodeContact{ref ip, ref port, ..} = find_entry;
                                let _ = alpha_sock.send_to(&ap.find_node_msg(&key), ip_port_pair(ip, port));
                                find_out.push((find_entry, get_time().sec+1));
                            });
                            lookup_qi.push((key, new_entry));
                        }
                    }
                }
            }
        })
    }

    ///proto thread waits for messages from other nodes to come in over a designated UdpSocket.
    ///Valid protocol messages are passed onto the state thread
    fn spawn_proto_thread(mut receiver: UdpSocket, m_tx: Sender<MessageType>) -> JoinHandle<()> {
        thread::spawn(move|| {
            loop {
                match receiver.wait_for_message() {
                    Ok((message, node_id, address)) => {
                        let _ = m_tx.send(MessageType::FromNode(message, node_id, address));
                    },
                    _ => ()
                };
            }
        })
    }

    /// 'Colors' at most num_to_color elements Grey and runs a function accepting a generic T
    fn color <F, T>(field: &mut[(T, Color)], num_to_color: usize, mut func: F) where F:FnMut(&mut T) -> (){
        //i wonder how the FP facilities in rust compare
        let mut num_left = num_to_color;
        for &mut(ref mut t, ref mut c) in field.iter_mut() {
            match c {
                &mut Color::White => {
                    *c = Color::Grey(get_time().sec + 1);
                    func(t);
                    num_left -= 1;
                    if num_left == 0 {
                        break
                    }
                },
                _ => ()
            }
        }
    }

    fn merge_into (into: &mut Vec<(NodeContact<Key>, Color)>, candidates: &mut Vec<NodeContact<Key>>, basis: &Key) {
        let mut list: Vec<Option<usize>> = vec![];
        {
            let mut cand = candidates.iter().enumerate().peekable();
            let mut present = into.iter().enumerate().peekable();
            loop {
                match (cand.peek(), present.peek()) {
                    (None, _)|(_, None) => break,
                    (Some(&(c_i, &NodeContact{id: c_id, ..})), Some(&(_, &(NodeContact{id: p_id, ..}, ref color)))) => {
                        if c_id == p_id {
                            cand.next();
                            present.next();
                            list.push(None);
                        } else {
                            let greater = KademliaNode::cmp_dist_wrt(&c_id, &p_id, basis);
                            match greater {
                                a if a == Some(&p_id) => {
                                    list.push(Some(c_i)); //push the smaller one
                                    cand.next();
                                },
                                _ => { present.next(); }
                            }
                        }
                    }
                }
            }
            list.extend(cand.map(|_| Some(candidates.len())));
        }
        for (i, new_item) in list.iter().rev().zip(candidates.iter()) {
            match i {
                &Some(index) => into.insert(index, (new_item.clone(), Color::White)),
                _ => ()
            };
        }
    }

}

struct AlphaProcessor {
    id: NodeAddr,
    k_val: usize
}

impl ProtoMessage for AlphaProcessor {
    fn id (&self) -> &NodeAddr {
        &self.id
    }
}

type FindEntry = (NodeAddr, [u8; 4], u16);

#[derive(Clone, PartialEq)]
///Colors a (kbucket) value representing its status in an arbitrary asynchronous lookup operation
enum Color {
    Black, // Responded
    Grey(i64), // InTransit, value means the time it is valid for
    White, // Unvisited
    Yellow // Quarantined (Timedout)
}
