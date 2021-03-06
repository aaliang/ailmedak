extern crate ailmedak;
use std::env;
use std::net::UdpSocket;
use std::mem;
use std::thread;

/// Basic cmd line tool to get and/or set from an ailmedak cluster
///
/// usage:
/// $ ./client get <key> <entry address> <local_port>
/// $ ./client set <key> <val> <entry address> <local_port>
///
/// ex:
/// $ ./client get hello 127.0.0.1:5000 5999

fn main () {
    let method = env::args().nth(1).unwrap();
    //hastily written byte manipulations
    match method.as_ref() {
        "get" => {
            let key = env::args().nth(2).unwrap();
            let addr = env::args().nth(3).unwrap();
            let binding = format!("0.0.0.0:{}", env::args().nth(4).unwrap().parse::<u16>().unwrap());
            let local_binding:&str = binding.as_ref();
            let sock = UdpSocket::bind(local_binding).unwrap();
            let rec_sock = sock.try_clone().unwrap();

            let handle = thread::spawn(move || {
                loop {
                    let mut buf = [0; 60000];
                    let (bytes_read, _) = rec_sock.recv_from(&mut buf).unwrap();
                    println!("{:?}", &buf[..bytes_read]);
                }
            });

            let mut msg = vec![0];
            let key_as_bytes = key.into_bytes();
            let len_as_bytes:[u8; 4] = unsafe{ mem::transmute((key_as_bytes.len() as u32).to_be())};

            msg.extend(len_as_bytes.iter().chain(key_as_bytes.iter()));
            let addr_ref:&str = addr.as_ref();
            let _ = sock.send_to(&msg, addr_ref);

            let _ = handle.join();
        },
        "set" => {
            let key = env::args().nth(2).unwrap();
            println!("key is: {}", key);
            let val = env::args().nth(3).unwrap();
            println!("val is: {}", val);
            let addr = env::args().nth(4).unwrap();
            let binding = format!("0.0.0.0:{}", env::args().nth(5).unwrap().parse::<u16>().unwrap());
            let local_binding:&str = binding.as_ref();
            let sock = UdpSocket::bind(local_binding).unwrap();

            let mut msg = vec![1];
            let key_as_bytes = key.into_bytes();
            println!("kab: {:?}", key_as_bytes);
            let val_as_bytes = val.into_bytes();
            let key_len_as_bytes:[u8; 4] = unsafe {mem::transmute((key_as_bytes.len() as u32).to_be())};

            println!("klb: {:?}", key_len_as_bytes);
            let val_len_as_bytes:[u8; 4] = unsafe {mem::transmute((val_as_bytes.len() as u32).to_be())};
            println!("vlb: {:?}", val_len_as_bytes);

            msg.extend(
                key_len_as_bytes.iter().chain(key_as_bytes.iter())
                                       .chain(val_len_as_bytes.iter())
                                       .chain(val_as_bytes.iter()));

            let addr_ref:&str = addr.as_ref();
            let _ = sock.send_to(&msg, addr_ref);
        },
        _ => {
            println!("invalid usage");
        }
    }
}
