extern crate time;

use utils::fmt::as_hex_string;
use std::fmt::Debug;

const ID_TRUNC:usize = 3;

pub struct Loggerator {
    id: String,
    id_full: String
}

impl Loggerator {
    pub fn new (id: &[u8]) -> Loggerator {
        Loggerator {
            id: as_hex_string(&id[..ID_TRUNC]),
            id_full: as_hex_string(id)
        }
    }

    pub fn log <T:Debug> (&self, message: &T) {
        println!("[{}@{}] {:?}", self.id, Self::now_str(), message);
    }

    pub fn logs <T: Debug> (&self, message: &T, source_id_bytes: &[u8]) {
        println!("[{}@{}] |{:?}| <- {}", self.id, Self::now_str(), message, as_hex_string(&source_id_bytes[..ID_TRUNC]));
    }

    pub fn log_with_full_id <T:Debug> (&self, message: &T) {
        println!("[{}] {:?}", self.id_full, message);
    }

    fn now_str () -> String {
        let tm = time::now_utc();
        time::strftime("%H:%M:%S.%f", &tm).unwrap()
    }

}
