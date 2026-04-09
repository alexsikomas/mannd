use mannd::{
    controller::WifiDaemonType,
    state::messages::{NetworkAction, WifiAction, WpaAction},
    store::{NetworkInfo, NetworkSecurity, PmfMode, SaePwe},
    wireless::wpa_supplicant::WpaInterface,
};

use crate::{
    keys::KeyAction,
    state::{AppContext, Component, Cursor, SelectableList, StateCommand, StateResult, TextInput},
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

#[derive(Eq, Clone, Debug, PartialEq)]
pub enum PskPromptField {
    Password,
    Show,
    AdvancedToggle,
    AdvSetting(AdvPskSetting),
    Connect,
    Back,
}

impl PskPromptField {
    fn get_fields(is_wpa3: bool, adv_open: bool, daemon: &WifiDaemonType) -> Vec<PskPromptField> {
        let mut fields = vec![
            PskPromptField::Password,
            PskPromptField::Show,
            PskPromptField::AdvancedToggle,
        ];

        if adv_open && *daemon == WifiDaemonType::Wpa {
            fields.push(PskPromptField::AdvSetting(AdvPskSetting::Bssid));
            fields.push(PskPromptField::AdvSetting(AdvPskSetting::BssidBlacklist));
            fields.push(PskPromptField::AdvSetting(AdvPskSetting::Pmf));

            if is_wpa3 {
                fields.push(PskPromptField::AdvSetting(AdvPskSetting::SaePwe));
            }
        }

        fields.push(PskPromptField::Connect);
        fields.push(PskPromptField::Back);
        fields
    }
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub enum AdvPskSetting {
    Bssid,
    BssidBlacklist,
    Pmf,
    SaePwe,
}

#[derive(Debug, Default)]
pub struct PskAdvInput {
    pub bssid: TextInput,
    pub bssid_blacklist: TextInput,
    pub pmf: Option<PmfMode>,
    pub sae_pwe: Option<SaePwe>,
}

#[derive(Debug)]
pub struct PskConnectionPrompt {
    pub network: NetworkInfo,
    pub daemon: WifiDaemonType,
    pub password: TextInput,
    pub show_password: bool,
    pub advanced_open: bool,
    pub advanced_inputs: PskAdvInput,
    pub select: SelectableList<PskPromptField>,
}

impl PskConnectionPrompt {
    pub fn new(network: NetworkInfo, daemon: WifiDaemonType) -> Self {
        let mut advanced = PskAdvInput::default();
        if let Some(bssid) = &network.bssid {
            advanced.bssid = TextInput::with_value(bssid.clone());
        }

        if !network.bssid_blacklist.is_empty() {
            advanced.bssid_blacklist = TextInput::with_value(network.bssid_blacklist.join(", "));
        }

        advanced.pmf = network.pmf.clone();

        if let NetworkSecurity::Wpa3Sae { pwe, .. } = &network.security {
            advanced.sae_pwe = pwe.clone();
        }

        let select = SelectableList::new(PskPromptField::get_fields(
            matches!(network.security, NetworkSecurity::Wpa3Sae { .. }),
            false,
            &daemon,
        ));

        Self {
            network,
            daemon,
            password: TextInput::new(),
            show_password: false,
            advanced_open: false,
            advanced_inputs: advanced,
            select,
        }
    }

    pub fn advanced_fields(&self) -> Vec<AdvPskSetting> {
        self.select
            .items
            .iter()
            .filter_map(|f| match f {
                PskPromptField::AdvSetting(setting) => Some(setting.clone()),
                _ => None,
            })
            .collect()
    }

    fn backspace(&mut self) {
        match self.select.selected() {
            Some(PskPromptField::Password) => self.password.backspace(),
            Some(PskPromptField::AdvSetting(setting)) => match setting {
                AdvPskSetting::Bssid => self.advanced_inputs.bssid.backspace(),
                AdvPskSetting::BssidBlacklist => self.advanced_inputs.bssid_blacklist.backspace(),
                _ => {}
            },
            _ => {}
        }
    }

    fn push_text(&mut self, s: &str) {
        match self.select.selected() {
            Some(PskPromptField::Password) => self.password.push_str(s),
            Some(PskPromptField::AdvSetting(setting)) => match setting {
                AdvPskSetting::Bssid => self.advanced_inputs.bssid.push_str(s),
                AdvPskSetting::BssidBlacklist => self.advanced_inputs.bssid_blacklist.push_str(s),
                _ => {}
            },
            _ => {}
        }
    }

    fn cycle_option<T: Clone + PartialEq>(
        current: &mut Option<T>,
        options: &[Option<T>],
        forward: bool,
    ) {
        let cur_idx = options.iter().position(|opt| opt == current).unwrap_or(0);

        let next_idx = if forward {
            (cur_idx + 1) % options.len()
        } else {
            cur_idx.checked_sub(1).unwrap_or(options.len() - 1)
        };

        *current = options[next_idx].clone();
    }

    fn cycle_pmf(&mut self, forward: bool) {
        let opts = [
            None,
            Some(PmfMode::Disabled),
            Some(PmfMode::Optional),
            Some(PmfMode::Required),
        ];
        Self::cycle_option(&mut self.advanced_inputs.pmf, &opts, forward);
    }

    fn cycle_sae_pwe(&mut self, forward: bool) {
        let opts = [
            None,
            Some(SaePwe::HuntAndPeck),
            Some(SaePwe::HashToElement),
            Some(SaePwe::Both),
        ];
        Self::cycle_option(&mut self.advanced_inputs.sae_pwe, &opts, forward);
    }

    fn apply_password(&self, network: &mut NetworkInfo) {
        match &mut network.security {
            NetworkSecurity::Wpa2 { passphrase } => {
                *passphrase = self.password.value.clone();
            }
            NetworkSecurity::Wpa2Hex { psk_hex } => {
                *psk_hex = self.password.value.clone();
            }
            NetworkSecurity::Wpa3Sae { password, .. } => {
                *password = self.password.value.clone();
            }
            NetworkSecurity::Wpa3Transition { password } => {
                *password = self.password.value.clone();
            }
            _ => {}
        }
    }

    fn apply_advanced(&self, network: &mut NetworkInfo) {
        if !matches!(self.daemon, WifiDaemonType::Wpa) {
            return;
        }

        let bssid = self.advanced_inputs.bssid.value.trim();
        network.bssid = if bssid.is_empty() {
            None
        } else {
            Some(bssid.to_string())
        };
        network.bssid_blacklist = self
            .advanced_inputs
            .bssid_blacklist
            .value
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<String>>();

        network.pmf = self.advanced_inputs.pmf.clone();
        if let NetworkSecurity::Wpa3Sae { pwe, .. } = &mut network.security {
            *pwe = self.advanced_inputs.sae_pwe.clone();
        }
    }
}

impl Component for PskConnectionPrompt {
    fn on_key(&mut self, key: &KeyAction, _ctx: &AppContext) -> StateResult {
        let Some(selected) = self.select.selected() else {
            return StateResult::Ignored;
        };

        match key {
            KeyAction::Up => match selected {
                PskPromptField::Password | PskPromptField::Show => {
                    self.select.set(PskPromptField::Connect);
                }
                PskPromptField::Connect | PskPromptField::Back => {
                    if self.advanced_open {
                        let adv = self.advanced_fields();
                        let Some(last) = adv.last() else {
                            return StateResult::Consumed;
                        };

                        self.select.set(PskPromptField::AdvSetting(last.clone()));
                    } else {
                        self.select.set(PskPromptField::AdvancedToggle);
                    }
                }
                PskPromptField::AdvancedToggle => {
                    self.select.set(PskPromptField::Password);
                }
                _ => {
                    self.select.prev();
                }
            },
            KeyAction::Down => match selected {
                PskPromptField::Password | PskPromptField::Show => {
                    let Some(idx) = self
                        .select
                        .items
                        .iter()
                        .position(|i| *i == PskPromptField::Show)
                    else {
                        return StateResult::Consumed;
                    };
                    self.select.selected_index = idx + 1;
                }
                PskPromptField::Connect | PskPromptField::Back => {
                    self.select.set(PskPromptField::Password);
                }
                _ => {
                    self.select.next();
                }
            },
            KeyAction::Left | KeyAction::Right => {
                let forward = key == &KeyAction::Right;
                match selected {
                    PskPromptField::Connect => {
                        self.select.set(PskPromptField::Back);
                    }
                    PskPromptField::Back => {
                        self.select.set(PskPromptField::Connect);
                    }
                    PskPromptField::Password => {
                        self.select.set(PskPromptField::Show);
                    }
                    PskPromptField::Show => {
                        self.select.set(PskPromptField::Password);
                    }
                    PskPromptField::AdvSetting(setting) => match setting {
                        AdvPskSetting::Pmf => {
                            self.cycle_pmf(forward);
                        }
                        AdvPskSetting::SaePwe => {
                            self.cycle_sae_pwe(forward);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            KeyAction::Backspace => {
                self.backspace();
            }
            KeyAction::Char(c) => {
                let mut s = String::with_capacity(1);
                s.push(*c);
                self.push_text(&s);
            }
            KeyAction::Paste(s) => {
                self.push_text(s);
            }
            KeyAction::Enter => match selected {
                PskPromptField::Connect => {
                    let mut network = self.network.clone();
                    self.apply_password(&mut network);
                    self.apply_advanced(&mut network);
                    return StateResult::Command(StateCommand::NetworkAction(NetworkAction::Wifi(
                        WifiAction::Connect(network),
                    )));
                }
                PskPromptField::Back => {
                    return StateResult::Command(StateCommand::PopPrompt);
                }
                PskPromptField::Show => {
                    self.show_password = !self.show_password;
                }
                PskPromptField::AdvancedToggle => {
                    self.advanced_open = !self.advanced_open;
                    let is_wpa3 = matches!(self.network.security, NetworkSecurity::Wpa3Sae { .. });
                    let previous = self.select.selected().cloned();

                    self.select.items =
                        PskPromptField::get_fields(is_wpa3, self.advanced_open, &self.daemon);

                    if let Some(prev) = previous
                        && let Some(index) = self.select.items.iter().position(|f| *f == prev)
                    {
                        self.select.selected_index = index;
                    } else {
                        self.select.selected_index = self
                            .select
                            .selected_index
                            .min(self.select.items.len().saturating_sub(1));
                    }
                }
                PskPromptField::AdvSetting(setting) => match setting {
                    AdvPskSetting::Pmf => {
                        self.cycle_pmf(true);
                    }
                    AdvPskSetting::SaePwe => {
                        self.cycle_sae_pwe(true);
                    }
                    _ => {}
                },
                _ => {}
            },
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
