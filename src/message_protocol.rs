use std::net::UdpSocket;
use std::io::Result;
use std::io::{Error, ErrorKind};

pub enum Message <K, V>{
    Ping,
    Store(K, V),
    FindNode(K),
    FindVal(K)
}

const KEYSIZE:usize = 20;
const MIN_MSG_SIZE:usize = 4;

pub type Key = Vec<u8>;
pub type Value = Vec<u8>;

pub fn try_decode (bytes: &[u8], keysize: &usize) -> Option<(Message<Key, Value>, usize)> {
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
                    let key = &rest[1..*keysize+1];
                    let value = &rest[*keysize+1..len];
                    Message::Store(key.to_owned(), value.to_owned())
                },
                2 => Message::FindNode((&rest[1..len]).to_owned()),
                3 => Message::FindVal((&rest[1..len]).to_owned()),
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

pub struct BufferedUdp {
    socket: UdpSocket,
    buffer: Vec<u8>
}

impl BufferedUdp {
    pub fn new (socket: UdpSocket) -> BufferedUdp {
        BufferedUdp {
            socket: socket,
            buffer: Vec::new()
        }
    }
}

pub trait DSocket {
    fn wait_for_message (&mut self) -> Result<Message<Key, Value>>;
}

impl DSocket for BufferedUdp {
    fn wait_for_message (&mut self) -> Result<Message<Key, Value>> {
        //eventually try to go heapless unless last resort.
        loop {
            let mut ibuf:[u8; 256] = [0; 256];
            match self.socket.recv_from(&mut ibuf) {
                Ok((0, _)) => {
                    return Err(Error::new(ErrorKind::Other, "graceful disconnect"))
                }
                Ok((num_read, addr)) => {
                    self.buffer.extend(ibuf[0..num_read].iter());
                    if self.buffer.len() >= MIN_MSG_SIZE {
                        match try_decode(&self.buffer, &KEYSIZE) {
                            None => continue,
                            Some((msg, bytes_consumed)) => {
                                //TOOD: make this zero copy
                                self.buffer = (&self.buffer[bytes_consumed..]).to_owned();
                                return Ok(msg)
                            }
                        };
                    }
                },
                Err(err) => return Err(err)
            };
         }
    }
}
