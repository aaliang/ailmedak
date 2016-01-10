use std::net::{SocketAddr, Ipv4Addr};
use utils::{u16_to_u8_2};

/// converts a socket address to a tuple of an ip, port represented as bytes
pub fn ip_port_pair_bytes (s_addr: SocketAddr) -> ([u8; 4], [u8; 2]) {
    match s_addr {
        SocketAddr::V4(s) => (s.ip().octets(), u16_to_u8_2(&s.port())),
        _ => panic!("IPV6 NOT CURRENTLY SUPPORTED")
    }
}

/// an Ipv4Addr, port pair
pub fn ip_port_pair(ip: &[u8; 4], port: &u16) -> (Ipv4Addr, u16) {
    (Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), *port)
}
