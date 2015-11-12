extern crate ailmedak;

use ailmedak::node::{KademliaNode, ASizedNode, Machine, AilmedakMachine};
use ailmedak::message_protocol::ProtoMessage;
use std::env;
use std::mem;
use std::net::UdpSocket;

fn main () {
    let port = env::args().nth(1).unwrap()
                                 .parse::<u16>()
                                 .unwrap();

    let mut machine = match env::args().nth(2) {
        None => AilmedakMachine::new(port),
        Some(a) => {
            let id_num = a.parse::<usize>().unwrap();
            let id = match id_num {
                1 => [255, 164, 237, 35, 202, 140, 149, 147, 86, 65, 224, 50, 236, 44, 179, 183, 114, 54, 239, 55],
                _ => panic!("invalid id_num index")
            };
            AilmedakMachine::with_id(port, id)
        }
    };

    let key:[u8; 20] = [5; 20];
    let val:[u8; 100] = unsafe {mem::uninitialized()};

    let remote = ("0.0.0.0", 5557);
    let msg = machine.find_node_msg(&key);
    println!("{:?}", &msg[..]);
    machine.send_msg(&msg, ("0.0.0.0", 5557));

}
