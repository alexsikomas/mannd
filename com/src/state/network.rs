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
        state_tx: Sender<NetUpdate>,
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
            let daemon = controller.daemon_type().inspect(|d| {
                state_tx.send(NetUpdate::SetDaemon(d.clone()));
            });

            loop {
                tokio::select! {
                    Some(action) = action_rx.recv() => {
                        if handle_action(&mut controller, state_tx.clone(), signal_tx.clone(), action).await {
                            break;
                        }
                    }
                    // add new signals to listen for
                    Some(update) = signal_rx.recv() => {
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
    GetNearbyNetworks,
    GetConnectedNetworks,
    Connect(ApConnectInfo),
    Forget(String, Security),
    Exit,
    Disconnect,
}

pub enum NetUpdate {
    AddKnownNetworks(Vec<AccessPoint>),
    UpdateAps(Vec<AccessPoint>),
    UpdateApsHidden(Vec<AccessPoint>),
    ConnectFailed(String),
    SetDaemon(DaemonType),
}

#[derive(Debug)]
pub struct NetworkState {
    pub selected: Option<usize>,
    pub aps: Vec<AccessPoint>,
}

/// Returns true if we are quitting the application
pub async fn handle_action<'a>(
    controller: &mut Controller,
    state_update: Sender<NetUpdate>,
    signal_tx: Sender<SignalUpdate<'a>>,
    action: NetworkAction,
) -> bool {
    match action {
        NetworkAction::Scan => if let Ok(()) = controller.scan(signal_tx.clone()).await {},
        NetworkAction::GetNearbyNetworks => {
            if let Ok(aps) = controller.get_all_networks().await {
                let _ = state_update.send(NetUpdate::UpdateAps(aps)).await;
            }
        }
        NetworkAction::Connect(info) => match controller.network_connect(info).await {
            Ok(()) => {
                info!("Connection to network was successful");
            }
            Err(e) => {
                tracing::error!("Connection to network was not successful.");
                state_update.send(NetUpdate::ConnectFailed(e.to_string()));
            }
        },
        NetworkAction::Disconnect => {
            if let Ok(()) = controller.disconenct().await {
                info!("Disconnected from a network");
            } else {
            }
        }
        NetworkAction::GetConnectedNetworks => {
            if let Ok(known_aps) = controller.get_known_networks().await {
                // At this point some of the networks will still be reachable
                // we don't have self so can't do check here
                let _ = state_update
                    .send(NetUpdate::AddKnownNetworks(known_aps))
                    .await;
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
