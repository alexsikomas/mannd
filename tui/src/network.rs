use com::{
    controller::Controller,
    netlink::WirelessNetlink,
    wireless::{iwd::Iwd, wpa_supplicant::WpaSupplicant},
};
use tracing::info;

pub struct NetworkState {
    connected: bool,
    ssid: Option<String>,
    signal: Option<i8>,
    // Info in networks will be displayed on the tui
    networks: Vec<String>,
    pub controller: Controller,
}

pub enum Backend {
    Iwd(Iwd),
    Wpa(WpaSupplicant),
    Netlink(WirelessNetlink),
}

impl NetworkState {
    pub async fn new() -> Self {
        Self {
            connected: false,
            ssid: None,
            signal: None,
            networks: vec![],
            controller: Controller::new().await.expect("Could not init controller"),
            // since netlink will always be available it;s the default
        }
    }

    /// Sets the adapters for the controller, tries to set all adapters that are installed
    pub async fn connect_wifi_adapter(&mut self) {
        self.controller.determine_adapter().await;
    }
}
