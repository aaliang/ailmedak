extern crate ailmedak;

use ailmedak::node::{KademliaNode, ASizedNode, Machine, AilmedakMachine};
use ailmedak::message_protocol::ProtoMessage;
use std::env;
use std::mem;
use std::net::UdpSocket;

fn main () {

    //let node = KademliaNode::new(50);

    let port = env::args().nth(1).unwrap()
                                 .parse::<u16>()
                                 .unwrap();

    //let id = [255, 164, 237, 35, 202, 140, 149, 147, 86, 65, 224, 50, 236, 44, 179, 183, 114, 54, 239, 55];
    let mut machine = AilmedakMachine::new(port);

    println!("{:?}", machine.id());



    let key:[u8; 20] = [5; 20];
    let val:[u8; 100] = unsafe {mem::uninitialized()};

    let remote = ("0.0.0.0", 5557);
    let msg = machine.find_node_msg(&key);
    println!("{:?}", &msg[..]);
    machine.send_msg(&msg, ("0.0.0.0", 5557));

}
