//extern crate
use std::thread;
use std::thread::JoinHandle;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::Arc;
use std::sync::mpsc::{Sender, Receiver, channel};
use std::net::UdpSocket;
use message_protocol::u8_4_to_u32;
use crypto::sha1::Sha1;
use crypto::digest::Digest;
use node::MessageType;
use message_protocol::{Message, Key, Value};

#[derive(Debug)]
pub enum ClientMessage {
    Get([u8; 20]),
    Set([u8; 20], Vec<u8>)
}

///Exposes Ailmedak to consumers (not nodes) who would like to access the core as a key value
///store. for now only UDP is used as transport
///TODO: perhaps it should not start the thread itself
pub fn spawn_api_thread (port: u16, send: Sender<MessageType>) -> JoinHandle<()>{
    //let (tx, rx) = channel();
    thread::spawn(move || {
        println!("binding client port: {}", port);
        let mut listener = UdpSocket::bind(("0.0.0.0", port)).unwrap();
        let sender = listener.try_clone().unwrap();
        loop {
            let mut buf:[u8; 2048] = [0; 2048];
            let (amt, src) = listener.recv_from(&mut buf).unwrap();
            match buf.first() {
                Some(&0) => { //this is a lookup type
                    let key_length = u8_4_to_u32(&buf[0..4]) as usize;
                    let key = &buf[4..4+key_length];
                    let mut sha = Sha1::new();
                    let _ = sha.input(key);
                    let mut hash_key:[u8; 20] = [0; 20];
                    let _ = sha.result(&mut hash_key);
                    send.send(MessageType::FromClient(ClientMessage::Get(hash_key)));
                },
                Some(&1) => { //this is a store
                    let key_length = u8_4_to_u32(&buf[0..4]) as usize;
                    let val_length = u8_4_to_u32(&buf[4..8]) as usize;

                    let key = &buf[8..8+key_length];
                    let val = &buf[8+key_length..8+key_length+val_length];
                    let mut sha = Sha1::new();
                    let _ = sha.input(key);
                    let mut hash_key:[u8; 20] = [0; 20];
                    let _ = sha.result(&mut hash_key);
                    send.send(MessageType::FromClient(ClientMessage::Set(hash_key, val.to_owned())));
                }
                _ => ()
            }
        }
    })
}
