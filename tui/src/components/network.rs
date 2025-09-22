struct NetworkState {
    connected: bool,
    ssid: Option<String>,
    signal: Option<i8>,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            connected: false,
            ssid: None,
            signal: None,
        }
    }
}
