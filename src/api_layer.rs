//extern crate
use std::thread;
use std::thread::JoinHandle;
use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::net::{UdpSocket, SocketAddr};
use message_protocol::u8_4_to_u32;
use crypto::sha1::Sha1;
use crypto::digest::Digest;
use node::MessageType;
use std::collections::hash_map::Entry::{Occupied, Vacant};

#[derive(Debug)]
pub enum ClientMessage {
    Get([u8; 20]),
    Set([u8; 20], Vec<u8>)
}

pub enum Callback {
    Register([u8; 20], SocketAddr),
    //TODO: an Arc wrapper, RwLock, or just a large array might be more performant
    Resolve([u8; 20], Vec<u8>)
}

///Exposes Ailmedak to consumers (not nodes) who would like to access the core as a key value
///store. for now only UDP is used as transport
///TODO: perhaps it should not start the thread itself
pub fn spawn_api_thread (port: u16, send: Sender<MessageType>) -> (JoinHandle<()>, Sender<Callback>){
    let (tx, rx) = channel();
    let bind = UdpSocket::bind(("0.0.0.0", port)).unwrap();
    let tx_clone = tx.clone();

    let listener = bind.try_clone().unwrap();
    let request_thread = thread::spawn(move || {
        println!("binding client port: {}", port);
        loop {
            let mut buf:[u8; 4096] = [0; 4096];
            let (amt, src) = listener.recv_from(&mut buf).unwrap();
            match buf.first() {
                Some(&0) => { //this is a lookup type
                    let key_length = u8_4_to_u32(&buf[1..5]) as usize;
                    let key = &buf[5..5+key_length];
                    let mut sha = Sha1::new();
                    let _ = sha.input(key);
                    let mut hash_key:[u8; 20] = [0; 20];
                    let _ = sha.result(&mut hash_key);
                    let _ = tx.send(Callback::Register(hash_key.clone(), src));
                    println!("GETTING: {:?}", hash_key);
                    let _ = send.send(MessageType::FromClient(ClientMessage::Get(hash_key)));
                },
                Some(&1) => { //this is a store
                    let key_length = u8_4_to_u32(&buf[1..5]) as usize;
                    let key = &buf[5..5+key_length];
                    let val_length = u8_4_to_u32(&buf[5+key_length..9+key_length]) as usize;

                    let val = &buf[9+key_length..9+key_length+val_length];
                    let mut sha = Sha1::new();
                    let _ = sha.input(key);
                    let mut hash_key:[u8; 20] = [0; 20];
                    let _ = sha.result(&mut hash_key);
                    println!("SETTING: {:?}", hash_key);
                    let _ = send.send(MessageType::FromClient(ClientMessage::Set(hash_key, val.to_owned())));
                }
                _ => ()
            };
        }
    });

    //there are a lot of threads going on. we could just do stuff from the state thread but at
    //least there's modularity this way
    let response_socket = bind.try_clone().unwrap();
    let outbound_thread = thread::spawn(move || {
        let mut req_map:HashMap<[u8; 20], Vec<SocketAddr>> = HashMap::new();
        loop {
            match rx.recv().unwrap() {
                Callback::Register(key, src) =>  {
                    let res = match req_map.entry(key) {
                        Vacant(entry) => entry.insert(Vec::new()),
                        Occupied(entry) => entry.into_mut()
                    };
                    res.push(src);
                    //storing
                    println!("{:?} from {:?}", &key, src);
                },
                Callback::Resolve(key, val) => {
                    if let Some(vec) = req_map.remove(&key) {
                        println!("g");
                        for addr in vec {
                            println!("found");
                            response_socket.send_to(&val, addr);
                        }
                    }

                }
            }
        }

        println!("state: {:?}", req_map);

    });

    (request_thread, tx_clone)
}
