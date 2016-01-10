pub mod fmt;
pub mod networking;

use std::mem;

//this is relatively unsafe
pub fn u8_2_to_u16 (bytes: &[u8]) -> u16 {
    (bytes[1] as u16 | (bytes[0] as u16) << 8)
}

pub fn u8_4_to_u32 (bytes: &[u8]) -> u32 {
    (bytes[3] as u32
        | ((bytes[2] as u32) << 8)
        | ((bytes[1] as u32) << 16)
        | ((bytes[0] as u32) << 24))
}

pub fn u16_to_u8_2 (n: &u16) -> [u8; 2]{
    unsafe{mem::transmute((*n).to_be())}
}
