use mannd::{
    state::messages::{NetworkAction, WifiAction, WpaAction},
    store::{NetworkInfo, NetworkSecurity},
    wireless::wpa_supplicant::WpaInterface,
};

use crate::{
    keys::KeyAction,
    state::{AppContext, Component, Cursor, SelectableList, StateCommand, StateResult},
};

#[derive(Debug)]
pub enum PromptState {
    PskConnect(PskConnectionPrompt),
    WpaInterface(WpaInterfacePrompt),
    /// Displays general info or errors to the user
    Info(InfoPrompt),
}

impl Component for PromptState {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        match self {
            PromptState::PskConnect(prompt) => prompt.on_key(key, ctx),
            PromptState::Info(prompt) => prompt.on_key(key, ctx),
            PromptState::WpaInterface(prompt) => prompt.on_key(key, ctx),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PskPromptSelect {
    Password,
    Show,
    Connect,
    Back,
}

impl PskPromptSelect {
    fn as_vec() -> Vec<Self> {
        vec![Self::Password, Self::Show, Self::Connect, Self::Back]
    }
}

#[derive(Debug)]
pub struct PskConnectionPrompt {
    pub network: NetworkInfo,
    pub password: String,
    pub select: SelectableList<PskPromptSelect>,
    pub show_password: bool,
}

impl PskConnectionPrompt {
    pub fn new(network: NetworkInfo) -> Self {
        Self {
            network,
            password: String::new(),
            select: SelectableList::new(PskPromptSelect::as_vec()),
            show_password: false,
        }
    }
}

impl Component for PskConnectionPrompt {
    fn on_key(&mut self, key: &KeyAction, _ctx: &AppContext) -> StateResult {
        let Some(selected) = self.select.selected() else {
            return StateResult::Ignored;
        };

        match key {
            KeyAction::Up | KeyAction::Down => match selected {
                PskPromptSelect::Password | PskPromptSelect::Show => {
                    self.select.set(PskPromptSelect::Connect);
                }
                PskPromptSelect::Connect | PskPromptSelect::Back => {
                    self.select.set(PskPromptSelect::Password);
                }
            },
            KeyAction::Left | KeyAction::Right => match selected {
                PskPromptSelect::Password => {
                    self.select.set(PskPromptSelect::Show);
                }
                PskPromptSelect::Show => {
                    self.select.set(PskPromptSelect::Password);
                }
                PskPromptSelect::Connect => {
                    self.select.set(PskPromptSelect::Back);
                }
                PskPromptSelect::Back => {
                    self.select.set(PskPromptSelect::Connect);
                }
            },
            KeyAction::Backspace => {
                if selected == &PskPromptSelect::Password {
                    self.password.pop();
                }
            }
            KeyAction::Enter => match selected {
                PskPromptSelect::Connect => {
                    let mut network = self.network.clone();
                    match &mut network.security {
                        NetworkSecurity::Wpa2 { passphrase } => {
                            *passphrase = self.password.clone();
                        }
                        NetworkSecurity::Wpa2Hex { psk_hex } => {
                            *psk_hex = self.password.clone();
                        }
                        NetworkSecurity::Wpa3Sae { password, .. } => {
                            *password = self.password.clone();
                        }
                        NetworkSecurity::Wpa3Transition { password } => {
                            *password = self.password.clone();
                        }
                        _ => {}
                    }

                    return StateResult::Command(StateCommand::NetworkAction(NetworkAction::Wifi(
                        WifiAction::Connect(network),
                    )));
                }
                PskPromptSelect::Back => {
                    self.password.pop();
                    return StateResult::Command(StateCommand::PopPrompt);
                }
                PskPromptSelect::Show => {
                    self.show_password = !self.show_password;
                }
                _ => {}
            },
            KeyAction::Char(c) => {
                if selected == &PskPromptSelect::Password {
                    self.password.push(*c);
                }
            }
            KeyAction::Paste(s) => {
                if selected == &PskPromptSelect::Password {
                    self.password.push_str(s);
                }
            }
            _ => {}
        }
        StateResult::Consumed
    }
}

#[derive(Debug, PartialEq)]
pub enum PopupType {
    General,
    Error,
}

#[derive(Debug)]
pub struct InfoPrompt {
    pub reason: String,
    pub kind: PopupType,
}

impl InfoPrompt {
    pub fn new(reason: String, kind: PopupType) -> Self {
        Self { reason, kind }
    }
}

impl Component for InfoPrompt {
    fn on_key(&mut self, key: &KeyAction, _ctx: &AppContext) -> StateResult {
        match key {
            KeyAction::Enter | KeyAction::Backspace => {
                StateResult::Command(StateCommand::PopPrompt)
            }
            _ => StateResult::Ignored,
        }
    }
}

#[derive(Debug)]
pub struct WpaInterfacePrompt {
    pub interface_cursor: Cursor,
    pub on_choice: bool,
    pub persist: bool,
    pub pending_remove: Option<String>,
}

impl Default for WpaInterfacePrompt {
    fn default() -> Self {
        Self {
            interface_cursor: Cursor::default(),
            on_choice: true,
            persist: false,
            pending_remove: None,
        }
    }
}

impl Component for WpaInterfacePrompt {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        let Some(ifaces) = &ctx.net_ctx.wpa_interfaces else {
            return StateResult::Consumed;
        };

