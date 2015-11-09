use rand::{thread_rng, Rng, Rand};
use std::mem;
use std::net::UdpSocket;

//the size of address space, in bytes
macro_rules! addr_spc { () => { 20 } }

macro_rules! meta_node {
    ($name: ident (id_len = $length: expr)) => {
        pub trait $name <T> where T: PartialEq + PartialOrd + Rand {
            fn my_id (&self) -> &[T; $length];
            fn dist_as_bytes (a: &[T; $length], b: &[T; $length]) -> [T; $length];
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

type NodeAddr = [u8; addr_spc!()];
type BucketArray = [Vec<NodeAddr>; addr_spc!() * 8];

meta_node!(ASizedNode (id_len = addr_spc!()));
meta_k_bucket!(BigBucket<NodeAddr> (k = 50) );

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
    pub fn new (k_val: usize) -> KademliaNode {
        let mut buckets:BucketArray = unsafe {mem::uninitialized()};

        for i in buckets.iter_mut() {
            unsafe {::std::ptr::write(i, Vec::with_capacity(k_val)) };
        }

        KademliaNode {
            addr_id: Self::gen_new_id(),
            buckets: buckets,
            k_val: k_val
        }
    }
}

pub struct Machine <T> {
    pub node: T
}

use std::thread;
use std::sync::mpsc::channel;

impl <T> Machine <T> {
    pub fn start (&self, port: u16) {
        let t = thread::spawn(move|| {
            let mut listen = UdpSocket::bind(("0.0.0.0", port)).unwrap();
            loop {
                let mut buf:[u8; 512] = [0; 512];
                let (num_read, addr) = listen.recv_from(&mut buf).unwrap();
                println!("{:?}", &buf[..]);
                /*
                // Send a reply to the socket we received data from
                let buf = &mut buf[..amt];
                buf.reverse();
                try!(listen.send_to(buf, &src));*/
            }
        });

        t.join();
    }
}
