#[macro_export]

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
