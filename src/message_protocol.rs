use std::net::UdpSocket;
use std::net::SocketAddr;
//use std::net::SocketAddrV4;
use std::io::Result;
use std::io::{Error, ErrorKind};
use std::mem::transmute;
use std::mem;

#[derive(Debug)]
pub enum Message <K, V>{
    Ping,
    Store(K, V),
    FindNode(K),
    FindVal(K)
}

pub fn ping_msg () -> [u8; 4] {
    [0; 4]
}

pub fn store_msg (key: &[u8; 20], val: &[u8]) -> Vec<u8> {
    let mut vec = Vec::with_capacity(key.len() + val.len() + 5);
    let bytes: [u8; 4] = unsafe { transmute(((1+ key.len() + val.len()) as u32).to_be()) };
    vec.extend(bytes.iter());
    vec.push(1);
    vec.extend(key.iter().chain(val.iter()));
    vec
}

pub fn find_node_msg (key: &[u8; 20]) -> [u8; 20 + 5] {
    let mut ret:[u8; 25] = unsafe{mem::uninitialized()};
    let len:[u8; 4] = unsafe { transmute(((1 + key.len()) as u32).to_be())};

    //what the hell is this lol... maybe make this sane someday
    for (x, y) in &mut ret.iter_mut().zip(len.iter().chain([2].iter()).chain(key.iter())) {
        *x = *y;
    }

    ret
}

pub fn find_val_msg (key: &[u8; 20]) -> [u8; 20 + 5] {
    let mut ret:[u8; 25] = unsafe{mem::uninitialized()};
    let len:[u8; 4] = unsafe { transmute(((1 + key.len()) as u32).to_be())};

    //what the hell is this
    for (x, y) in &mut ret.iter_mut().zip(len.iter().chain([3].iter()).chain(key.iter())) {
        *x = *y;
    }

    ret
}

const KEYSIZE:usize = 20;
const MIN_MSG_SIZE:usize = 4;

pub type Key = [u8; 20];
pub type Value = Vec<u8>;

fn key_cpy (key_addr: &[u8]) -> Key {
    let mut key: Key = [0; 20]; //for now hardcode... later use macros
    for (a, b) in key_addr.iter().zip(key.iter_mut()) {
        *b = *a;
    }
    key
}

pub fn try_decode <'a> (bytes: &'a [u8], keysize: &usize) -> Option<(Message<Key, Value>, usize)> {
    let rest = &bytes[4..];
    match u8_4_to_u32(&bytes[0..4]) as usize {
        0 => Some((Message::Ping, 4)),
        len => { //len is inclusive of the id byte
            let message_type = match rest.first() {
                None => return None,
                Some(a) => a
            };
            let message = match *message_type {
                _ if len > rest.len() => return None, //the entire envelope is not here yet
                1 => {
                    let key_addr = &rest[1..*keysize+1];
                    let key = key_cpy(key_addr);
                    let value = &rest[*keysize+1..len];
                    Message::Store(key, value.to_owned())
                },
                2 => Message::FindNode(key_cpy(&rest[1..len])),
                3 => Message::FindVal(key_cpy(&rest[1..len])),
                _ => return None
            };

            Some((message, len + 4))
        }
    }
}

//this is relatively unsafe
fn u8_2_to_u16 (bytes: &[u8]) -> u16 {
    (bytes[1] as u16 | (bytes[0] as u16) << 8)
}

fn u8_4_to_u32 (bytes: &[u8]) -> u32 {
    (bytes[3] as u32
        | ((bytes[2] as u32) << 8)
        | ((bytes[1] as u32) << 16)
        | ((bytes[0] as u32) << 24))
}
pub trait DSocket {
    fn wait_for_message (&mut self) -> Result<(Message<Key, Value>, SocketAddr)>;
}

use std::thread;

impl DSocket for UdpSocket {
    fn wait_for_message (&mut self) -> Result<(Message<Key, Value>, SocketAddr)> {
        loop {
            let mut ibuf:[u8; 4096] = unsafe {mem::uninitialized()};
            match self.recv_from(&mut ibuf) {
                Ok((0, _)) => return Err(Error::new(ErrorKind::Other, "graceful disconnect")),
                Ok((num_read, addr)) => {
                    println!("{:?}", &ibuf[0..num_read]);
                    match try_decode(&ibuf[0..num_read], &KEYSIZE) {
                        None => continue,
                        Some((msg, _)) => return Ok((msg, addr))
                    }
                },
                Err(err) => return Err(err)
            }
        }
    }
}
