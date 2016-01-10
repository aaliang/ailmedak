use rand::{thread_rng, Rng, Rand};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::mem;
use std::collections::HashMap;
use std::sync::mpsc::{Sender};
use message_protocol::{Key, ProtoMessage, NodeContact};
use node::machine::{EvictionCandidate, AsyncAction};
use utils::networking::{ip_port_pair_bytes};
use utils::{u8_2_to_u16};

//the size of address space, in bytes
macro_rules! addr_spc { () => { 20 } }

/// A macro for generating a Node trait. Node implementations define how a communicating entity
/// identifies itself and other entities. More specifically it must implement a unique (or unique
/// enough id) as well as a distance metric (such as XOR)
macro_rules! meta_node {
    ($name: ident (id_len = $length: expr)) => {
        pub trait $name <T> where T: PartialEq + PartialOrd + Rand {

            fn my_id (&self) -> &[T; $length];

            /// an arbitrary distance metric
            fn dist_as_bytes (a: &[T; $length], b: &[T; $length]) -> [T; $length];

            /// Returns the distance from this to another id
            fn distance_to (&self, to: &[T; $length]) -> [T; $length] {
                Self::dist_as_bytes(self.my_id(), to)
            }

            /// Generates a new id, randomly
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

            /// Given two ids/addresses returns an option containing whichever one is larger
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

            /// Given two ids and a basis, returns an option over the id which is further from the
            /// basis
            fn cmp_dist_wrt <'a> (a: &'a[T; $length], b: &'a[T; $length], basis: &'a[T; $length]) -> Option<&'a[T; $length]> {
                let a_dist = Self::dist_as_bytes(basis, a);
                let b_dist = Self::dist_as_bytes(basis, b);
                match Self::cmp_dist(&a_dist, &b_dist) {
                    Some(_a) if _a == &a_dist => Some(a),
                    Some(_b) if _b == &b_dist => Some(b),
                    _ => None
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
pub type BucketArray = [Vec<(NodeAddr, SocketAddr)>; addr_spc!() * 8 + 1];

///Defines a trait called ASizedNode (using the meta_node template) using a fixed length array of
///an arbitrary type as id (currently set to 20)
meta_node!(ASizedNode (id_len = addr_spc!()));

pub struct KademliaNode {
    pub addr_id: NodeAddr,
    pub buckets: BucketArray,
    pub k_val: usize,
    pub data: HashMap<Key, Vec<u8>>,
    pub socket: UdpSocket
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

/// Vanilla implementation of the state of a node, according to the Kademlia paper.
/// provides facilities for retrieving and putting into k-buckets (governed by distance)
/// and returning the k closest known nodes (that are considered active) to a given id
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
        //find the first index that is not all ones. note: this needs to be tested thoroughly
        match distance.iter().enumerate().find(|&(_, byte_val)| *byte_val != 0) {
            Some ((index, val)) => {
                let push_macro = 8 * (distance.len() - index - 1);
                let push_micro = 8 - (val.leading_zeros() as usize) - 1;
                let comb_push = push_macro + push_micro;
                comb_push
            },
            _ => 0
        }
    }

    ///updates the k buckets to enforce least recently seen ordering
    pub fn update_k_bucket (&mut self, k_index: usize, tup: (NodeAddr, SocketAddr)) -> Option<EvictionCandidate> {
        let (node_id, _) = tup;
        let mut k_bucket = &mut self.buckets[k_index];
        let _ = k_bucket.retain(|&(n, _)| node_id != n);
        if k_bucket.len() < self.k_val {
            //add contact info if below threshold
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
    pub fn find_k_closest_global(&self, target_node_id: Key, alpha_channel: &Sender<AsyncAction>) {
        let local_closest = self.find_k_closest(&target_node_id).iter().map(|&(_, (node_id, (ip, port)))| {
            NodeContact{id: node_id, ip: ip, port: u8_2_to_u16(&port)}
        }).collect::<Vec<NodeContact<Key>>>();
        //TODO: consider case where there are no contacts
        let _ = alpha_channel.send(AsyncAction::LookupResults(target_node_id, local_closest, None));
    }

    /// finds locally, the k closest nodes to the target_node_id
    pub fn find_k_closest (&self, target_node_id: &Key) -> Vec<(Key, (Key, ([u8; 4], [u8; 2])))> {
        let ivec = Vec::with_capacity(self.k_val);
        let fbuckets = self.buckets.iter().flat_map(|bucket| bucket.iter());

        fbuckets.fold(ivec, |mut acc, c| {
            let &(node_id, s_addr) = c;
            let dist = Self::dist_as_bytes(&node_id, target_node_id);
            let todo = {
                //yields the first element that is greater than the current
                //distance
                let find_result = acc.iter().enumerate().find(|&(_, x)| {
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
                    acc.insert(i, (dist, (node_id, ip_port_pair_bytes(s_addr))));
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
        let _ = self.socket.send_to(msg, addr);
    }

}