        let ordered = WpaInterfacePrompt::ordered_iface_indicies(ifaces);

        if ordered.is_empty() {
            self.on_choice = true;
            self.interface_cursor.index = 0;
            self.pending_remove = None;
        } else if self.interface_cursor.index >= ordered.len() {
            self.interface_cursor.index = ordered.len() - 1;
        }

        match key {
            KeyAction::Enter => {
                if self.on_choice {
                    self.pending_remove = None;
                    self.persist = !self.persist;
                    return StateResult::Command(StateCommand::NetworkAction(NetworkAction::Wpa(
                        WpaAction::TogglePersist,
                    )));
                }

                let Some(&real_idx) = ordered.get(self.interface_cursor.index) else {
                    return StateResult::Consumed;
                };

                let Some(selected) = ifaces.get(real_idx) else {
                    return StateResult::Consumed;
                };

                match selected {
                    WpaInterface::Unmanaged(iface) => {
                        self.pending_remove = None;
                        StateResult::Command(StateCommand::NetworkAction(NetworkAction::Wpa(
                            WpaAction::CreateInterface(iface.clone()),
                        )))
                    }
                    WpaInterface::Managed(_) => {
                        let name = selected.name().to_string();

                        if self.pending_remove.as_deref() == Some(name.as_str()) {
                            self.pending_remove = None;
                            StateResult::Command(StateCommand::NetworkAction(NetworkAction::Wpa(
                                WpaAction::RemoveInterface(name),
                            )))
                        } else {
                            self.pending_remove = Some(name);
                            StateResult::Consumed
                        }
                    }
                }
            }
            // can't use generic cursor up down implmentation
            KeyAction::Up => {
                self.pending_remove = None;
                if self.on_choice {
                    if !ordered.is_empty() {
                        self.interface_cursor.index = ordered.len() - 1;
                        self.on_choice = false;
                    }
                } else if self.interface_cursor.index == 0 {
                    self.on_choice = true;
                } else {
                    self.interface_cursor.index -= 1;
                }
                StateResult::Consumed
            }
            KeyAction::Down => {
                self.pending_remove = None;
                if self.on_choice {
                    if !ordered.is_empty() {
                        self.interface_cursor.index = 0;
                        self.on_choice = false;
                    }
                } else if self.interface_cursor.index + 1 >= ordered.len() {
                    self.on_choice = true;
                } else {
                    self.interface_cursor.index += 1;
                }
                StateResult::Consumed
            }
            _ => StateResult::Consumed,
        }
    }
}

impl WpaInterfacePrompt {
    pub fn ordered_iface_indicies(ifaces: &[WpaInterface]) -> Vec<usize> {
        let mut unmanaged = vec![];
        let mut managed = vec![];

        for (idx, iface) in ifaces.iter().enumerate() {
            if iface.is_managed() {
                managed.push(idx);
            } else {
                unmanaged.push(idx);
            }
        }

        unmanaged.extend(managed);
        unmanaged
    }
}
