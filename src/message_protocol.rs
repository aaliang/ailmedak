use std::net::{UdpSocket, SocketAddr};
use std::io::Result;
use std::io::{Error, ErrorKind};
use std::mem::transmute;
use std::mem;
use utils::{u8_2_to_u16, u8_4_to_u32};

#[derive(Debug)]
pub enum Message <K, V>{
    //out
    Ping,
    Store(K, V),
    FindNode(K),
    FindVal(K),
    //acks
    PingResp,
    FindNodeResp(K, Vec<(K, [u8; 4], u16)>),
    FindValResp(K, V)
}

/*macro_rules! byte_envelope {
    (
        $len:expr,
        $(
            $x:expr,
         ),*
    ) => {{
        let mut bytes: [u8; $len] = unsafe {mem::uninitialized()};
        for (x, y) in &mut bytes.iter_mut().zip(rec_env!($($x),*)) {
            *x = *y;
        }
        //[$($x),*]
        bytes
    }};
}

macro_rules! rec_env {
    ($head:expr,) => ();
    (
        $head:expr, $($x:expr),*
    ) => {{
        $head.iter().chain(byte_envelope!($($x),*))
    }};
}*/

pub trait ProtoMessage {
    fn id (&self) -> &Key;

    //yeah these can be done with partially applied functions instead... too late
    fn ping_msg (&self) -> [u8; 1 + 20] {
        let mut bytes: [u8; 21] = unsafe { mem::uninitialized()};
        //[0, + id] (21 bytes)
        for (x, y) in &mut bytes.iter_mut().zip(
            [0].iter().chain(self.id().iter())) {
            *x = *y;
        }
        bytes
    }

    fn ping_ack (&self) -> [u8; 1 + 20] {
        let mut bytes: [u8; 21] = unsafe {mem::uninitialized()};
        for (x, y) in &mut bytes.iter_mut().zip(
            [1].iter().chain(self.id().iter())) {
            *x = *y;
        }
        bytes
    }

    fn store_msg (&self, key: &Key, val: &[u8]) -> Vec<u8> {
        let mut vec = Vec::with_capacity(key.len() + val.len() + 5);
        let payload_size: [u8; 4] = unsafe{transmute(((key.len() + val.len()) as u32).to_be())};
        vec.extend([2].iter().chain(self.id().iter())
                      .chain(payload_size.iter())
                      .chain(key.iter())
                      .chain(val.iter()));
        vec
    }

    fn find_node_msg (&self, key: &Key) -> [u8; 1+20+4+20] {
        let mut ret:[u8; 45] = unsafe { mem::uninitialized()};
        let len:[u8; 4] = unsafe { transmute(((key.len()) as u32).to_be())};

        //what the hell is this lol... maybe make this sane someday
        for (x, y) in &mut ret.iter_mut().zip([3].iter().chain(self.id().iter())
                                                        .chain(len.iter())
                                                        .chain(key.iter())) {
            *x = *y;
        }
        ret
    }

    fn find_val_msg (&self, key: &[u8; 20]) -> [u8; 1+20+4+20] {
        let mut ret:[u8; 45] = unsafe { mem::uninitialized()};
        let len:[u8; 4] = unsafe { transmute(((key.len()) as u32).to_be())};
        //what the hell is this lol... maybe make this sane someday
        for (x, y) in &mut ret.iter_mut().zip([4].iter().chain(self.id().iter())
                                                        .chain(len.iter())
                                                        .chain(key.iter())) {
            *x = *y;
        }
        ret
    }

    fn find_node_resp (&self, closest: &Vec<(Key, (Key, ([u8; 4], [u8; 2])))>, key: &Key) -> Vec<u8> {
        let payload_size = (mem::size_of::<Key>() + 6) * closest.len();
        println!("psize {}", payload_size);
        println!("klen: {}", key.len());
        let mut vec = Vec::with_capacity(payload_size + key.len() + 5 + 20);
        let bytes: [u8; 4] = unsafe {transmute(((payload_size + key.len()) as u32 ).to_be())};
        vec.extend(
            [5].iter().chain(self.id().iter())
                      .chain(bytes.iter())
                      .chain(key.iter()));
        vec.extend(closest.iter().flat_map(|&(_, (ref a, (ref b, ref c)))| {
            a.iter().chain(b.iter()).chain(c.iter())
        }).map(|b| *b));
        vec
    }

