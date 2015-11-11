extern crate ailmedak;

use ailmedak::node::{KademliaNode, ASizedNode, AilmedakMachine, Machine};
use ailmedak::message_protocol::ProtoMessage;
use std::env;

fn main () {
    let port = env::args().nth(1)
                          .unwrap()
                          .parse::<u16>()
                          .unwrap();

    let mut machine = AilmedakMachine::new(port);
    machine.start();

}
