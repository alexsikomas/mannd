use async_trait::async_trait;
use zbus::Connection;

use crate::{error::ComError, wireless::WifiAdapter};

#[derive(Debug)]
pub struct WpaSupplicant {}

#[async_trait]
impl WifiAdapter for WpaSupplicant {
    async fn new(conn: Connection) -> Result<Self, ComError> {
        todo!()
    }
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), ComError> {
        todo!()
    }
    async fn disconnect(&self) -> Result<(), ComError> {
        todo!()
    }
    async fn status(&self) -> Result<String, ComError> {
        todo!()
    }
    async fn list_configured_networks(&self) -> Result<Vec<String>, ComError> {
        todo!()
    }
    async fn add_network(&self, ssid: &str, psk: &str) -> Result<(), ComError> {
        todo!()
    }
    async fn remove_network(&self, ssid: &str) -> Result<(), ComError> {
        todo!()
    }
}

impl WpaSupplicant {
    async fn new() -> Result<Self, ComError> {
        todo!()
    }
}
