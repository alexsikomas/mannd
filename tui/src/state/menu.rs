use mannd::controller::WifiDaemonType;

use crate::state::{
    StateCommand, StateResult, View, networkd::NetdState, vpn::VpnState, wifi::WifiState,
};

#[derive(Debug, PartialEq)]
pub enum MainMenuSelection {
    Wifi,
    Vpn,
    Networkd,
    Config,
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
            Self::Config => StateResult::Command(StateCommand::ChangeView(View::Config)),
            Self::Networkd => StateResult::Command(StateCommand::ChangeView(View::Networkd(
                NetdState::default(),
            ))),
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
            Self::Networkd => "Networkd",
            Self::Config => "Config",
            Self::Exit => "Exit",
        }
    }
}
