use std::path::PathBuf;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{
    controller::WifiDaemonType,
    store::WgMeta,
    wireless::{
        common::{AccessPoint, Security},
        wpa_supplicant::WpaInterface,
    },
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NetworkAction {
    GetCapabilities,
    Wifi(WifiAction),
    Wireguard(WireguardAction),
    Wpa(WpaAction),
    Exit,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum WifiAction {
    Scan,
    GetNetworks,
    Connect(ApConnectInfo),
    ConnectKnown(String, Security),
    Disconnect,
    Forget(String, Security),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum WireguardAction {
    Toggle,
    GetInfo,
    Connect(PathBuf),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum WpaAction {
    GetInterfaces,
    CreateInterface(String),
    TogglePersist,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NetworkState {
    SetCapabilities(Capability),

    // wifi
    SetNetworks(Vec<AccessPoint>),
    SetInterfaces(Vec<String>),

    // wpa
    SetWpaInterfaces(Vec<WpaInterface>),
    ToggleWpaPesist,

    // wireguard
    SetWireguardInfo((Vec<String>, Vec<WgMeta>)),

    // FUTURE: NETWORKD
    // SetNetworkdFiles(Vec<PathBuf>),
    Start(Started),
    Success(Success),
    Failed(Failure),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Started(pub Process);

#[derive(Debug, Serialize, Deserialize)]
pub enum Success {
    Generic,
    EnableWireguard,
    DisableWireguard,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Process {
    WifiConnect,
    WifiScan,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Failure {
    pub process: Process,
    pub reason: String,
}

impl Failure {
    pub fn new(process: Process, reason: impl Into<String>) -> Self {
        Self {
            process,
            reason: reason.into(),
        }
    }
}

// determines what options will be visible/selectable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub wifi_daemon: Option<WifiDaemonType>,
    pub networkd_active: bool,
    // installed, wg-mannd interface active
    pub wireguard: (bool, bool),
}

impl Capability {
    pub fn new(
        wifi_daemon: Option<WifiDaemonType>,
        networkd_active: bool,
        wireguard: (bool, bool),
    ) -> Self {
        Capability {
            wifi_daemon,
            networkd_active,
            wireguard,
        }
    }
}

impl Default for Capability {
    fn default() -> Self {
        Capability {
            wifi_daemon: None,
            networkd_active: false,
            wireguard: (false, false),
        }
    }
}

/// For connecting to APs used by wpa and iwd
#[derive(Builder, Debug, Serialize, Deserialize, Clone)]
pub struct ApConnectInfo {
    pub ssid: String,
    pub security: Security,
    pub credentials: Credentials,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Credentials {
    Password(String),
    // Eap(EapInfo),
}
