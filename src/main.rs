extern crate ailmedak;

use ailmedak::node::{KademliaNode, ASizedNode, AilmedakMachine, Machine};
use std::env;

fn main () {
    let node = KademliaNode::new(50);
    let dist = KademliaNode::dist_as_bytes(&[0; 20], node.my_id());

    println!("dist: {:?}", dist);

    let port = env::args().nth(1)
                          .unwrap()
                          .parse::<u16>()
                          .unwrap();

    let machine = AilmedakMachine {
        node: node
    };

    machine.start(port);
}
