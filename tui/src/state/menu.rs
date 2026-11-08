use mannd::controller::WifiDaemonType;

use crate::state::{StateCommand, StateResult, View, vpn::VpnState, wifi::WifiState};

#[derive(Debug, PartialEq)]
pub enum MainMenuSelection {
    Wifi,
    Vpn,
    Exit,
}

impl MainMenuSelection {
    pub fn execute(&self, daemon: &Option<WifiDaemonType>) -> StateResult {
        match self {
            Self::Wifi => {
                // DaemonType safe here
                StateResult::Command(StateCommand::ChangeView(View::Wifi(WifiState::new(
                    daemon.as_ref().unwrap(),
                ))))
            }
            Self::Vpn => {
                StateResult::Command(StateCommand::ChangeView(View::Vpn(VpnState::default())))
            }
            Self::Exit => StateResult::Command(StateCommand::Exit),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wifi => "Wi-Fi",
            Self::Vpn => "VPN",
            Self::Exit => "Exit",
        }
    }
}
