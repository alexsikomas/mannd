use std::{path::PathBuf, process::Command};

use bitflags::bitflags;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tracing::info;

use crate::{
    controller::{Controller, DaemonType},
    error::ManndError,
    state::signals::{SignalManager, SignalUpdate},
    systemd::networkd::get_netd_files,
    utils::list_interfaces,
    wireguard::store::WgMeta,
    wireless::common::{AccessPoint, Security},
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
                let wg = Command::new("wg")
                    .arg("--version")
                    .output()
                    .map_or(false, |_| true);

                let caps = Capability::new(wifi_daemon, networkd_active, wg);
                state_send.push(NetworkState::SetCapabilities(caps));
            }
            // WIREGUARD
            NetworkAction::ConnectWireguard(file) => {
                // controller.connect_wg(file).await?;
            }
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
        match action {
            // WIFI
            // NetworkAction::GetNetworks => {
            //     if let Ok(aps) = self.controller.get_all_networks().await {
            //         state_send.push(NetworkState::SetNetworks(aps));
            //     }
            // }
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
                    }
                    Err(e) => {}
                }
            }
            NetworkAction::Disconnect => {
                if let Ok(()) = self.controller.disconenct().await {
                    info!("Disconnected from a network");
                } else {
                }
            }
            NetworkAction::Forget(ssid, sec) => {
                if let Ok(()) = self.controller.remove_network(ssid, sec).await {
                    //     if let Ok(aps) = controller.get_networks().await {
                    //         let _ = state_update.send(NetUpdate::UpdateAps(aps)).await;
                    //     }
                }
            }
            _ => {}
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
        if flags.intersects(NetCtxFlags::Interfaces) {
            let interfaces = list_interfaces();
            state_send.push(NetworkState::SetInterfaces(interfaces));
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
    pub interfaces: Vec<String>,
    pub wg_info: (Vec<String>, Vec<WgMeta>),
    pub netd_files: Vec<String>,
}

impl Default for NetworkContext {
    fn default() -> Self {
        Self {
            networks: vec![],
            interfaces: vec![],
            wg_info: (vec![], vec![]),
            netd_files: vec![],
        }
    }
}

bitflags! {
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct NetCtxFlags: u8 {
        const Network = 0b00000001;
        const Interfaces = 0b00000010;
        const Wireguard = 0b00000100;
        const Netd = 0b00001000;
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
    pub wireguard: bool,
}

impl Capability {
    pub fn new(wifi_daemon: Option<DaemonType>, networkd_active: bool, wireguard: bool) -> Self {
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
            wireguard: false,
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
