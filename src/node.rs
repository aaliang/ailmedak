use rand::{thread_rng, Rng, Rand};
use std::mem;
use std::net::{UdpSocket, SocketAddr, ToSocketAddrs};
use std::collections::HashMap;
use message_protocol::{DSocket, Message, Key, Value, ProtoMessage, u16_to_u8_2};
use time::get_time;

//the size of address space, in bytes
macro_rules! addr_spc { () => { 20 } }
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

meta_node!(ASizedNode (id_len = addr_spc!()));

//TODO: this should be genericized
pub struct KademliaNode {
    addr_id: NodeAddr,
    buckets: BucketArray,
    k_val: usize,
    data: HashMap<Key, Vec<u8>>,
    socket: UdpSocket
}

impl ProtoMessage for KademliaNode {
    fn id (&self) -> &NodeAddr {
        &self.addr_id
    }
}

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

    //Ailmedak's (naive) version of locate node
    fn find_k_closest (&self, target_node_id: &Key) -> Vec<(Key, (Key, ([u8; 4], [u8; 2])))> {
        let mut ivec = Vec::with_capacity(self.k_val);
        let fbuckets = self.buckets.iter().flat_map(|bucket| bucket.iter());

        fbuckets.fold(ivec, |mut acc, c| {
            let &(node_id, s_addr) = c;
            let dist = Self::dist_as_bytes(&node_id, target_node_id);
            //TODO: this is a naive implementation, this can be optimized by
            //maintaining sorted order
            let remove_result = {
                //yields the first element that is greater than the current
                //distance
                if acc.len() >= self.k_val {
                    let find_result = acc.iter().enumerate().find(|&(i, x)| {
                        let &(i_dist, _) = x;
                        match Self::cmp_dist(&i_dist, &dist) {
                            a if a == Some(&i_dist) => true,
                            _ => false
                        }
                    });
                    match find_result {
                        None => None,
                        Some((i, _)) => Some(i)
                    }
                }
                else {
                    acc.push((dist, (node_id, ip_port_pair(s_addr))));
                    None
                }
            };
            match remove_result {
                Some(i) => {
                    acc.remove(i);
                    acc.push((dist, (node_id, ip_port_pair(s_addr))));
                },
                _ => ()
            };
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
    EvictTimeout(EvictionCandidate),
    LookupNode(Key, Vec<(Key, [u8; 4], u16)>)
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
            Message::FindVal (key) => {
                self.socket.send_to(&(match self.data.get(&key) {
                    None => self.find_node_resp(&self.find_k_closest(&key), &key),
                    Some(data) => self.find_val_resp(&key, data)
                }), src_addr);
            },
            Message::Store(key, val) => {self.data.insert(key, val);},
            //Responses
            Message::FindNodeResp(key, node_vec) => {
                a_sender.send(AsyncAction::LookupNode(key, node_vec));
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

    /// blocks
    pub fn start (&self) {
        let mut state = self.init_state(8);
        let mut receiver = self.socket.try_clone().unwrap();
        let (m_tx, m_rx) = channel();
        let (a_tx, a_rx) = channel();

        let reader = thread::spawn(move|| {
            loop {
                match receiver.wait_for_message() {
                    Ok(a) => {m_tx.send(a);},
                    _ => ()
                };
            }
        });

        let a_tx_state = a_tx.clone();
        let state_thread = thread::spawn(move|| {
            loop {
                let (message, node_id, addr) = m_rx.recv().unwrap();
                let diff = state.distance_to(&node_id);
                let k_index = KademliaNode::k_bucket_index(&diff);
                let e_cand = state.update_k_bucket(k_index, (node_id, addr));
                match e_cand {
                    None => (),
                    Some(e_c) => {
                        (&a_tx_state).send(AsyncAction::EvictTimeout(e_c));
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
        });

        let ALPHA_FACTOR = 6; //system wide FIND_NODE limit
        //alpha processor processes events that may be waiting on a future condition. performance
        //requirements are less stringent within this thread
        let alpha_processor = thread::spawn(move|| {
            let mut timeoutbuf:Vec<(EvictionCandidate, i64)> = Vec::new();
            let mut lookup_q:Vec<(Key, [u8; 4], u16)> = Vec::new();
            let mut find_out:Vec<((Key, [u8; 4], u16), i64)> = Vec::new();
            loop {
                match a_rx.recv().unwrap() {
                    AsyncAction::Awake => {
                        /*while !lookup_q.is_empty() && find_out < ALPHA_FACTOR {

                        }*/
                        //these are orthogonal. can be handled in an isolated thread
                        if !timeoutbuf.is_empty() {
                            let now_secs = get_time().sec;
                            let (exp, rem):(Vec<_>, Vec<_>) = timeoutbuf.into_iter().partition(|&(ref ec, ref expire)| expire >= &now_secs);
                            timeoutbuf = rem;
                            //TODO: this needs to signal back to the k-buckets owner to update
                            println!("UNHANDLED");
                        }
                    },
                    AsyncAction::EvictTimeout(ec) => {
                        let expire_at = get_time().sec + DEFAULT_TTL;
                        timeoutbuf.push((ec, expire_at));
                    },
                    AsyncAction::LookupNode(key, close_nodes) => {
                        lookup_q.extend(close_nodes.iter());
                    }
                }
                //println!("{:?}", a);
            }
        });

        loop {
            thread::sleep_ms(800);
            a_tx.send(AsyncAction::Awake);
        }

        //main thread handles api requests
        //this is temporary
        reader.join();
    }

    fn fill_to_capacity (lookup_q: &mut Vec<(Key, [u8; 4], u16)>, find_out: &mut Vec<((Key, [u8; 4], u16), i64)>, alpha_factor: &usize) {
        while !lookup_q.is_empty() && find_out.len() < *alpha_factor {
            let new_lookup = lookup_q.pop().unwrap();
            //TODO: need to outbound send the lookup
            find_out.push((new_lookup, get_time().sec + DEFAULT_TTL));
        }
    }

    pub fn send_msg <A:ToSocketAddrs> (&self, msg: &[u8], addr: A) {
        self.socket.send_to(msg, addr);
    }
}

impl ProtoMessage for AilmedakMachine {
    fn id (&self) -> &NodeAddr {
        &self.id
    }
}
