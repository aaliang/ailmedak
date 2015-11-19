use rand::{thread_rng, Rng, Rand};
use std::cmp::Ordering;
use std::mem;
use std::net::{UdpSocket, SocketAddr, ToSocketAddrs, Ipv4Addr};
use std::collections::HashMap;
use message_protocol::{DSocket, Message, Key, Value, ProtoMessage, u16_to_u8_2};
use api_layer::{spawn_api_thread, ClientMessage, Callback};
use time::get_time;

//the size of address space, in bytes
macro_rules! addr_spc { () => { 20 } }

/// A macro for generating a Node trait. Node implementations define how a communicating entity
/// identifies itself and other entities. More specifically it must implement a unique (or unique
/// enough id) as well as a distance metric (such as XOR)
macro_rules! meta_node {
    ($name: ident (id_len = $length: expr)) => {
        pub trait $name <T> where T: PartialEq + PartialOrd + Rand {
            fn my_id (&self) -> &[T; $length];
            fn dist_as_bytes (a: &[T; $length], b: &[T; $length]) -> [T; $length];
            fn distance_to (&self, to: &[T; $length]) -> [T; $length] {
                Self::dist_as_bytes(self.my_id(), to)
            }
            fn gen_new_id () -> [T; $length] {
                let mut id_buf:[T; $length] = unsafe { mem::uninitialized() };
                let mut nrg = thread_rng();
                let iter = nrg.gen_iter::<T>();
                //this is kind of a silly way to avoid using the heap
                for (x, i) in iter.take(addr_spc!()).enumerate() {
                    id_buf[x] = i;
                }
                id_buf
            }
            fn cmp_dist <'a> (a: &'a[T; $length], b: &'a[T; $length]) -> Option<&'a[T; $length]>{
                let zipped = a.iter().zip(b.iter());
                for (_a, _b) in zipped {
                    if _a < _b {
                        return Some(a)
                    }
                    else if _a > _b {
                        return Some(b)
                    }
                }
                return None
            }
            fn cmp_dist_wrt <'a> (a: &'a[T; $length], b: &'a[T; $length], basis: &'a[T; $length]) -> Option<&'a[T; $length]> {
                let a_dist = Self::dist_as_bytes(basis, a);
                let b_dist = Self::dist_as_bytes(basis, b);
                match Self::cmp_dist(&a_dist, &b_dist) {
                    Some(_a) if _a == &a_dist => Some(a),
                    Some(_b) if _b == &b_dist => Some(b),
                    _ => panic!("invalid dist")
                }
            }
        }
    };
}

//for now do not use meta_k_bucket! going with vector for maximum flexibility
macro_rules! meta_k_bucket {
    ($name:ident <$addr_type: ident> (k = $ksize:expr) ) => {
        type $name = [NodeAddr; $ksize];
    }
}

pub type NodeAddr = [u8; addr_spc!()];
type BucketArray = [Vec<(NodeAddr, SocketAddr)>; addr_spc!() * 8 + 1];

///Defines a trait called ASizedNode (using the meta_node template) using a fixed length array of
///an arbitrary type as id (currently set to 20)
meta_node!(ASizedNode (id_len = addr_spc!()));

pub struct KademliaNode {
    addr_id: NodeAddr,
    buckets: BucketArray,
    k_val: usize,
    data: HashMap<Key, Vec<u8>>,
    socket: UdpSocket
}

///Implements ProtoMessage so we can create Message envelopes
impl ProtoMessage for KademliaNode {
    fn id (&self) -> &NodeAddr {
        &self.addr_id
    }
}

///Concrete implementation using ASizedNode, uses exclusive or (XOR) as a distance metric
impl ASizedNode<u8> for KademliaNode {
    fn my_id (&self) -> &NodeAddr {
        &self.addr_id
    }
    fn dist_as_bytes(a: &NodeAddr, b: &NodeAddr) -> NodeAddr {
        let mut dist:NodeAddr = unsafe { mem::uninitialized() };
        for (i, (_a, _b)) in a.iter().zip(b.iter()).enumerate() {
            dist[i] = _a ^ _b;
        }
        dist
    }
}

#[derive(Debug)]
pub struct EvictionCandidate {
    old: (NodeAddr, SocketAddr),
    new: (NodeAddr, SocketAddr)
}

