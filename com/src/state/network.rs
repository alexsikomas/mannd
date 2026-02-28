use std::{path::PathBuf, process::Command};

use bitflags::bitflags;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tracing::info;

use crate::{
    controller::{Controller, DaemonType, WirelessAdapter},
    error::ManndError,
    state::signals::{SignalManager, SignalUpdate},
    systemd::networkd::get_netd_files,
    utils::list_interfaces,
    wireguard::{network::Wireguard, store::WgMeta},
    wireless::{
        common::{AccessPoint, Security},
        wpa_supplicant::WpaInterface,
    },
};

pub struct NetworkActor<'a> {
    pub controller: Controller,
    pub signal_manager: SignalManager<'a>,
    signal_tx: Sender<SignalUpdate<'a>>,
    sock_tx: Sender<NetworkState>,
}

impl<'a> NetworkActor<'a> {
    pub async fn new(
        signal_tx: Sender<SignalUpdate<'a>>,
        sock_tx: Sender<NetworkState>,
    ) -> Result<Self, ManndError> {
        let mut controller = Controller::new().await?;
        controller.determine_adapter().await;
        let signal_manager = SignalManager::new();
        Ok(Self {
            controller,
            signal_manager,
            signal_tx,
            sock_tx,
        })
    }

    /// Returns true if we are quitting the application
    pub async fn handle_action(&mut self, action: NetworkAction) -> Result<bool, ManndError> {
        // check if wifi then allow wifi requests
        let mut state_send: Vec<NetworkState> = vec![];
        if self.controller.wifi.is_some() {
            self.handle_wifi_action(action.clone(), &mut state_send)
                .await;
        };

        // Wi-Fi not needed
        match action {
            NetworkAction::GetCapabilities => {
                let wifi_daemon = self.controller.daemon_type();
                let networkd_active = self.controller.networkd_status().await;
                let wg_installed = Command::new("wg")
                    .arg("--version")
                    .output()
                    .map_or(false, |_| true);

                let wg_iface = Wireguard::check_state().await?;

                let caps = Capability::new(wifi_daemon, networkd_active, (wg_installed, wg_iface));
                state_send.push(NetworkState::SetCapabilities(caps));
            }
            // WIREGUARD
            NetworkAction::ConnectWireguard(file) => match self.controller.connect_wg(file).await {
                Ok(res) => {
                    info!("Success");
                }
                Err(e) => {
                    tracing::error!("{:?}", e)
                }
            },
            NetworkAction::InitWireguard => {
                if let Ok(()) = self.controller.start_wg().await {
                    self.handle_net_ctx(NetCtxFlags::Wireguard, &mut state_send)
                        .await?;
                }
            }
            NetworkAction::GetNetworkContext(flags) => {
                self.handle_net_ctx(flags, &mut state_send).await?;
            }
            NetworkAction::Exit => {
                if let Ok(()) = self.controller.exit().await {
                    return Ok(true);
                }
            }
            _ => {}
        };

        for req in state_send {
            let _ = self.sock_tx.send(req).await;
        }

        Ok(false)
    }

    async fn handle_wifi_action(
        &mut self,
        action: NetworkAction,
        state_send: &mut Vec<NetworkState>,
    ) {
        // a lot of times we want to update network list
        // after an action
        let mut should_refresh = false;

        match action {
            NetworkAction::Scan => {
                state_send.push(NetworkState::Start(NetStart::Scan));
                let _ = self.controller.scan(self.signal_tx.clone()).await;
            }
            NetworkAction::Connect(info) => {
                state_send.push(NetworkState::Start(NetStart::Wifi));

                match self.controller.network_connect(info).await {
                    Ok(()) => {
                        info!("Connection to network was successful");
                        state_send.push(NetworkState::Success(NetSuccess::Wifi));
                    }
                    Err(e) => {
                        tracing::error!("Connection to network was not successful.");
                        state_send.push(NetworkState::Failed(NetFailure::Wifi(e.to_string())));
                    }
                }
            }
            NetworkAction::ConnectKnown(ssid, security) => {
                match self.controller.connect_known(ssid, security).await {
                    Ok(()) => {
                        state_send.push(NetworkState::CallAction(
                            NetworkAction::GetNetworkContext(NetCtxFlags::Network),
                        ));
                        should_refresh = true;
                    }
                    Err(e) => {}
                }
            }
            NetworkAction::CreateWpaInterface(ifname) => {
                if let Some(WirelessAdapter::Wpa(wpa)) = self.controller.wifi.as_mut() {
                    let _ = wpa.create_interface(ifname).await;
                }
            }
            NetworkAction::Disconnect => {
                if let Ok(()) = self.controller.disconenct().await {
                    info!("Disconnected from a network");
                    should_refresh = true;
                } else {
                }
            }
            NetworkAction::Forget(ssid, sec) => {
                if let Ok(()) = self.controller.remove_network(ssid, sec).await {
                    should_refresh = true
                }
            }
            _ => {}
        };

        if should_refresh {
            if let Ok(networks) = self.controller.get_all_networks().await {
                state_send.push(NetworkState::SetNetworks(networks));
            }
        }
    }

