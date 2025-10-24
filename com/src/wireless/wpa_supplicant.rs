use async_trait::async_trait;
use zbus::Connection;

use crate::{
    error::ComError,
    wireless::{common::Security, WifiAdapter},
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {}

#[async_trait]
impl WifiAdapter for WpaSupplicant {
    // async fn new(conn: Connection) -> Result<Self, ComError> {
    //     todo!()
    // }
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
    pub async fn new(conn: Connection) -> Result<Self, ComError> {
        todo!()
    }
}
