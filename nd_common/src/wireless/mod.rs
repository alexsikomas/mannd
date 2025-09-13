use crate::error::NdError;

pub trait WifiAdapter {
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), NdError>;
    async fn disconnect(&self);
    async fn status(&self) -> String;
    async fn list_configured_networks(&self) -> Vec<String>;
    async fn add_network(&self, ssid: &str, psk: &str) -> Result<(), NdError>;
    async fn remove_network(&self, ssid: &str) -> Result<(), NdError>;
}

pub mod defs;
pub mod iwd;
pub mod netlink;
pub mod wpa_supplicant;
