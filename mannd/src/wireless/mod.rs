use tokio::sync::mpsc::Sender;

use crate::{error::ManndError, state::signals::SignalUpdate, store::NetworkInfo};

pub mod agent;
pub mod common;
pub mod iwd;
pub mod wifi_config;
pub mod wpa_supplicant;

#[allow(async_fn_in_trait)]
pub trait WifiBackend {
    async fn scan_networks(&self, signal_tx: Sender<SignalUpdate<'_>>) -> Result<(), ManndError>;
    async fn get_networks(&self) -> Result<Vec<NetworkInfo>, ManndError>;
    async fn connect_network(&self, network: &NetworkInfo) -> Result<(), ManndError>;
    async fn disconnect(&self) -> Result<(), ManndError>;
    async fn forget_network(&self, network: &NetworkInfo) -> Result<(), ManndError>;
}