impl KademliaNode {
    pub fn new (id: NodeAddr, k_val: usize, write_socket: UdpSocket) -> KademliaNode {
        let mut buckets:BucketArray = unsafe {mem::uninitialized()};
        for i in buckets.iter_mut() {
            unsafe {::std::ptr::write(i, Vec::with_capacity(k_val)) };
        }
        KademliaNode {
            addr_id: id,
            buckets: buckets,
            k_val: k_val,
            data: HashMap::new(),
            socket: write_socket
        }
    }

    ///Returns the index of the k_bucket (within KademliaNode::buckets) that the given distance
    ///belongs in
    pub fn k_bucket_index (distance: &NodeAddr) -> usize {
        match distance.iter().enumerate().find(|&(i, byte_val)| *byte_val != -1) {
            Some ((index, val)) => {
                let push_macro = 8 * (distance.len() - index - 1);
                let push_micro = 8 - (val.leading_zeros() as usize) - 1;
                let comb_push = push_macro + push_micro;
                comb_push
            },
            _ => 0
        }
    }

    pub fn update_k_bucket (&mut self, k_index: usize, tup: (NodeAddr, SocketAddr)) -> Option<EvictionCandidate> {
        let (node_id, sock_addr) = tup;
        let mut k_bucket = &mut self.buckets[k_index];
        let _ = k_bucket.retain(|&(n, _)| node_id != n);
        if k_bucket.len() < self.k_val {
            k_bucket.push(tup);
            None
        } else {
            //TODO: need to ping
            let last_recently_seen = k_bucket.first().unwrap();

            Some(EvictionCandidate{
                new: tup,
                old: last_recently_seen.to_owned()
            })
        }
    }

    ///Ailmedak's (naive) version of locate node
    ///A vector of closest addresses of length {K factor} or {the total number of contacts} (whichever is smaller is) is
    ///returned
    /*fn find_k_closest_global(&self, target_node_id: &Key, alpha_channel: &Sender<Message<Key, Value>>) {
        let local_closest = self.find_k_closest(target_node_id);
        //TODO: consider case where there are no contacts
        alpha_channel.send(AsyncAction::LookupResults(local_closest));
    
    }*/


    fn find_k_closest (&self, target_node_id: &Key) -> Vec<(Key, (Key, ([u8; 4], [u8; 2])))> {
        let mut ivec = Vec::with_capacity(self.k_val);
        let fbuckets = self.buckets.iter().flat_map(|bucket| bucket.iter());

        fbuckets.fold(ivec, |mut acc, c| {
            let &(node_id, s_addr) = c;
            let dist = Self::dist_as_bytes(&node_id, target_node_id);
            let todo = {
                //yields the first element that is greater than the current
                //distance
                let find_result = acc.iter().enumerate().find(|&(i, x)| {
                    let &(i_dist, _) = x;
                    match Self::cmp_dist(&i_dist, &dist) {
                        a if a == Some(&i_dist) => true,
                        _ => false
                    }
                });
                match find_result {
                    None => {
                        if acc.len() >= self.k_val {
                            None
                        } else {
                            Some(acc.len())
                        }
                    },
                    Some((i, _)) => { Some(i) }
                }
            };
            match todo {
                Some(i) => {
                    acc.insert(i, (dist, (node_id, ip_port_pair(s_addr))));
                    if acc.len() > self.k_val {
                        acc.pop();
                    }
                },
                _ => ()
            };
            assert!(acc.len() <= self.k_val);
            acc
        })
    }

    pub fn send_msg <A:ToSocketAddrs> (&self, msg: &[u8], addr: A) {
        self.socket.send_to(msg, addr);
    }

}

fn ip_port_pair (s_addr: SocketAddr) -> ([u8; 4], [u8; 2]) {
    match s_addr {
        SocketAddr::V4(s) => (s.ip().octets(), u16_to_u8_2(&s.port())),
        _ => panic!("IPV4 NOT CURRENTLY SUPPORTED")
    }
}

#[derive(Debug)]
pub enum AsyncAction {
    Awake,
    SetEvictTimeout(EvictionCandidate),
    LookupResults(Key, Vec<(Key, [u8; 4], u16)>)
    //PingResp(),

}

trait ReceiveMessage <M, A> {
    fn receive (&mut self, msg: M, src_addr: SocketAddr, a_sender: &Sender<A>);
}