    async fn handle_net_ctx(
        &mut self,
        flags: NetCtxFlags,
        state_send: &mut Vec<NetworkState>,
    ) -> Result<(), ManndError> {
        if flags.intersects(NetCtxFlags::Network) {
            if let Ok(aps) = self.controller.get_all_networks().await {
                state_send.push(NetworkState::SetNetworks(aps));
                state_send.push(NetworkState::Success(NetSuccess::Scan));
            }
        }
        let interface_flags =
            flags.clone() & (NetCtxFlags::Interfaces | NetCtxFlags::InterfacesWpa);

        if interface_flags == NetCtxFlags::Interfaces {
            let interfaces = list_interfaces();
            state_send.push(NetworkState::SetInterfaces(interfaces));
        } else if interface_flags == NetCtxFlags::InterfacesWpa {
            // typically we call through the controller but
            // here I've decided to just go directly for the
            // wireless adapter since only wpa supports it
            if let Some(WirelessAdapter::Wpa(wpa)) = &self.controller.wifi {
                let interfaces = wpa.get_interfaces().await?;
                state_send.push(NetworkState::SetWpaInterfaces(interfaces));
            }
        } else if flags.intersects(NetCtxFlags::InterfacesWpa | NetCtxFlags::Interfaces) {
            tracing::error!("Tried to send a NetCtxFlag with InterfacesWpa and Interfaces, this is not allowed.");
        }

        if flags.intersects(NetCtxFlags::Wireguard) {
            if let Ok((names, meta)) = self.controller.update_wg() {
                state_send.push(NetworkState::SetWireguardInfo((names, meta)));
            }
        }
        if flags.intersects(NetCtxFlags::Netd) {
            let files = get_netd_files().await?;
            state_send.push(NetworkState::SetNetdFiles(files));
        }

        Ok(())
    }
}

pub struct NetworkContext {
    pub networks: Vec<AccessPoint>,
    pub interfaces: Option<InterfaceTypes>,
    // name, metadata, is on?
    pub wg_info: (Vec<String>, Vec<WgMeta>, bool),
    pub netd_files: Vec<String>,
}

pub enum InterfaceTypes {
    Wpa(Vec<WpaInterface>),
    Normal(Vec<String>),
}

impl InterfaceTypes {
    pub fn len(&self) -> usize {
        match self {
            Self::Wpa(ifaces) => ifaces.len(),
            Self::Normal(ifaces) => ifaces.len(),
        }
    }

    pub fn wpa_get(&self, index: usize) -> Option<&WpaInterface> {
        match self {
            Self::Wpa(ifaces) => ifaces.get(index),
            _ => None,
        }
    }

    pub fn norm_get(&self, index: usize) -> Option<&String> {
        match self {
            Self::Normal(ifaces) => ifaces.get(index),
            _ => None,
        }
    }
}

impl Default for NetworkContext {
    fn default() -> Self {
        Self {
            networks: vec![],
            interfaces: None,
            wg_info: (vec![], vec![], false),
            netd_files: vec![],
        }
    }
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub struct NetCtxFlags: u8 {
        const Network = 0b00000001;
        const Interfaces = 0b00000010;
        const Wireguard = 0b00000100;
        const Netd = 0b00001000;
        /// Similar to Interfaces flag but checks
        /// if interface already used by wpa
        const InterfacesWpa = 0b00010000;
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NetworkAction {
    Scan,
    // gets known & nearby networks
    GetCapabilities,
    GetNetworkContext(NetCtxFlags),
    InitWireguard,
    GetWireguard,
    CreateWpaInterface(String),
    ConnectWireguard(PathBuf),
    DisableWireguard,

    Connect(ApConnectInfo),
    ConnectKnown(String, Security),
    Forget(String, Security),

    Exit,
    Disconnect,
}

// to update the ui, mainly
// with prompts
#[derive(Debug, Serialize, Deserialize)]
pub enum NetStart {
    Wifi,
    Scan,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NetSuccess {
    Wifi,
    Scan,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NetFailure {
    Wifi(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NetworkState {
    SetCapabilities(Capability),
    // Use when you want to update state
    // i.e. after connecting to a known network,
    // without recursive call in handle_action
    CallAction(NetworkAction),

    SetNetworks(Vec<AccessPoint>),
    SetInterfaces(Vec<String>),
    SetWpaInterfaces(Vec<WpaInterface>),
    SetWireguardInfo((Vec<String>, Vec<WgMeta>)),
    SetNetdFiles(Vec<String>),

    SetNetworkdFiles(Vec<PathBuf>),
    Start(NetStart),
    Success(NetSuccess),
    Failed(NetFailure),
}

// determines what options will be visible/selectable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub wifi_daemon: Option<DaemonType>,
    pub networkd_active: bool,
    // installed, wg-mannd interface active
    pub wireguard: (bool, bool),
}

impl Capability {
    pub fn new(
        wifi_daemon: Option<DaemonType>,
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

// #[derive(Builder, Debug, Clone, Serialize, Deserialize)]
// pub struct EapInfo {
//     pub eap_method: EapMethod,
//     pub identity: String,
//     #[builder(default = "None")]
//     pub anonymous_identity: Option<String>,
//
//     // Optional because EAP-TLS uses certs instead.
//     pub password: Option<String>,
//     pub ca_cert: PathBuf,
//
//     // limits accepted certs
//     #[builder(default = "None")]
//     pub domain_match: Option<String>,
//
//     // Required for PEAP and TTLS
//     pub phase2: Option<PhaseTwo>,
//     // EAP-TLS
//     #[builder(default = "None")]
//     pub client_cert: Option<PathBuf>,
//     #[builder(default = "None")]
//     pub client_key: Option<PathBuf>,
//     // Used if client key is encrypted
//     #[builder(default = "None")]
//     pub client_key_password: Option<String>,
// }
//
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum PhaseTwo {
//     Eap(EapMethod),
//     // non-eap variants
//     Pap,
//     Chap,
//     Mschap,
//     Mschapv2,
//     // user can specify custom
//     Legacy(String),
// }
//
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum EapMethod {
//     TLS,
//     PEAP,
//     TTLS,
//     PWD,
//     SIM,
//     AKA,
//     // AKA'
//     AKA_PRIME,
//     MSCHAPV2,
//     GTC,
//     // methods below not in iwd
//     MD5,
//     FAST,
//     LEAP,
// }
