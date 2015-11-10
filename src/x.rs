extern crate ailmedak;

use ailmedak::node::{KademliaNode, ASizedNode, Machine};
use ailmedak::message_protocol::{ping_msg, store_msg, find_val_msg, find_node_msg};
use std::env;
use std::mem;
use std::net::UdpSocket;

fn main () {

    let mut get = UdpSocket::bind(("0.0.0.0", 5556)).unwrap();

    let key:[u8; 20] = [5; 20];
    let val:[u8; 100] = unsafe {mem::uninitialized()};

    //let msg = store_msg(&key, &val);
    let msg = find_node_msg(&key);
    println!("{:?}", msg);
    let num_sent = get.send_to(&msg, ("0.0.0.0", 5557)).unwrap();

    println!("{}", num_sent);
}
