use std::net::{UdpSocket, SocketAddr};
use std::io::Result;
use std::io::{Error, ErrorKind};
use std::mem::transmute;
use std::mem;

#[derive(Debug)]
pub enum Message <K, V>{
    //out
    Ping,
    Store(K, V),
    FindNode(K),
    FindVal(K),
    //acks
    PingResp,
    FindNodeResp(Vec<SocketAddr>, K),
    FindValResp(K, V)
}

pub trait ProtoMessage {
    fn id (&self) -> &Key;

    //yeah these can be done with partially applied functions instead... too late
    fn ping_msg (&self) -> [u8; 4 + 20] {
        let mut bytes: [u8; 24] = unsafe { mem::uninitialized()};
        for (x, y) in &mut bytes.iter_mut().zip([0; 4].iter().chain(self.id().iter())) {
            *x = *y;
        }
        bytes
    }

    fn ping_ack (&self) -> [u8; 5 + 20] {
        let mut bytes: [u8; 25] = unsafe {mem::uninitialized()};
        for (x, y) in &mut bytes.iter_mut().zip([0, 0, 0, 1, 4].iter().chain(self.id().iter())) {
            *x = *y;
        }
        bytes
    }

    fn store_msg (&self, key: &Key, val: &[u8]) -> Vec<u8> {
        let mut vec = Vec::with_capacity(key.len() + val.len() + 5);
        let bytes: [u8; 4] = unsafe { transmute(((1+ key.len() + val.len()) as u32).to_be()) };
        vec.extend(bytes.iter());
        vec.push(1);
        vec.extend(key.iter().chain(val.iter()));
        vec.extend(self.id().iter());
        vec
    }

    fn find_node_msg (&self, key: &Key) -> [u8; 20*2 + 5] {
        let mut ret:[u8; 45] = unsafe{mem::uninitialized()};
        let len:[u8; 4] = unsafe { transmute(((1 + key.len()) as u32).to_be())};

        //what the hell is this lol... maybe make this sane someday
        for (x, y) in &mut ret.iter_mut().zip(len.iter().chain([2].iter()).chain(key.iter()).chain(self.id().iter())) {
            *x = *y;
        }
        ret
    }

    fn find_node_resp (&self, closest: &Vec<Key>, key: &Key) -> Vec<u8> {
        let payload_size = mem::size_of::<Key>() * closest.len();
        let mut vec = Vec::with_capacity(payload_size + key.len() + 5 + 20);
        let bytes: [u8; 4] = unsafe {transmute((payload_size + 1) as u32 )};

        vec.extend(bytes.iter()
                        .chain([5].iter()));

        vec.extend(closest.iter().flat_map(|a| a.iter()).map(|b| *b));
        vec.extend(self.id().iter());
        vec
    }

    fn find_val_msg (&self, key: &[u8; 20]) -> [u8; 20*2 + 5] {
        let mut ret:[u8; 45] = unsafe{mem::uninitialized()};
        let len:[u8; 4] = unsafe { transmute(((1 + key.len()) as u32).to_be())};

        //what the hell is this
        for (x, y) in &mut ret.iter_mut().zip(len.iter().chain([3].iter()).chain(key.iter()).chain(self.id().iter())) {
            *x = *y;
        }

        ret
    }

    fn find_val_resp (&self, key: &[u8; 20], val: &[u8]) -> Vec<u8> {
        let pure_payload = key.len() + val.len();
        let mut vec:Vec<u8> = Vec::with_capacity(pure_payload + 5 + 20);
        let len:[u8; 4] = unsafe { transmute(((1 + key.len() + val.len()) as u32).to_be())};
        vec.extend(len.iter());
        vec.extend([6].iter());
        vec.extend(key.iter());
        vec.extend(val.iter());
        vec.extend(self.id().iter());
        vec
    }

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


//this is actually possible without any copies at all (even on the stack)
//for now do it this way
pub fn try_decode <'a> (bytes: &'a [u8], keysize: &usize) -> Option<(Message<Key, Value>, Key)> {
    let rest = &bytes[4..];

    println!("len: {}", bytes.len());
    let (some_msg, len) = match u8_4_to_u32(&bytes[0..4]) as usize {
        0 => (Message::Ping, 4),
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
                4 => Message::PingResp,
                5 => Message::PingResp, //TODO: not PingResp its FindNodeResp
                6 => Message::PingResp, //TODO: not PingResp its FindValResp
                _ => return None
            };
            //let from_id = (&bytes[len+4..len+4+keysize]);
            (message, len + 4)
        }
    };

    println!("l: {}", len);
    println!("x");
    let from_id = key_cpy(&bytes[len..len+keysize]);
    Some((some_msg, from_id))
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
    fn wait_for_message (&mut self) -> Result<(Message<Key, Value>, Key, SocketAddr)>;
}

use std::thread;

impl DSocket for UdpSocket {
    fn wait_for_message (&mut self) -> Result<(Message<Key, Value>, Key, SocketAddr)> {
        loop {
            let mut ibuf:[u8; 4096] = unsafe {mem::uninitialized()};
            match self.recv_from(&mut ibuf) {
                Ok((0, _)) => return Err(Error::new(ErrorKind::Other, "graceful disconnect")),
                Ok((num_read, addr)) => {
                    println!("{:?}", &ibuf[0..num_read]);
                    match try_decode(&ibuf[0..num_read], &KEYSIZE) {
                        None => continue,
                        Some((msg, from_id)) => return Ok((msg, from_id, addr))
                    }
                },
                Err(err) => return Err(err)
            }
        }
    }
}
