use std::{path::PathBuf, process::Command};

use derive_builder::Builder;
use postcard::to_stdvec_cobs;
use serde::{Deserialize, Serialize};
use tokio::{net::unix::WriteHalf, sync::mpsc::Sender};
use tracing::info;

use crate::{
    controller::{Controller, DaemonType},
    error::ManndError,
    state::signals::{SignalManager, SignalUpdate},
    utils::list_interfaces,
    wireguard::store::WgMeta,
    wireless::common::{AccessPoint, Security},
};

pub struct NetworkActor<'a> {
    pub controller: Controller,
    pub signal_manager: SignalManager<'a>,
}

impl<'a> NetworkActor<'a> {
    pub async fn new() -> Result<Self, ManndError> {
        let mut controller = Controller::new().await?;
        controller.determine_adapter().await;
        let signal_manager = SignalManager::new();
        Ok(Self {
            controller,
            signal_manager,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NetworkAction {
    Scan,
    // gets known & nearby networks
    GetNetworks,
    InitWireguard,
    UpdateWireguard,
    ConnectWireguard(PathBuf),
    Connect(ApConnectInfo),
    ConnectKnown(String, Security),
    Forget(String, Security),
    GetCapabilities,
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
    UpdateNetworks(Vec<AccessPoint>),
    UpdateWgDb((Vec<String>, Vec<WgMeta>)),
    Start(NetStart),
    Success(NetSuccess),
    Failed(NetFailure),
}

/// Returns true if we are quitting the application
pub async fn handle_action<'a>(
    controller: &mut Controller,
    action: NetworkAction,
    sock_tx: Sender<NetworkState>,
    signal_tx: Sender<SignalUpdate<'a>>,
) -> Result<bool, ManndError> {
    // check if wifi then allow wifi requests
    if controller.wifi.is_some() {
        match action.clone() {
            // WIFI
            NetworkAction::GetNetworks => {
                if let Ok(aps) = controller.get_all_networks().await {
                    if let Ok(()) = sock_tx.send(NetworkState::UpdateNetworks(aps)).await {
                        let _ = sock_tx.send(NetworkState::Success(NetSuccess::Scan)).await;
                    }
                }
            }
            NetworkAction::Scan => {
                if let Ok(()) = sock_tx.send(NetworkState::Start(NetStart::Scan)).await {
                    if let Ok(()) = controller.scan(signal_tx.clone()).await {}
                }
            }

            NetworkAction::Connect(info) => {
                let _ = sock_tx.send(NetworkState::Start(NetStart::Wifi)).await;

                match controller.network_connect(info).await {
                    Ok(()) => {
                        info!("Connection to network was successful");
                        let _ = sock_tx.send(NetworkState::Success(NetSuccess::Wifi)).await;
                    }
                    Err(e) => {
                        tracing::error!("Connection to network was not successful.");
                        let _ = sock_tx
                            .send(NetworkState::Failed(NetFailure::Wifi(e.to_string())))
                            .await;
                    }
                }
            }
            NetworkAction::ConnectKnown(ssid, security) => {
                match controller.connect_known(ssid, security).await {
                    Ok(()) => {
                        let _ = sock_tx
                            .send(NetworkState::CallAction(NetworkAction::GetNetworks))
                            .await;
                    }
                    Err(e) => {}
                }
            }
            NetworkAction::Disconnect => {
                if let Ok(()) = controller.disconenct().await {
                    info!("Disconnected from a network");
                } else {
                }
            }
            NetworkAction::GetCapabilities => {
                let interfaces = list_interfaces();
                let wifi_daemon = controller.daemon_type();
                let networkd_active = controller.networkd_status().await;
                let wg = Command::new("wg")
                    .arg("--version")
                    .output()
                    .map_or(false, |_| true);

                let caps = Capability::new(interfaces, wifi_daemon, networkd_active, wg);
                let _ = sock_tx.send(NetworkState::SetCapabilities(caps)).await;
            }
            NetworkAction::Forget(ssid, sec) => {
                if let Ok(()) = controller.remove_network(ssid, sec).await {
                    //     if let Ok(aps) = controller.get_networks().await {
                    //         let _ = state_update.send(NetUpdate::UpdateAps(aps)).await;
                    //     }
                }
            }
            _ => {}
        }
    };

    // Wi-Fi not needed
    match action {
        NetworkAction::ConnectWireguard(file) => {
            // controller.connect_wg(file).await?;
        }
        // WIREGUARD
        NetworkAction::InitWireguard => {
            if let Ok(()) = controller.start_wg().await {
                if let Ok((names, meta)) = controller.update_wg() {
                    let _ = sock_tx.send(NetworkState::UpdateWgDb((names, meta))).await;
                }
            }
        }
        NetworkAction::Exit => {
            if let Ok(()) = controller.exit().await {
                return Ok(true);
            }
        }
        _ => {}
    };
    Ok(false)
}

// determines what options will be visible/selectable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    // non-virtual interfaces
    pub interfaces: Vec<String>,
    pub wifi_daemon: Option<DaemonType>,
    pub networkd_active: bool,
    pub wireguard: bool,
}

impl Capability {
    pub fn new(
        interfaces: Vec<String>,
        wifi_daemon: Option<DaemonType>,
        networkd_active: bool,
        wireguard: bool,
    ) -> Self {
        Capability {
            interfaces,
            wifi_daemon,
            networkd_active,
            wireguard,
        }
    }

    pub fn default() -> Self {
        Capability {
            interfaces: vec![],
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
    Eap(EapInfo),
}

#[derive(Builder, Debug, Clone, Serialize, Deserialize)]
pub struct EapInfo {
    pub eap_method: EapMethod,
    pub identity: String,
    #[builder(default = "None")]
    pub anonymous_identity: Option<String>,

    // Optional because EAP-TLS uses certs instead.
    pub password: Option<String>,
    pub ca_cert: PathBuf,

    // limits accepted certs
    #[builder(default = "None")]
    pub domain_match: Option<String>,

    // Required for PEAP and TTLS
    pub phase2: Option<PhaseTwo>,
    // EAP-TLS
    #[builder(default = "None")]
    pub client_cert: Option<PathBuf>,
    #[builder(default = "None")]
    pub client_key: Option<PathBuf>,
    // Used if client key is encrypted
    #[builder(default = "None")]
    pub client_key_password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseTwo {
    Eap(EapMethod),
    // non-eap variants
    Pap,
    Chap,
    Mschap,
    Mschapv2,
    // user can specify custom
    Legacy(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EapMethod {
    TLS,
    PEAP,
    TTLS,
    PWD,
    SIM,
    AKA,
    // AKA'
    AKA_PRIME,
    MSCHAPV2,
    GTC,
    // methods below not in iwd
    MD5,
    FAST,
    LEAP,
}
