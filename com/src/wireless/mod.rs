use std::fmt::Debug;

use async_trait::async_trait;
use zbus::Connection;

use crate::{error::ComError, wireless::common::Security};

#[async_trait]
pub trait WifiAdapter: Debug {
    // async fn new(conn: Connection) -> Result<Self, ComError>
    // where
    // Self: Sized;
    async fn connect_network(
        &self,
        ssid: String,
        psk: String,
        security: Security,
    ) -> Result<(), ComError>;
    async fn disconnect(&self) -> Result<(), ComError>;
    async fn status(&self) -> Result<String, ComError>;
    async fn list_configured_networks(&self) -> Result<Vec<String>, ComError>;
    async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError>;
}

pub mod agent;
pub mod common;
pub mod defs;
pub mod iwd;
pub mod wpa_supplicant;
