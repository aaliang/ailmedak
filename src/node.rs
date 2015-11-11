use rand::{thread_rng, Rng, Rand};
use std::mem;
use std::net::{UdpSocket, SocketAddr, ToSocketAddrs};
use message_protocol::{DSocket, Message, Key, Value, ProtoMessage};

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
type BucketArray = [Vec<NodeAddr>; addr_spc!() * 8 + 1];

meta_node!(ASizedNode (id_len = addr_spc!()));
//meta_k_bucket!(BigBucket<NodeAddr> (k = 50) );

pub struct KademliaNode {
    addr_id: NodeAddr,
    buckets: BucketArray,
    k_val: usize
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
    pub fn new (id: NodeAddr, k_val: usize) -> KademliaNode {
        let mut buckets:BucketArray = unsafe {mem::uninitialized()};

        for i in buckets.iter_mut() {
            unsafe {::std::ptr::write(i, Vec::with_capacity(k_val)) };
        }

        KademliaNode {
            addr_id: id,
            buckets: buckets,
            k_val: k_val
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

struct State;
impl State {
    fn new () -> State {
        State
    }
    fn update_k_buckets() {
    }

    fn k_bucket_for(&self, k: &Key) {
    }

    fn receive (&mut self, msg: &Message<Key, Value>) {
        match msg {
            _ => {
                println!("msg");
            }
        }
    }
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
        KademliaNode::new(self.id.clone(), k_val)
    }

    pub fn k_bucket_index (distance: &Key) -> usize {
        let x = distance.iter().enumerate().find(|&(i, byte_val)| {
            *byte_val != 0
        });

        match x {
            Some ((index, val)) => {
                let push_macro = 8 * (distance.len() - index - 1);
                let push_micro = 8 - (val.leading_zeros() as usize) - 1;
                let comb_push = push_macro + push_micro;
                comb_push
            },
            _ => 0
        }
    }

    /// blocks
    pub fn start (&self) {
        let state = self.init_state(50);

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

        let state_thread = thread::spawn(move|| {
            loop {
                //let x_id = self.my_id();
                let (ref message, ref node_id, ref addr) = rx.recv().unwrap();
                println!("addr: {:?}", addr);
                println!("them: {:?}", node_id); 
                println!("us: {:?}", state.my_id());

                let diff = state.distance_to(node_id);
                let k_index = Self::k_bucket_index(&diff);
                println!("kindex: {}", k_index);
                println!("diff: {:?}", &diff[..]);

                //let x = state.k_bucket_for(node_id);
                //let id = self.node.id();
                //state.update_k_buckets(addr);
                //state.receive(message);
                println!("got {:?}", message);
            }
        });

        //this is temporary
        reader.join();
    }

    pub fn send_msg <A:ToSocketAddrs> (&mut self, msg: &[u8], addr: A) {
        self.socket.send_to(msg, addr);
    }
}

impl ProtoMessage for AilmedakMachine {
    fn id (&self) -> &NodeAddr {
        &self.id
    }
}
