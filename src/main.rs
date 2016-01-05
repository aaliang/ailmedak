extern crate ailmedak;

use ailmedak::node::{KademliaNode, ASizedNode, AilmedakMachine, Machine};
use ailmedak::message_protocol::ProtoMessage;
use ailmedak::config::Config;
use std::env;

fn main () {
    let port = env::args().nth(1)
                          .unwrap()
                          .parse::<u16>()
                          .unwrap();

    let configuration = Config {
        network_port: port,
        api_port: Some(5000),
        k_val: 8
    };

    AilmedakMachine::start(configuration);

}
