use rand::{thread_rng, Rng};


///FixedSizeArray isn't stable yet to be used as a trait so macros are the only way to achieve easy
///configurable arrays on the stack (at compile time). The easy way would be to use vectors but
///those are boxed and therefore heap allocated
///
/// our address space is 20 bytes of random fun (160 bits)
macro_rules! addr_spc {
    () => { 20 };
}

trait BaseNode <T> {
    fn my_id (&self) -> &T;
    fn gen_new_id () -> T;
}

#[derive(Debug)]
pub struct KademliaNode {
    addr_id: [u8; addr_spc!()]
}

impl BaseNode <[u8; addr_spc!()]> for KademliaNode {
    fn my_id (&self) -> &[u8; addr_spc!()] {
        &self.addr_id
    }
    fn gen_new_id() -> [u8; addr_spc!()] {
        let mut id_buf = [0; addr_spc!()];
        let mut nrg = thread_rng();
        let iter = nrg.gen_iter::<u8>();
        //this is kind of a silly way to avoid using the heap
        for (x, i) in iter.take(addr_spc!()).enumerate() {
            id_buf[x] = i;
        }
        id_buf
    }
}


impl KademliaNode {
    pub fn new () -> KademliaNode {
        KademliaNode {
            addr_id: Self::gen_new_id()
        }
    }
}
