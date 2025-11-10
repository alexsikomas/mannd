//! Reference: https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network
use async_trait::async_trait;

use crate::{
    error::ComError,
    wireless::{common::Security, WifiAdapter},
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {}

#[async_trait]
impl WifiAdapter for WpaSupplicant {
    async fn connect_network(
        &self,
        ssid: String,
        psk: String,
        security: Security,
    ) -> Result<(), ComError> {
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
    async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError> {
        todo!()
    }
}

impl WpaSupplicant {
    pub fn new() -> Result<Self, ComError> {
        Ok(Self {})
    }
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wpa_scan() -> Result<(), ComError> {
        // let conn = Connection::system().await.unwrap();
        let wpa = WpaSupplicant::new()?;
        // wpa.scan().await?;
        Ok(())
    }
}
