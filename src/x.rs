extern crate ailmedak;

use ailmedak::node::{KademliaNode, ASizedNode, Machine};
use std::env;

use std::net::UdpSocket;

fn main () {

    let mut get = UdpSocket::bind(("0.0.0.0", 5556)).unwrap();

    let num_sent = get.send_to(&[1,2,3,4], ("0.0.0.0", 5555)).unwrap();

    println!("{}", num_sent);
}
