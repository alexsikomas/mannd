use crate::{error::NdError, wireless::WifiAdapter};

pub struct WpaSupplicant {}

impl WifiAdapter for WpaSupplicant {
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), NdError> {
        todo!()
    }
    async fn disconnect(&self) {
        todo!()
    }
    async fn status(&self) -> String {
        todo!()
    }
    async fn list_configured_networks(&self) -> Vec<String> {
        todo!()
    }
    async fn add_network(&self, ssid: &str, psk: &str) -> Result<(), NdError> {
        todo!()
    }
    async fn remove_network(&self, ssid: &str) -> Result<(), NdError> {
        todo!()
    }
}

impl WpaSupplicant {
    async fn new() -> Result<Self, NdError> {
        todo!()
    }
}
