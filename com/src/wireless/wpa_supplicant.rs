use async_trait::async_trait;
use zbus::Connection;

use crate::{error::NdError, wireless::WifiAdapter};

pub struct WpaSupplicant {}

#[async_trait]
impl WifiAdapter for WpaSupplicant {
    async fn new(conn: Connection) -> Result<Self, NdError> {
        todo!()
    }
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), NdError> {
        todo!()
    }
    async fn disconnect(&self) -> Result<(), NdError> {
        todo!()
    }
    async fn status(&self) -> Result<String, NdError> {
        todo!()
    }
    async fn list_configured_networks(&self) -> Result<Vec<String>, NdError> {
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
