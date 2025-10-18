use com::{controller::Controller, wireless::common::AccessPoint};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::info;

use crate::app::AppState;

#[derive(Debug)]
pub enum NetworkAction {
    Scan,
    Connect(String, String),
    ForceIwd,
    ForceWpa,
    ForceWifiNetlink,
}

pub enum NetworkUpdate {
    Select(usize),
    Deselect,
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
                        info!("Connection to network was successful\n");
                    } else {
                        tracing::error!("Connection to network was not successful.\n");
                    }
                }
                NetworkAction::ForceIwd => {}
                NetworkAction::ForceWpa => {}
                _ => {}
            };
        }
    }
}
