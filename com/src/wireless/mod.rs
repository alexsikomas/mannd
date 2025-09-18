use async_trait::async_trait;
use zbus::Connection;

use crate::error::NdError;

#[async_trait]
pub trait WifiAdapter {
    async fn new(conn: Connection) -> Result<Self, NdError>
    where
        Self: Sized;
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), NdError>;
    async fn disconnect(&self) -> Result<(), NdError>;
    async fn status(&self) -> Result<String, NdError>;
    async fn list_configured_networks(&self) -> Result<Vec<String>, NdError>;
    async fn add_network(&self, ssid: &str, psk: &str) -> Result<(), NdError>;
    async fn remove_network(&self, ssid: &str) -> Result<(), NdError>;
}

pub mod defs;
pub mod iwd;
pub mod wpa_supplicant;
