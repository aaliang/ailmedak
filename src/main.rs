extern crate ailmedak;

use ailmedak::node::{KademliaNode};

fn main () {
    let node = KademliaNode::new();
    println!("{:?}", node);
}