impl ReceiveMessage <Message<Key, Value>, AsyncAction> for KademliaNode {
    fn receive (&mut self, msg: Message<Key, Value>, src_addr: SocketAddr, a_sender: &Sender<AsyncAction>) {
        match msg {
            Message::Ping => {
                self.socket.send_to(&self.ping_ack(), src_addr);
            },
            Message::FindNode(key) => {
                let kclosest = self.find_k_closest(&key);
                let response = self.find_node_resp(&kclosest, &key);
                self.socket.send_to(&response, src_addr);
            },
            Message::FindVal(key) => {
                self.socket.send_to(&(match self.data.get(&key) {
                    None => self.find_node_resp(&self.find_k_closest(&key), &key),
                    Some(data) => self.find_val_resp(&key, data)
                }), src_addr);
            },
            Message::Store(key, val) => {self.data.insert(key, val);},
            //Responses
            Message::FindNodeResp(key, node_vec) => {
                a_sender.send(AsyncAction::LookupResults(key, node_vec));
            },
            Message::PingResp => {
                println!("unhandled right now");
            }
            _ => {}
        }
    }
}

pub struct AilmedakMachine {
    socket: UdpSocket,
    id: NodeAddr
}

pub trait Machine {
    fn start (&mut self, port: u16);
}

use std::thread;
use std::thread::JoinHandle;
use std::sync::mpsc::{Sender, Receiver, channel};

const DEFAULT_TTL:i64 = 3;

pub enum MessageType {
    FromClient(ClientMessage),
    FromNode(Message<Key, Value>, NodeAddr, SocketAddr)
}

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

impl AilmedakMachine {

    pub fn with_id(port: u16, id: NodeAddr) -> AilmedakMachine {
        let mut socket = match UdpSocket::bind(("0.0.0.0", port)) {
            Ok(a) => a,
            _ => panic!("unable to bind")
        };
        AilmedakMachine {
            socket: socket,
            id: id
        }
    }

    pub fn new(port: u16) -> AilmedakMachine {
        Self::with_id(port, KademliaNode::gen_new_id())
    }

