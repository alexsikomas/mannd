use mannd::{
    controller::WifiDaemonType,
    state::messages::{NetworkAction, WifiAction, WpaAction},
    store::{NetworkInfo, NetworkSecurity},
    wireless::common::NetworkFlags,
};

use crate::{
    keys::KeyAction,
    state::{
        AppContext, Component, Cursor, SelectableList, StateCommand, StateResult,
        prompts::{PromptState, PskConnectionPrompt, WpaInterfacePrompt},
    },
};

// Connection
//
// Possible actions the user can take in the connection menu
#[derive(Debug)]
pub struct WifiState {
    pub focused_area: ConnectionFocus,
    pub actions: SelectableList<ConnectionAction>,
    // selected network
    pub network_cursor: Cursor,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionAction {
    Scan,
    Connect,
    Disconnect,
    Interfaces,
    // Info,
    Forget,
}

impl ConnectionAction {
    // actions which once enabled shouldn't be able to go away
    fn perma_actions() -> Vec<Self> {
        vec![Self::Scan, Self::Interfaces]
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionFocus {
    Networks,
    Actions,
}

impl WifiState {
    pub fn new(daemon: &WifiDaemonType) -> Self {
        let mut actions = SelectableList::new(vec![ConnectionAction::Scan]);
        if daemon == &WifiDaemonType::Wpa {
            actions.items.push(ConnectionAction::Interfaces);
        }

        Self {
            focused_area: ConnectionFocus::Actions,
            actions,
            network_cursor: Cursor::default(),
        }
    }

    pub fn refresh_available_actions(&mut self, networks: &[NetworkInfo]) {
        let perma_actions = ConnectionAction::perma_actions();
        self.actions.items.retain(|a| perma_actions.contains(a));

        if let Some(ap) = networks.get(self.network_cursor.index) {
            let flags = ap.flags;
            if flags.contains(NetworkFlags::NEARBY) && !flags.contains(NetworkFlags::CONNECTED) {
                self.actions.items.push(ConnectionAction::Connect);
            }

            if flags.contains(NetworkFlags::CONNECTED) {
                self.actions.items.push(ConnectionAction::Disconnect);
            }

            if flags.contains(NetworkFlags::KNOWN) {
                self.actions.items.push(ConnectionAction::Forget);
            }
        }
    }
}

impl Component for WifiState {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        let net_ctx = ctx.net_ctx;
        // check if up or down
        match key {
            KeyAction::Right | KeyAction::Left => {
                self.focused_area = match self.focused_area {
                    ConnectionFocus::Actions => ConnectionFocus::Networks,
                    ConnectionFocus::Networks => ConnectionFocus::Actions,
                };
                return StateResult::Consumed;
            }
            _ => {}
        }

        match self.focused_area {
            ConnectionFocus::Actions => {
                self.actions.on_key(key, ctx);
                if key == &KeyAction::Enter {
                    return match self.actions.selected() {
                        Some(ConnectionAction::Scan) => StateResult::Command(
                            StateCommand::NetworkAction(NetworkAction::Wifi(WifiAction::Scan)),
                        ),
                        Some(ConnectionAction::Connect) => {
                            if let Some(network) = net_ctx.networks.get(self.network_cursor.index) {
                                if network.flags.contains(NetworkFlags::KNOWN) {
                                    return StateResult::Command(StateCommand::NetworkAction(
                                        NetworkAction::Wifi(WifiAction::ConnectKnown(
                                            network.clone(),
                                        )),
                                    ));
                                }

                                match &network.security {
                                    NetworkSecurity::Open | NetworkSecurity::Owe => {
                                        StateResult::Command(StateCommand::NetworkAction(
                                            NetworkAction::Wifi(WifiAction::Connect(
                                                network.clone(),
                                            )),
                                        ))
                                    }
                                    NetworkSecurity::Wpa2 { .. }
                                    | NetworkSecurity::Wpa2Hex { .. }
                                    | NetworkSecurity::Wpa3Sae { .. }
                                    | NetworkSecurity::Wpa3Transition { .. } => {
                                        let prompt = PskConnectionPrompt::new(network.clone());
                                        StateResult::Command(StateCommand::Prompt(
                                            PromptState::PskConnect(prompt),
                                        ))
                                    }
                                    _ => StateResult::Consumed,
                                }
                            } else {
                                StateResult::Consumed
                            }
                        }
                        Some(ConnectionAction::Disconnect) => {
                            StateResult::Command(StateCommand::NetworkAction(NetworkAction::Wifi(
                                WifiAction::Disconnect,
                            )))
                        }
                        Some(ConnectionAction::Forget) => {
                            if let Some(network) = net_ctx.networks.get(self.network_cursor.index) {
                                if network.flags.contains(NetworkFlags::KNOWN) {
                                    return StateResult::Command(StateCommand::NetworkAction(
                                        NetworkAction::Wifi(WifiAction::Forget(network.clone())),
                                    ));
                                }
                                StateResult::Consumed
                            } else {
                                StateResult::Consumed
                            }
                        }
                        Some(ConnectionAction::Interfaces) => {
                            let mut cmds: Vec<StateCommand> = vec![];
                            cmds.push(StateCommand::Prompt(PromptState::WpaInterface(
                                WpaInterfacePrompt::default(),
                            )));
                            cmds.push(StateCommand::NetworkAction(NetworkAction::Wpa(
                                WpaAction::GetInterfaces,
                            )));
                            return StateResult::Commands(cmds);
                        }
                        _ => StateResult::Consumed,
                    };
                }
            }
            ConnectionFocus::Networks => match key {
                KeyAction::Down => {
                    if !ctx.net_ctx.networks.is_empty() {
                        self.network_cursor.next(net_ctx.networks.len());
                        self.refresh_available_actions(&net_ctx.networks);
                    }
                }
                KeyAction::Up => {
                    if !net_ctx.networks.is_empty() {
                        self.network_cursor.prev(net_ctx.networks.len());
                        self.refresh_available_actions(&net_ctx.networks);
                    }
                }
                _ => {}
            },
        }
        StateResult::Consumed
    }
}

impl ConnectionAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scan => "Scan",
            Self::Connect => "Connect",
            Self::Disconnect => "Disconnect",
            Self::Interfaces => "Interfaces",
            Self::Forget => "Forget",
        }
    }
}
