use std::path::PathBuf;

use derive_builder::Builder;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::info;

use crate::{
    controller::{Controller, DaemonType},
    state::signals::{SignalManager, SignalUpdate},
    wireless::common::{AccessPoint, Security},
};

pub struct NetworkActor {}

impl NetworkActor {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(
        &mut self,
        mut action_rx: Receiver<NetworkAction>,
        action_tx: Sender<NetworkAction>,
        state_tx: Sender<NetworkState>,
    ) {
        // networking thread
        // Signal <-> Network -> UI Update
        //
        // A signal update leads to a network update i.e. if networks are
        // loaded and we get a signal then this leads to getting the
        // network values via a network update
        let (signal_tx, mut signal_rx) = mpsc::channel::<SignalUpdate>(32);
        let mut signal_manager = SignalManager::new();

        if let Ok(mut controller) = Controller::new().await {
            controller.determine_adapter().await;
            let daemon = controller.daemon_type();

            if let Some(d) = &daemon {
                let _ = state_tx.send(NetworkState::SetDaemon(d.clone())).await;
            }

            loop {
                tokio::select! {
                    Some(action) = action_rx.recv() => {
                        if handle_action(&mut controller, state_tx.clone(), signal_tx.clone(), action).await {
                            break;
                        }
                    }
                    // add new signals to listen for
                    Some(update) = signal_rx.recv() => {
                        info!("New signal received");
                        signal_manager.handle_update(update);
                    }
                    Some(msg) = signal_manager.recv() => {
                        match daemon {
                            // iwd
                            Some(DaemonType::Iwd) => {
                                signal_manager.process_iwd_msg(msg, action_tx.clone()).await;
                            }
                            // wpa
                            Some(DaemonType::Wpa) => {
                                signal_manager.process_wpa_msg(msg, action_tx.clone()).await;
                            }
                            _ => {
                                break;
                            }
                        }
                    }
                };
            }
        } else {
            tracing::error!("Fatal: Controller not initalised!");
        }
    }
}

#[derive(Debug)]
pub enum NetworkAction {
    Scan,
    // gets known & nearby networks
    GetNetworks,
    GetWireguardFiles,
    Connect(ApConnectInfo),
    ConnectKnown(String, Security),
    Forget(String, Security),
    Exit,
    Disconnect,
}

// to update the ui, mainly
// with prompts
pub enum NetStart {
    Connection,
    Scan,
}

pub enum NetSuccess {
    Connection,
    Scan,
}

pub enum NetFailure {
    Connection(String),
}

pub enum NetworkState {
    // Use when you want to update state
    // i.e. after connecting to a known network,
    // without recursive call in handle_action
    CallAction(NetworkAction),
    UpdateNetworks(Vec<AccessPoint>),
    SetDaemon(DaemonType),
    Start(NetStart),
    Success(NetSuccess),
    Failed(NetFailure),
}

/// Returns true if we are quitting the application
pub async fn handle_action<'a>(
    controller: &mut Controller,
    state_update: Sender<NetworkState>,
    signal_tx: Sender<SignalUpdate<'a>>,
    action: NetworkAction,
) -> bool {
    match action {
        NetworkAction::GetNetworks => {
            if let Ok(aps) = controller.get_all_networks().await {
                let _ = state_update.send(NetworkState::UpdateNetworks(aps)).await;
                let _ = state_update
                    .send(NetworkState::Success(NetSuccess::Scan))
                    .await;
            }
        }
        NetworkAction::Scan => {
            let _ = state_update.send(NetworkState::Start(NetStart::Scan)).await;

            if let Ok(()) = controller.scan(signal_tx.clone()).await {}
        }

        NetworkAction::Connect(info) => {
            let _ = state_update
                .send(NetworkState::Start(NetStart::Connection))
                .await;

            match controller.network_connect(info).await {
                Ok(()) => {
                    info!("Connection to network was successful");
                    let _ = state_update
                        .send(NetworkState::Success(NetSuccess::Connection))
                        .await;
                }
                Err(e) => {
                    tracing::error!("Connection to network was not successful.");
                    let _ = state_update
                        .send(NetworkState::Failed(NetFailure::Connection(e.to_string())))
                        .await;
                }
            }
        }
        NetworkAction::ConnectKnown(ssid, security) => {
            match controller.connect_known(ssid, security).await {
                Ok(()) => {
                    let _ = state_update
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
        NetworkAction::Forget(ssid, sec) => {
            if let Ok(()) = controller.remove_network(ssid, sec).await {
                //     if let Ok(aps) = controller.get_networks().await {
                //         let _ = state_update.send(NetUpdate::UpdateAps(aps)).await;
                //     }
            }
        }
        NetworkAction::Exit => {
            if let Ok(()) = controller.exit().await {
                return true;
            }
        }
        _ => {}
    };
    false
}

/// For connecting to APs used by wpa and iwd
#[derive(Builder, Debug, Clone)]
pub struct ApConnectInfo {
    pub ssid: String,
    pub security: Security,
    pub credentials: Credentials,
}

#[derive(Debug, Clone)]
pub enum Credentials {
    Password(String),
    Eap(EapInfo),
}

#[derive(Builder, Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
