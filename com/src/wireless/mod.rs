use async_trait::async_trait;
use zbus::Connection;

use crate::error::ComError;

#[async_trait]
pub trait WifiAdapter {
    async fn new(conn: Connection) -> Result<Self, ComError>
    where
        Self: Sized;
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), ComError>;
    async fn disconnect(&self) -> Result<(), ComError>;
    async fn status(&self) -> Result<String, ComError>;
    async fn list_configured_networks(&self) -> Result<Vec<String>, ComError>;
    async fn add_network(&self, ssid: &str, psk: &str) -> Result<(), ComError>;
    async fn remove_network(&self, ssid: &str) -> Result<(), ComError>;
}

pub mod common;
pub mod defs;
pub mod iwd;
pub mod wpa_supplicant;
