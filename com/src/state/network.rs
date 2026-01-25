use std::path::PathBuf;

use derive_builder::Builder;
use postcard::to_stdvec_cobs;
use serde::{Deserialize, Serialize};
use tokio::{net::unix::WriteHalf, sync::mpsc::Sender};
use tracing::info;

use crate::{
    controller::{Controller, DaemonType},
    error::ManndError,
    state::signals::{SignalManager, SignalUpdate},
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

#[derive(Debug, Serialize, Deserialize)]
pub enum NetworkAction {
    Scan,
    // gets known & nearby networks
    GetNetworks,
    InitWireguard,
    UpdateWireguard,
    Connect(ApConnectInfo),
    ConnectKnown(String, Security),
    Forget(String, Security),
    Exit,
    Disconnect,
}

// to update the ui, mainly
// with prompts
#[derive(Serialize, Deserialize)]
pub enum NetStart {
    Connection,
    Scan,
}

#[derive(Serialize, Deserialize)]
pub enum NetSuccess {
    Connection,
    Scan,
}

#[derive(Serialize, Deserialize)]
pub enum NetFailure {
    Connection(String),
}

#[derive(Serialize, Deserialize)]
pub enum NetworkState {
    // Use when you want to update state
    // i.e. after connecting to a known network,
    // without recursive call in handle_action
    CallAction(NetworkAction),
    UpdateNetworks(Vec<AccessPoint>),
    UpdateWgDb((Vec<String>, Vec<WgMeta>)),
    SetDaemon(DaemonType),
    Start(NetStart),
    Success(NetSuccess),
    Failed(NetFailure),
}

/// Returns true if we are quitting the application
pub async fn handle_action<'a>(
    controller: &mut Controller,
    action: NetworkAction,
    sock_tx: Sender<Vec<u8>>,
    signal_tx: Sender<SignalUpdate<'a>>,
) -> Result<bool, ManndError> {
    match action {
        // WIFI
        NetworkAction::GetNetworks => {
            if let Ok(aps) = controller.get_all_networks().await {
                let update_msg = to_stdvec_cobs(&NetworkState::UpdateNetworks(aps))?;
                if let Ok(()) = sock_tx.send(update_msg).await {
                    let scan_start_msg = to_stdvec_cobs(&NetworkState::Success(NetSuccess::Scan))?;
                    let _ = sock_tx.send(scan_start_msg).await;
                }
            }
        }
        NetworkAction::Scan => {
            let start_scan_msg = to_stdvec_cobs(&NetworkState::Start(NetStart::Scan))?;
            if let Ok(()) = sock_tx.send(start_scan_msg).await {
                if let Ok(()) = controller.scan(signal_tx.clone()).await {}
            }
        }

        NetworkAction::Connect(info) => {
            let msg = to_stdvec_cobs(&NetworkState::Start(NetStart::Connection))?;
            let _ = sock_tx.send(msg).await;

            match controller.network_connect(info).await {
                Ok(()) => {
                    info!("Connection to network was successful");
                    let msg = to_stdvec_cobs(&NetworkState::Success(NetSuccess::Connection))?;
                    let _ = sock_tx.send(msg).await;
                }
                Err(e) => {
                    tracing::error!("Connection to network was not successful.");
                    let msg = to_stdvec_cobs(&NetworkState::Failed(NetFailure::Connection(
                        e.to_string(),
                    )))?;
                    let _ = sock_tx.send(msg).await;
                }
            }
        }
        NetworkAction::ConnectKnown(ssid, security) => {
            match controller.connect_known(ssid, security).await {
                Ok(()) => {
                    let msg =
                        to_stdvec_cobs(&NetworkState::CallAction(NetworkAction::GetNetworks))?;
                    let _ = sock_tx.send(msg).await;
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
        NetworkAction::Forget(ssid, sec) => {
            if let Ok(()) = controller.remove_network(ssid, sec).await {
                //     if let Ok(aps) = controller.get_networks().await {
                //         let _ = state_update.send(NetUpdate::UpdateAps(aps)).await;
                //     }
            }
        }
        // WIREGUARD
        NetworkAction::InitWireguard => {
            if let Ok(()) = controller.start_wg().await {
                if let Ok((names, meta)) = controller.update_wg() {
                    info!("Updated wireguard successfully");
                    let msg = to_stdvec_cobs(&NetworkState::UpdateWgDb((names, meta)))?;
                    let _ = sock_tx.send(msg).await;
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