    fn find_val_resp (&self, key: &[u8; 20], val: &[u8]) -> Vec<u8> {
        let pure_payload = key.len() + val.len();
        let mut vec:Vec<u8> = Vec::with_capacity(pure_payload + 5 + 20);
        let len:[u8; 4] = unsafe { transmute(((key.len() + val.len()) as u32).to_be())};

        vec.extend(
            [6].iter().chain(self.id().iter())
                      .chain(len.iter())
                      .chain(key.iter())
                      .chain(val.iter()));
        vec
    }

}


const KEYSIZE:usize = 20;

pub type Key = [u8; 20];
pub type Value = Vec<u8>;

fn key_cpy (key_addr: &[u8]) -> Key {
    let mut key: Key = [0; 20]; //for now hardcode... later use macros
    for (a, b) in key_addr.iter().zip(key.iter_mut()) {
        *b = *a;
    }
    key
}

fn ip_cpy (key_addr: &[u8]) -> [u8; 4] {
    let mut key:[u8; 4] = [0; 4]; //for now hardcode... later use macros
    for (a, b) in key_addr.iter().zip(key.iter_mut()) {
        *b = *a;
    }
    key
}

//this is actually possible without any copies at all (even on the stack)
//for now do it this way
pub fn try_decode <'a> (bytes: &'a [u8], keysize: &usize) -> Option<(Message<Key, Value>, Key)> {
    let node_id = key_cpy(&bytes[1..keysize+1]);
    let some_msg = match *(bytes.first().unwrap()) {
        0 => Message::Ping,
        1 => Message::PingResp,
        x => {
            let len = u8_4_to_u32(&bytes[keysize+1..keysize+5]) as usize;
            let rest = &bytes[keysize+5..];
            match x {
                2 => Message::Store(key_cpy(&rest[0..*keysize]), (&rest[*keysize..]).to_owned()),
                3 => Message::FindNode(key_cpy(&rest[0..])),
                4 => Message::FindVal(key_cpy(&rest[0..])),
                5 => { 
                    let key = key_cpy(&rest[0..*keysize]);
                    let nfield = &rest[*keysize..];
                    let num_returned = (len-keysize)/keysize;
                    let result_vec = (0..num_returned).map(|n| {
                        let section = &nfield[n * (*keysize + 6)..];
                        let node_id = key_cpy(&section[0..*keysize]);
                        let ip_addr = ip_cpy(&section[*keysize..*keysize+4]);
                        let port = u8_2_to_u16(&section[*keysize+4..*keysize+6]);
                        (node_id, ip_addr, port)
                    }).collect::<Vec<([u8; 20], [u8; 4], u16)>>();
                    Message::FindNodeResp(key, result_vec)
                },
                6 => Message::FindValResp(key_cpy(&rest[0..*keysize]), (&rest[*keysize..]).to_owned()),
                _ => return None
            }
        }
    };

    Some((some_msg, node_id))
}

pub trait DSocket {
    fn wait_for_message (&mut self) -> Result<(Message<Key, Value>, Key, SocketAddr)>;
}

impl DSocket for UdpSocket {
    fn wait_for_message (&mut self) -> Result<(Message<Key, Value>, Key, SocketAddr)> {
        loop {
            let mut ibuf:[u8; 4096] = unsafe {mem::uninitialized()};
            match self.recv_from(&mut ibuf) {
                Ok((0, _)) => return Err(Error::new(ErrorKind::Other, "graceful disconnect")),
                Ok((num_read, addr)) => {
                    //println!("{:?}", &ibuf[0..num_read]);
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
