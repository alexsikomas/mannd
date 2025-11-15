use tokio::sync::mpsc::Sender;
use tracing::info;

use crate::{
    controller::Controller,
    state::signals::SignalUpdate,
    wireless::common::{AccessPoint, Security},
};

#[derive(Debug)]
pub enum NetworkAction {
    Scan,
    GetKnownNetworks,
    Connect(String, String, Security),
    Forget(String, Security),
    Info,
    Exit,
    Disconnect,
    ForceIwd,
    ForceWpa,
    ForceWifiNetlink,
}

pub enum StateUpdate {
    Select(usize),
    Update,
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

/// Returns true if we are quitting the application
pub async fn handle_action<'a>(
    controller: &mut Controller,
    state_update: Sender<StateUpdate>,
    signal_tx: Sender<SignalUpdate<'a>>,
    action: NetworkAction,
) -> bool {
    match action {
        NetworkAction::Scan => {
            if let Ok(()) = controller.scan(signal_tx.clone()).await {
                if let Ok(aps) = controller.get_networks().await {
                    let _ = state_update.send(StateUpdate::UpdateAps(aps)).await;
                }
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
                    .send(StateUpdate::AddKnownNetworks(known_aps))
                    .await;
            }
        }
        NetworkAction::Forget(ssid, sec) => {
            if let Ok(()) = controller.remove_network(ssid, sec).await {
                if let Ok(aps) = controller.get_networks().await {
                    let _ = state_update.send(StateUpdate::UpdateAps(aps)).await;
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
