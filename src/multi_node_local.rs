extern crate ailmedak;

use ailmedak::node::machine::{AilmedakMachine};
use ailmedak::config::Config;

use std::thread;
// executable entry point for development purposes.
// spins multiple nodes up within a single process

fn main () {
    let num_slave_nodes = 3;
    let api_port = 4000;
    let port_range_start = 3000;

    thread::spawn(move|| {
        AilmedakMachine::start(Config {
            network_port: 3000,
            api_port: Some(api_port),
            k_val: 8,
            async_poll_interval: 300,
            initial_neighbors: vec![]
        }, None);
    });

    for i in 1..num_slave_nodes+1 {
        thread::spawn(move|| {
            AilmedakMachine::start(Config {
                network_port: port_range_start + i,
                api_port: None,
                k_val: 8,
                async_poll_interval: 300,
                initial_neighbors: vec!["0.0.0.0:3000".to_string()]
            }, None);
        });
    }

    thread::park();
}
