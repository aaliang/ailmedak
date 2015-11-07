use rand::{thread_rng, Rng, Rand};
use std::mem;

macro_rules! addr_spc { () => { 20 } }

macro_rules! meta_node {
    ($name: ident (id_len = $length: expr)) => {
        trait $name <T> where T: PartialEq + PartialOrd + Rand {
            fn my_id (&self) -> &[T; $length];
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
            fn dist_as_bytes (a: &[T; $length], b: &[T; $length]) -> [T; $length];
        }
    };
}

meta_node!(ASizedNode (id_len = addr_spc!()));

type NodeAddr = [u8; addr_spc!()];

#[derive(Debug)]
pub struct KademliaNode {
    addr_id: NodeAddr
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
    pub fn new () -> KademliaNode {
        KademliaNode {
            addr_id: Self::gen_new_id()
        }
    }
}
