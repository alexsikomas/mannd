pub enum CurrentMenu {
    Select,
    Connection,
    Vpn,
    Config,
}

impl Default for CurrentMenu {
    fn default() -> Self {
        Self::Select
    }
}
