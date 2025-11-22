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
            info!("{:?}", controller.wifi);
            let daemon = controller.daemon_type();
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
    GetAllNetworks,
    GetKnownNetworks,
    Connect(String, String, Security),
    Forget(String, Security),
    Info,
    Exit,
    Disconnect,
    ForceIwd,
    ForceWpa,
}

pub enum NetUpdate {
    AddKnownNetworks(Vec<AccessPoint>),
    UpdateAps(Vec<AccessPoint>),
    UpdateApsHidden(Vec<AccessPoint>),
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
        NetworkAction::GetAllNetworks => {
            if let Ok(aps) = controller.get_networks().await {
                let _ = state_update.send(NetUpdate::UpdateAps(aps)).await;
            }
        }
        NetworkAction::Connect(ssid, psk, sec) => {
            if let Ok(()) = controller.ssid_connect(ssid, psk, sec).await {
                info!("Connection to network was successful");
            } else {
                tracing::error!("Connection to network was not successful.");
            }
        }
        NetworkAction::Disconnect => {
            if let Ok(()) = controller.disconenct().await {
                info!("Disconnected from a network");
            } else {
            }
        }
        NetworkAction::GetKnownNetworks => {
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
                if let Ok(aps) = controller.get_networks().await {
                    let _ = state_update.send(NetUpdate::UpdateAps(aps)).await;
                }
            }
        }
        NetworkAction::Exit => {
            if let Ok(()) = controller.exit().await {
                return true;
            }
        }
        NetworkAction::ForceIwd => {}
        NetworkAction::ForceWpa => {}
        _ => {}
    };
    false
}
