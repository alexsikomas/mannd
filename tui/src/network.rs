use com::{controller::Controller, wireless::common::AccessPoint};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::info;

use crate::app::AppState;

#[derive(Debug)]
pub enum NetworkAction {
    Scan,
    ForceIwd,
    ForceWpa,
    ForceWifiNetlink,
}

pub enum NetworkUpdate {
    Select(usize),
    Deselect,
    UpdateAps(Vec<AccessPoint>),
}

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
                NetworkAction::ForceIwd => {}
                NetworkAction::ForceWpa => {}
                _ => {}
            };
        }
    }
}