    pub fn init_state (&self, k_val: usize) -> KademliaNode {
        KademliaNode::new(self.id.clone(), k_val, self.socket.try_clone().unwrap())
    }

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
    ///
    pub fn start (&self) {
        let mut state = self.init_state(8);
        let mut receiver = self.socket.try_clone().unwrap();
        let ap = AlphaProcessor {id: self.id.clone(), k_val: state.k_val.clone()};

        let (m_tx, m_rx) = channel();
        let (a_tx, a_rx) = channel();
        let m_tx_reader = m_tx.clone();
        let reader_thread = thread::spawn(move|| {
            loop {
                match receiver.wait_for_message() {
                    Ok((message, node_id, address)) => {m_tx_reader.send(MessageType::FromNode(message, node_id, address));},
                    _ => ()
                };
            }
        });

        //TODO: this needs to be opt-in
        let (_, cb_tx) = spawn_api_thread(5000, m_tx.clone());
        let cb_tx_state = cb_tx.clone();

        let a_tx_state = a_tx.clone();
        let state_thread = thread::spawn(move|| {
            loop {
                match m_rx.recv().unwrap() {
                    MessageType::FromClient(message) => {
                        println!("{:?}", message);
                        match message {
                            ClientMessage::Get(key) => {
                                match state.data.get(&key) {
                                    None => {
                                        println!("need to node_lookup");
                                    },
                                    Some(data) => {
                                        println!("f");
                                        cb_tx_state.send(Callback::Resolve(key, data.clone()));
                                    }
                                }
                            },
                            ClientMessage::Set(key, val) => { state.data.insert(key, val);}
                        };
                    }
                    MessageType::FromNode(message, node_id, addr) => {
                        let diff = state.distance_to(&node_id);
                        let k_index = KademliaNode::k_bucket_index(&diff);
                        let e_cand = state.update_k_bucket(k_index, (node_id, addr));
                        match e_cand {
                            None => (),
                            Some(e_c) => {
                                (&a_tx_state).send(AsyncAction::SetEvictTimeout(e_c));
                                state.send_msg(&state.ping_msg(), addr);
                            }
                        };
                        println!("addr: {:?}", addr);
                        println!("them: {:?}", node_id);
                        println!("us: {:?}", state.my_id());
                        println!("diff: {:?}", &diff[..]);
                        println!("got {:?}", message);

                        let action = state.receive(message, addr, &a_tx_state);
                    }

                }

            }
        });

        let alpha_sock = self.socket.try_clone().unwrap();
        let ALPHA_FACTOR = 4; //system wide FIND_NODE limit
        //alpha processor processes events that may be waiting on a future condition. performance
        //requirements are less stringent within this thread

        //this sender allows a feedback loop back into into itself
        let a_tx_self = a_tx.clone();
        let alpha_processor = thread::spawn(move|| {
            let mut timeoutbuf:Vec<(EvictionCandidate, i64)> = Vec::new();
            //It would probably be better to use a HashMap for highly concurrent api requests
            //but for now focus on lower latency in small batches. maybe make this configurable
            let mut lookup_qi: Vec<(Key, Vec<(FindEntry, Color)>)> = Vec::new();
            let mut find_out:Vec<(FindEntry, i64)> = Vec::new();

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
                        for f in find_out.iter() {
                            //
                        }

                    },
                    AsyncAction::SetEvictTimeout(ec) => {
                        let expire_at = get_time().sec + DEFAULT_TTL;
                        timeoutbuf.push((ec, expire_at));
                    },
                    AsyncAction::LookupResults(key, mut close_nodes) => {
                        let is_new = {
                            let f_res = lookup_qi.iter_mut().find(|&&mut(k, _)| k == key);
                            match f_res {
                                None => true,
                                Some(&mut (ref a, ref mut key_vec)) => {
                                    Self::merge_into(key_vec, &mut close_nodes, &key);
                                    match is_lookup_finished!(ap.k_val, key_vec) {
                                        false => {
                                            Self::color(key_vec, ALPHA_FACTOR - find_out.len(), |find_entry| {
                                                let (ref node_id, ref ip, ref port) = find_entry;
                                                alpha_sock.send_to(&ap.find_node_msg(&key), to_ip_port_pair(ip, port));
                                                find_out.push((find_entry, get_time().sec+1));
                                            });
                                        },
                                        true => {
                                            println!("DONE");
                                            a_tx_self.send(AsyncAction::Awake);
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
                            Self::color(&mut new_entry, ALPHA_FACTOR - find_out.len(), |find_entry| {
                                let (ref node_id, ref ip, ref port) = find_entry;
                                alpha_sock.send_to(&ap.find_node_msg(&key), to_ip_port_pair(ip, port));
                                find_out.push((find_entry, get_time().sec+1));
                            });
                            lookup_qi.push((key, new_entry));
                        }
                    }
                }
            }
        });

        loop {
            thread::sleep_ms(300);
            a_tx.send(AsyncAction::Awake);
        }
    }

    /// 'Colors' at most num_to_color elements Grey and runs a function accepting a generic T
    fn color <F, T>(field: &mut[(T, Color)], num_to_color: usize, func: F) where F:FnMut(T) -> (){
        //i wonder how the FP facilities in rust compare
        let mut num_left = num_to_color;
        for &mut(_, ref mut c) in field.iter_mut() {
            match c {
                &mut Color::White => {
                    *c = Color::Grey(get_time().sec + 1);
                    num_left -= 1;
                    if num_left == 0 {
                        break
                    }
                },
                _ => ()
            }
        }
    }

    fn merge_into (into: &mut Vec<(FindEntry, Color)>, candidates: &mut Vec<FindEntry>, basis: &Key) {
        let mut list: Vec<Option<usize>> = vec![];
        {
            let mut cand = candidates.iter().enumerate().peekable();
            let mut present = into.iter().enumerate().peekable();
            loop {
                match (cand.peek(), present.peek()) {
                    (None, _)|(_, None) => break,
                    (Some(&(c_i, &(c_id, _, _))), Some(&(p_i, &((p_id, _, _), ref color)))) => {
                        if c_id == p_id {
                            cand.next();
                            present.next();
                            list.push(None);
                        } else {
                            let greater = KademliaNode::cmp_dist_wrt(&c_id, &p_id, basis);
                            match greater {
                                a if a == Some(&p_id) => {
                                    list.push(Some(p_i));
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

    pub fn send_msg <A:ToSocketAddrs> (&self, msg: &[u8], addr: A) {
        self.socket.send_to(msg, addr);
    }
}

fn to_ip_port_pair(ip: &[u8; 4], port: &u16) -> (Ipv4Addr, u16) {
    (Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), *port)
}

impl ProtoMessage for AilmedakMachine {
    fn id (&self) -> &NodeAddr {
        &self.id
    }
}

//this is out of hand... REFACTOR LATER
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
