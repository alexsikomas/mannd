use com::{controller::Controller, wireless::common::AccessPoint};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::info;

use crate::app::AppState;

#[derive(Debug)]
pub enum NetworkAction {
    Scan,
    GetKnownNetworks,
    Connect(String, String),
    Info,
    Disconnect,
    ForceIwd,
    ForceWpa,
    ForceWifiNetlink,
}

pub enum NetworkUpdate {
    Select(usize),
    Deselect,
    /// Unreachable known networks
    AddKnownNetworks(Vec<AccessPoint>),
    UpdateAps(Vec<AccessPoint>),
}

#[derive(Debug)]
pub struct NetworkState {
    pub selected: Option<usize>,
    pub aps: Vec<AccessPoint>,
}

pub async fn network_handle(
    net_action_rx: &mut Receiver<NetworkAction>,
    net_update_tx: Sender<NetworkUpdate>,
) {
    if let Ok(mut controller) = Controller::new().await {
        controller.determine_adapter().await;
        while let Some(action) = net_action_rx.recv().await {
            match action {
                NetworkAction::Scan => {
                    if let Ok(()) = controller.scan().await {
                        if let Ok(aps) = controller.get_networks().await {
                            let _ = net_update_tx.send(NetworkUpdate::UpdateAps(aps)).await;
                        }
                    }
                }
                NetworkAction::Connect(ssid, psk) => {
                    if let Ok(()) = controller.ssid_connect(ssid, psk).await {
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
                        let _ = net_update_tx.send(NetworkUpdate::AddKnownNetworks(known_aps));
                    }
                }
                NetworkAction::ForceIwd => {}
                NetworkAction::ForceWpa => {}
                _ => {}
            };
        }
    }
}
