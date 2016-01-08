pub struct Config {
    pub network_port: u16,
    pub api_port: Option<u16>,
    pub k_val: usize,
    pub async_poll_interval: u32,
    pub initial_neighbors: Vec<String>
}

impl Config {
  //creates a Config with the network port set to port, all other fields are
  //default values
  pub fn default_with_port(port: u16) -> Config {
    Config {
        network_port: port,
        api_port: None,
        k_val: 8,
        async_poll_interval: 300,
        initial_neighbors: vec![]
    }
  }
}
