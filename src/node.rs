use rand::{thread_rng, Rng, Rand};
use std::mem;
use std::net::{UdpSocket, SocketAddr, ToSocketAddrs};
use std::collections::HashMap;
use message_protocol::{DSocket, Message, Key, Value, ProtoMessage, u16_to_u8_2};

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
    data: HashMap<Key, Value>,
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

    pub fn update_k_bucket (&mut self, k_index: usize, tup: (NodeAddr, SocketAddr)) {
        let (node_id, sock_addr) = tup;
        let mut k_bucket = &mut self.buckets[k_index];
        let _ = k_bucket.retain(|&(n, _)| node_id != n);
        if k_bucket.len() < self.k_val {
            k_bucket.push(tup);
        } else {
            //TODO: need to ping
            println!("TODO: NEED TO HANDLE PING");
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
}

fn ip_port_pair (s_addr: SocketAddr) -> ([u8; 4], [u8; 2]) {
    match s_addr {
        SocketAddr::V4(s) => (s.ip().octets(), u16_to_u8_2(&s.port())),
        _ => panic!("IPV4 NOT CURRENTLY SUPPORTED")
    }
}

trait ReceiveMessage <M> {
    fn receive (&mut self, msg: M, src_addr: SocketAddr);
}

impl ReceiveMessage <Message<Key, Value>> for KademliaNode {
    fn receive (&mut self, msg: Message<Key, Value>, src_addr: SocketAddr) {
        match msg {
            Message::FindNode(key) => {
                let k_closest = self.find_k_closest(&key);
                let message = self.find_node_resp(&k_closest, &key);
                //this is kind of messy. might want to make it return an enum to do sending
                self.socket.send_to(&message, src_addr);
            },
            Message::Store(key, val) => {
                self.data.insert(key, val);
               // Action::Nothing
            },
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
        let (tx, rx) = channel();
        let reader = thread::spawn(move|| {
            loop {
                match receiver.wait_for_message() {
                    Ok(a) => {tx.send(a);},
                    _ => ()
                };
            }
        });

        //let mut send_socket = self.socket.try_clone().unwrap();
        let state_thread = thread::spawn(move|| {
            //let send_socket =             i
            loop {
                let (message, node_id, addr) = rx.recv().unwrap();
                let diff = state.distance_to(&node_id);
                let k_index = KademliaNode::k_bucket_index(&diff);
                let _ = state.update_k_bucket(k_index, (node_id, addr));
                println!("addr: {:?}", addr);
                println!("them: {:?}", node_id);
                println!("us: {:?}", state.my_id());
                println!("diff: {:?}", &diff[..]);
                println!("got {:?}", message);

                let action = state.receive(message, addr);

            }
        });

        //main thread handles api requests
        //this is temporary
        reader.join();
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
