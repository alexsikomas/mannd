use mannd::{
    state::messages::{ApConnectInfoBuilder, Credentials, NetworkAction, WifiAction, WpaAction},
    wireless::{common::Security, wpa_supplicant::WpaInterface},
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
    pub ssid: String,
    pub password: String,
    pub select: SelectableList<PskPromptSelect>,
    pub show_password: bool,
}

impl PskConnectionPrompt {
    pub fn new(ssid: String) -> Self {
        Self {
            ssid,
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
                    let ap_info = ApConnectInfoBuilder::default()
                        .ssid(self.ssid.clone())
                        .credentials(Credentials::Password(self.password.clone()))
                        .security(Security::Psk)
                        .build()
                        .unwrap();
                    return StateResult::Command(StateCommand::NetworkAction(NetworkAction::Wifi(
                        WifiAction::Connect(ap_info),
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
}

impl Default for WpaInterfacePrompt {
    fn default() -> Self {
        Self {
            interface_cursor: Cursor::default(),
            on_choice: true,
            persist: false,
        }
    }
}

impl Component for WpaInterfacePrompt {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        if let Some(ifaces) = &ctx.net_ctx.wpa_interfaces {
            match key {
                KeyAction::Enter => {
                    if self.on_choice {
                        self.persist = !self.persist;
                        return StateResult::Command(StateCommand::NetworkAction(
                            NetworkAction::Wpa(WpaAction::TogglePersist),
                        ));
                    } else if let Some(WpaInterface::Unmanaged(iface)) =
                        ifaces.get(self.interface_cursor.index)
                    {
                        return StateResult::Command(StateCommand::NetworkAction(
                            NetworkAction::Wpa(WpaAction::CreateInterface(iface.into())),
                        ));
                    }
                }
                KeyAction::Up => {
                    if self.on_choice && !ifaces.is_empty() {
                        self.interface_cursor.index = ifaces.len() - 1;
                        self.on_choice = false;
                    } else if self.interface_cursor.index == 0 {
                        self.on_choice = true;
                    } else {
                        self.interface_cursor.prev(ifaces.len());
                    }
                }
                KeyAction::Down => {
                    if self.on_choice && !ifaces.is_empty() {
                        self.interface_cursor.index = 0;
                        self.on_choice = false;
                    } else {
                        self.interface_cursor.next(ifaces.len());
                    }
                }
                _ => {}
            }
        }

        StateResult::Consumed
    }
}
