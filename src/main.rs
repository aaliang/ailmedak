extern crate ailmedak;
extern crate getopts;

use ailmedak::node::{KademliaNode, ASizedNode, AilmedakMachine, Machine};
use ailmedak::message_protocol::ProtoMessage;
use ailmedak::config::Config;
use std::env;
use getopts::Options;

const DEFAULT_NETPORT:u16 = 3000;

fn main () {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();

    opts.optopt("p", "port", "port for internal ailmedak protocols", "PORTNUM");
    opts.optopt("a", "api-port", "client port", "PORTNUM");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string())
    };

    let port = match matches.opt_str("p") {
        Some(p) => p.parse::<u16>().unwrap(),
        None => DEFAULT_NETPORT
    };

    let api_port_opt = match matches.opt_str("a"){
        Some(s) => s.parse::<u16>().ok(),
        None => None
    };

    let configuration = Config {
        network_port: port,
        api_port: api_port_opt,
        k_val: 8,
        async_poll_interval: 300,
        initial_neighbors: vec![]
    };

    AilmedakMachine::start(configuration, None);

}
