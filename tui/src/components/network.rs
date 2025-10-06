struct NetworkState {
    connected: bool,
    ssid: Option<String>,
    signal: Option<i8>,
    networks: Vec<String>,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            connected: false,
            ssid: None,
            signal: None,
            networks: vec![],
        }
    }
}

impl NetworkState {
    fn scan() {}
}
