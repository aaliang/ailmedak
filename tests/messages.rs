extern crate ailmedak;

use ailmedak::message_protocol::*;
use ailmedak::node::NodeAddr;

const KEYSIZE:&'static usize = &20;
static MOCK_ID:[u8; 20] = [9; 20];

struct MessageFactory;

impl ProtoMessage for MessageFactory {
    fn id(&self) -> &NodeAddr{
        &MOCK_ID
    }
}

#[test]
fn msg_ping() {
    let ping = MessageFactory.ping_msg();
    let ds = try_decode(&ping, KEYSIZE).unwrap();
    assert_eq!(ds, (Message::Ping, MOCK_ID));
}

#[test]
fn msg_ping_ack() {
    let ping_ack = MessageFactory.ping_ack();
    let ds = try_decode(&ping_ack, KEYSIZE).unwrap();
    assert_eq!(ds, (Message::PingResp, MOCK_ID));
}

#[test]
fn msg_store() {
    let key = [10; 20];
    let val = vec![1, 2, 3, 4, 5];
    let store = MessageFactory.store_msg(&key, &val);
    let ds = try_decode(&store, KEYSIZE).unwrap();
    assert_eq!(ds, (Message::Store(key, val), MOCK_ID));
}

#[test]
fn msg_find_node() {
    let key = [10; 20];
    let find_val = MessageFactory.find_node_msg(&key);
    let ds = try_decode(&find_val, KEYSIZE).unwrap();
    assert_eq!(ds, (Message::FindNode(key), MOCK_ID));
}

#[test]
fn msg_find_val() {
    let key = [10; 20];
    let find_val = MessageFactory.find_val_msg(&key);
    let ds = try_decode(&find_val, KEYSIZE).unwrap();
    assert_eq!(ds, (Message::FindVal(key), MOCK_ID));
}

/*#[test]
fn resp_find_node() {
    let key = [10; 20];
    let closest = vec![
        ([1; 20], ([5; 20], ([1, 2, 3, 4], [1, 2]))),
        ([2; 20], ([6; 20], ([5, 6, 7, 8], [3, 4])))
        ];
    let find_node_resp = MessageFactory.find_node_resp(&closest, &key);
    let ds = try_decode(&find_node_resp, KEYSIZE).unwrap();
    assert_eq!(ds, (Message::FindNodeResp(key, vec![
                                          //([1; 20])
                                          ]), MOCK_ID));
}*/ //on hold. refactoring
