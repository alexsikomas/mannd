use com::{controller::Controller, wireless::common::AccessPoint};
use tokio::sync::mpsc::{Receiver, Sender};

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
    event_rx: &mut Receiver<NetworkAction>,
    net_state_tx: Sender<NetworkUpdate>,
) {
    if let Ok(mut controller) = Controller::new().await {
        controller.determine_adapter().await;
        while let Some(action) = event_rx.recv().await {
            match action {
                NetworkAction::Scan => {
                    if let Ok(aps) = controller.scan().await {
                        let _ = net_state_tx.send(NetworkUpdate::UpdateAps(aps)).await;
                    }
                }
                NetworkAction::ForceIwd => {}
                NetworkAction::ForceWpa => {}
                _ => {}
            };
        }
    }
}
