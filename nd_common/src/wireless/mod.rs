use crate::error::NdError;

pub trait WifiAdapter {
    fn connect_network(
        &self,
        ssid: &str,
        psk: &str,
    ) -> impl std::future::Future<Output = Result<(), NdError>> + Send;
    fn disconnect(&self) -> impl std::future::Future<Output = Result<(), NdError>> + Send;
    fn status(&self) -> impl std::future::Future<Output = Result<String, NdError>> + Send;
    fn list_configured_networks(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<String>, NdError>> + Send;
    fn add_network(
        &self,
        ssid: &str,
        psk: &str,
    ) -> impl std::future::Future<Output = Result<(), NdError>> + Send;
    fn remove_network(
        &self,
        ssid: &str,
    ) -> impl std::future::Future<Output = Result<(), NdError>> + Send;
}

pub mod defs;
pub mod iwd;
pub mod wpa_supplicant;
