#[allow(dead_code, unused_must_use, unused_imports)]

extern crate ailmedak;

use ailmedak::node::{NodeAddr, KademliaNode};

#[test]
fn test_k_bucket_index_0() {
    let dist = [0; 20];
    assert!(KademliaNode::k_bucket_index(&dist) == 0);
}

#[test]
fn test_k_bucket_index_1() {
    let mut dist = [0; 20];
    dist[19] = 1;
    let index = KademliaNode::k_bucket_index(&dist);
    assert!(index == 0);
}

#[test]
fn test_k_bucket_index_2() {
    let mut dist = [0; 20];
    dist[19] = 2;
    let index = KademliaNode::k_bucket_index(&dist);
    assert!(index == 1);
}

#[test]
fn test_k_bucket_index() {
    let dist = [1; 20];
    let bucket_index = KademliaNode::k_bucket_index(&dist);
    assert!(bucket_index == 152);
}
