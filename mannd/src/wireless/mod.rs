use zbus::proxy::SignalStream;

use crate::{error::ManndError, store::NetworkInfo};

pub mod agent;
pub mod common;
pub mod iwd;
pub mod wifi_config;
pub mod wpa_supplicant;

#[allow(async_fn_in_trait)]
pub trait WifiBackend {
    /// Starts a scan, returning the signal that fires once it is done or
    /// `None` if one was already running
    async fn scan_networks(&self) -> Result<Option<SignalStream<'static>>, ManndError>;
    async fn get_networks(&self) -> Result<Vec<NetworkInfo>, ManndError>;
    async fn connect_network(&self, network: &NetworkInfo) -> Result<(), ManndError>;
    async fn disconnect(&self) -> Result<(), ManndError>;
    async fn forget_network(&self, network: &NetworkInfo) -> Result<(), ManndError>;
}
