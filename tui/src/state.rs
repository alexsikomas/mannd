use std::path::PathBuf;
use std::sync::OnceLock;
use std::{fmt::Debug, usize};

use com::controller::DaemonType;
use com::state::network::{Capability, NetCtx, NetCtxFlags, NetworkAction};
use com::{
    state::network::ApConnectInfoBuilder,
    wireless::common::{AccessPoint, NetworkFlags, Security},
};
use crossterm::event::Event;

use crate::app::AppAction;
use crate::keys::{KeyAction, Keymap};

pub static KEYMAP: OnceLock<Keymap> = OnceLock::new();

#[derive(Debug)]
pub enum StateResult {
    Consumed,
    Command(StateCommand),
    Commands(Vec<StateCommand>),
    Ignored,
}

#[derive(Debug)]
pub enum StateCommand {
    Exit,
    ChangeView(View),
    Back,
    NetworkAction(NetworkAction),
    Prompt(PromptState),
    PopPrompt,
    ClearPrompts,
}

pub struct AppContext<'a> {
    pub net_ctx: &'a NetCtx,
    pub capabilities: &'a Capability,
    pub vpn_cols: usize,
}

pub struct UiState {
    // kbd input block
    pub should_block: bool,
    pub current_view: View,
    pub prompt_stack: Vec<PromptState>,
    pub vpn_cols: usize,
    pub caps: Capability,
}

impl<'a> AppContext<'a> {
    pub fn create(
        net_ctx: &'a NetCtx,
        capabilities: &'a Capability,
        // one-to-one map
        vpn_cols: usize,
    ) -> Self {
        Self {
            net_ctx,
            capabilities,
            vpn_cols,
        }
    }
}

trait Component {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult;
}

impl UiState {
    pub fn new(caps: Capability) -> Self {
        let keymap = Keymap::load_keys();

        match KEYMAP.set(keymap) {
            Ok(_) => {}
            Err(_) => {
                tracing::warn!("Keymap has already been initialised");
            }
        };

        UiState {
            should_block: false,
            current_view: View::main_menu(&caps),
            prompt_stack: vec![],
            vpn_cols: 0,
            caps,
        }
    }

    pub fn refresh_view(&mut self) {
        match &self.current_view {
            View::MainMenu(_) => {
                self.current_view = View::main_menu(&self.caps);
            }
            _ => {}
        }
    }

    pub fn handle_event(&mut self, event: Event, ctx: &AppContext) -> Vec<AppAction> {
        if self.should_block {
            return vec![];
        }
        let keymap = KEYMAP.get().unwrap();

        let key_action = match event {
            Event::Key(key) => match keymap.bindings.get(&key) {
                Some(key) => key,
                None => {
                    if let Some(c) = key.code.as_char() {
                        &KeyAction::Char(c)
                    } else {
                        &KeyAction::None
                    }
                }
            },
            _ => &KeyAction::None,
        };

        if key_action == &KeyAction::Escape {
            if !self.prompt_stack.is_empty() {
                self.prompt_stack.pop();
                return vec![];
            }
        }

        if let Some(prompt) = self.prompt_stack.last_mut() {
            let res = prompt.on_key(key_action, ctx);
            match res {
                StateResult::Command(cmd) => {
                    return self.process_commands([cmd]);
                }
                StateResult::Commands(cmds) => {
                    return self.process_commands(cmds);
                }
                StateResult::Consumed => return vec![],
                StateResult::Ignored => {}
            };
        }

        let res = self.current_view.on_key(key_action, ctx);
        if let StateResult::Command(cmd) = res {
            return self.process_commands([cmd]);
        } else if let StateResult::Commands(cmds) = res {
            return self.process_commands(cmds);
        }
        vec![]
    }

    pub fn process_commands(
        &mut self,
        cmds: impl IntoIterator<Item = StateCommand>,
    ) -> Vec<AppAction> {
        let mut actions: Vec<AppAction> = vec![];
        for cmd in cmds {
            match cmd {
                StateCommand::Exit => actions.push(AppAction::Exit),
                StateCommand::ChangeView(view) => {
                    self.current_view = view;
                    match self.current_view {
                        // improves usability by fetching any networks
                        // that are currently known in connection
                        View::Wifi(_) => actions.push(AppAction::Network(
                            NetworkAction::GetNetworkContext(NetCtxFlags::Network),
                        )),
                        // View::Vpn(_) => {
                        //     actions.push(AppAction::Network(NetworkAction::))
                        // }
                        _ => {}
                    };
                }
                StateCommand::Prompt(prompt) => {
                    match &prompt {
                        PromptState::Info(info) => {
                            // removes clutter of general messages
                            // if there is an error
                            if info.kind == PopupType::Error {
                                self.remove_general_prompts();
                            }
                        }
                        _ => {}
                    };
                    self.prompt_stack.push(prompt);
                }
                StateCommand::Back => {
                    self.current_view = View::main_menu(&self.caps);
                }
                StateCommand::NetworkAction(action) => actions.push(AppAction::Network(action)),
                StateCommand::PopPrompt => {
                    if !self.prompt_stack.is_empty() {
                        self.prompt_stack.pop();
                    }
                }
                StateCommand::ClearPrompts => {
                    self.prompt_stack = vec![];
                }
            };
        }
        actions
    }

    fn remove_general_prompts(&mut self) {
        self.prompt_stack.retain(|prompt| match prompt {
            PromptState::Info(info) => {
                if info.kind == PopupType::General {
                    false
                } else {
                    true
                }
            }
            _ => true,
        });
    }
}

#[derive(Debug)]
pub enum View {
    MainMenu(SelectableList<MainMenuSelection>),
    Wifi(WifiState),
    Vpn(VpnState),
    Networkd(NetdState),
    Config,
}

impl View {
    pub fn main_menu(caps: &Capability) -> Self {
        let mut options: SelectableList<MainMenuSelection> = SelectableList::new(vec![]);
        // possibly no interface
        if caps.wifi_daemon.is_some() {
            options.items.push(MainMenuSelection::Wifi);
        }

        if caps.wireguard.0 {
            options.items.push(MainMenuSelection::Vpn);
        }

        if caps.networkd_active {
            options.items.push(MainMenuSelection::Networkd);
        }

        options
            .items
            .extend([MainMenuSelection::Config, MainMenuSelection::Exit]);
        Self::MainMenu(options)
    }
}

impl Component for View {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        if key == &KeyAction::Escape {
            return match self {
                View::MainMenu(_) => StateResult::Command(StateCommand::Exit),
                _ => StateResult::Command(StateCommand::Back),
            };
        }

        match self {
            View::MainMenu(list) => {
                if list.on_key(key, ctx).is_consumed() {
                    return StateResult::Consumed;
                }

                if key == &KeyAction::Enter {
                    if let Some(selection) = list.selected() {
                        return selection.execute(&ctx.capabilities.wifi_daemon);
                    }
                    return StateResult::Consumed;
                }
            }
            View::Wifi(state) => return state.on_key(key, ctx),
            View::Vpn(state) => return state.on_key(key, ctx),
            View::Networkd(_state) => {}
            View::Config => {}
        };
        StateResult::Ignored
    }
}

#[derive(Debug, PartialEq)]
pub enum MainMenuSelection {
    Wifi,
    Vpn,
    Networkd,
    Config,
    Exit,
}

impl MainMenuSelection {
    fn execute(&self, daemon: &Option<DaemonType>) -> StateResult {
        match self {
            Self::Wifi => {
                // DaemonType safe here
                StateResult::Command(StateCommand::ChangeView(View::Wifi(WifiState::new(
                    daemon.as_ref().unwrap(),
                ))))
            }
            Self::Config => StateResult::Command(StateCommand::ChangeView(View::Config)),
            Self::Networkd => {
                StateResult::Command(StateCommand::ChangeView(View::Networkd(NetdState::new())))
            }
            Self::Vpn => StateResult::Command(StateCommand::ChangeView(View::Vpn(VpnState::new()))),
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

// Connection
//
// Possible actions the user can take in the connection menu
#[derive(Debug)]
pub struct WifiState {
    pub focused_area: ConnectionFocus,
    pub actions: SelectableList<ConnectionAction>,
    // selected network
    pub network_cursor: usize,
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
    pub fn new(daemon: &DaemonType) -> Self {
        let mut actions = SelectableList::new(vec![ConnectionAction::Scan]);
        if daemon == &DaemonType::Wpa {
            actions.items.push(ConnectionAction::Interfaces);
        }

        Self {
            focused_area: ConnectionFocus::Actions,
            actions,
            network_cursor: 0,
        }
    }

    pub fn refresh_available_actions(&mut self, networks: &[AccessPoint]) {
        let perma_actions = ConnectionAction::perma_actions();
        self.actions.items.retain(|a| perma_actions.contains(a));

        if let Some(ap) = networks.get(self.network_cursor) {
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
        };

        match self.focused_area {
            ConnectionFocus::Actions => {
                self.actions.on_key(key, ctx);
                if key == &KeyAction::Enter {
                    return match self.actions.selected() {
                        Some(ConnectionAction::Scan) => {
                            StateResult::Command(StateCommand::NetworkAction(NetworkAction::Scan))
                        }
                        Some(ConnectionAction::Connect) => {
                            if let Some(network) = net_ctx.networks.get(self.network_cursor) {
                                if network.flags.contains(NetworkFlags::KNOWN) {
                                    return StateResult::Command(StateCommand::NetworkAction(
                                        NetworkAction::ConnectKnown(
                                            network.ssid.clone(),
                                            network.security.clone(),
                                        ),
                                    ));
                                }

                                match network.security {
                                    Security::Psk => {
                                        let prompt = PskConnectionPrompt::new(network.ssid.clone());
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
                        Some(ConnectionAction::Disconnect) => StateResult::Command(
                            StateCommand::NetworkAction(NetworkAction::Disconnect),
                        ),
                        Some(ConnectionAction::Forget) => {
                            if let Some(network) = net_ctx.networks.get(self.network_cursor) {
                                if network.flags.contains(NetworkFlags::KNOWN) {
                                    return StateResult::Command(StateCommand::NetworkAction(
                                        NetworkAction::Forget(
                                            network.ssid.clone(),
                                            network.security.clone(),
                                        ),
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
                                WpaInterfacePrompt::new(),
                            )));
                            cmds.push(StateCommand::NetworkAction(
                                NetworkAction::GetNetworkContext(NetCtxFlags::InterfacesWpa),
                            ));
                            return StateResult::Commands(cmds);
                        }
                        _ => StateResult::Consumed,
                    };
                }
            }
            ConnectionFocus::Networks => match key {
                KeyAction::Down => {
                    if !ctx.net_ctx.networks.is_empty() {
                        self.network_cursor = (self.network_cursor + 1) % net_ctx.networks.len();
                        self.refresh_available_actions(&net_ctx.networks);
                    }
                }
                KeyAction::Up => {
                    if !net_ctx.networks.is_empty() {
                        if self.network_cursor == 0 {
                            self.network_cursor = net_ctx.networks.len() - 1;
                        } else {
                            self.network_cursor -= 1;
                        }
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

// Prompt
//
// Displays a prompt visually on
// top of the current view
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
            PromptState::PskConnect(prompt) => {
                return prompt.on_key(key, ctx);
            }
            PromptState::Info(prompt) => {
                return prompt.on_key(key, ctx);
            }
            PromptState::WpaInterface(prompt) => {
                return prompt.on_key(key, ctx);
            }
            _ => {}
        };
        StateResult::Consumed
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
    fn new(ssid: String) -> Self {
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
            KeyAction::Backspace => match selected {
                PskPromptSelect::Password => {
                    self.password.pop();
                }
                _ => {}
            },
            KeyAction::Enter => match selected {
                PskPromptSelect::Connect => {
                    let ap_info = ApConnectInfoBuilder::default()
                        .ssid(self.ssid.clone())
                        .credentials(com::state::network::Credentials::Password(
                            self.password.clone(),
                        ))
                        .security(Security::Psk)
                        .build()
                        .unwrap();
                    return StateResult::Command(StateCommand::NetworkAction(
                        NetworkAction::Connect(ap_info),
                    ));
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
            KeyAction::Char(c) => match selected {
                PskPromptSelect::Password => {
                    self.password.push(*c);
                }
                _ => {}
            },
            _ => {}
        };
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
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        match key {
            KeyAction::Enter | KeyAction::Backspace => {
                StateResult::Command(StateCommand::PopPrompt)
            }
            _ => StateResult::Ignored,
        }
    }
}

impl StateResult {
    pub fn is_consumed(&self) -> bool {
        matches!(self, StateResult::Consumed)
    }
}

// Generic data structure used to keep
// track of menu items
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SelectableList<T> {
    pub items: Vec<T>,
    pub selected_index: usize,
}

impl<T> SelectableList<T> {
    pub fn new(v: Vec<T>) -> Self {
        Self {
            items: v,
            selected_index: 0,
        }
    }
    pub fn selected(&self) -> Option<&T> {
        self.items.get(self.selected_index)
    }

    fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected_index = { self.selected_index + 1 } % self.items.len();
    }

    fn prev(&mut self) {
        if self.items.is_empty() {
            return;
        }

        if self.selected_index == 0 {
            self.selected_index = self.items.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }
}

impl<T> Component for SelectableList<T> {
    fn on_key(&mut self, key: &KeyAction, _ctx: &AppContext) -> StateResult {
        match key {
            KeyAction::Up => {
                self.prev();
                StateResult::Consumed
            }
            KeyAction::Down => {
                self.next();
                StateResult::Consumed
            }
            _ => StateResult::Ignored,
        }
    }
}

impl<T: PartialEq> SelectableList<T> {
    pub fn set(&mut self, item: T) {
        if let Some(index) = self.items.iter().position(|x| *x == item) {
            self.selected_index = index;
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum VpnSelection {
    // Connect,
    Toggle,
    Scan,
    Country,
    Filter,
    // isn't a menu option but rather a section
    Files,
}

impl VpnSelection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Toggle => "Toggle",
            Self::Scan => "Scan Files",
            Self::Country => "Get Countries",
            Self::Filter => "Filter",
            Self::Files => "",
        }
    }
}

#[derive(Debug)]
pub struct VpnState {
    pub selection: SelectableList<VpnSelection>,
    pub file_cursor: usize,
}

impl VpnState {
    fn new() -> Self {
        Self {
            selection: Self::get_actions(),
            file_cursor: 0,
        }
    }

    fn get_actions() -> SelectableList<VpnSelection> {
        SelectableList::new(vec![
            VpnSelection::Toggle,
            VpnSelection::Scan,
            VpnSelection::Country,
            VpnSelection::Filter,
            VpnSelection::Files,
        ])
    }
}

impl Component for VpnState {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        if let Some(selected) = self.selection.selected() {
            match key {
                KeyAction::Enter => match selected {
                    VpnSelection::Toggle => {
                        return StateResult::Command(StateCommand::NetworkAction(
                            NetworkAction::ToggleWireguard,
                        ));
                    }
                    VpnSelection::Files => {
                        let mut wg_path = PathBuf::from("/etc/wireguard");
                        if let Some(data) = ctx.net_ctx.wg_ctx.get_index(self.file_cursor) {
                            wg_path.push(format!("{}", data.0));
                            return StateResult::Command(StateCommand::NetworkAction(
                                NetworkAction::ConnectWireguard(wg_path),
                            ));
                        }
                    }
                    _ => {}
                },
                KeyAction::Left => match selected {
                    VpnSelection::Files => {
                        self.file_cursor = self.file_cursor.saturating_sub(1);
                    }
                    VpnSelection::Toggle => {
                        self.selection.selected_index = self.selection.items.len() - 2;
                    }
                    _ => {
                        self.selection.prev();
                    }
                },
                KeyAction::Right => {
                    match selected {
                        VpnSelection::Files => {
                            self.file_cursor = self
                                .file_cursor
                                .saturating_add(1)
                                .min(ctx.net_ctx.wg_ctx.len() - 1);
                        }
                        VpnSelection::Filter => {
                            self.selection.selected_index = 0;
                        }
                        _ => {
                            self.selection.next();
                        }
                    };
                }
                KeyAction::Down => {
                    if selected == &VpnSelection::Files {
                        self.file_cursor = self
                            .file_cursor
                            .saturating_add(ctx.vpn_cols)
                            .min(ctx.net_ctx.wg_ctx.len() - 1);
                    } else {
                        self.selection.set(VpnSelection::Files);
                    }
                }
                KeyAction::Up => {
                    if self.file_cursor < ctx.vpn_cols {
                        self.selection.selected_index = 0;
                    } else {
                        self.file_cursor = self.file_cursor.saturating_sub(ctx.vpn_cols);
                    }
                }
                _ => {}
            }
        }
        StateResult::Consumed
    }
}

#[derive(Debug)]
pub struct NetdState {
    pub selection: SelectableList<NetdSelection>,
    pub config_cursor: usize,
}

#[derive(Debug)]
pub enum NetdSelection {
    Configs,
    Create,
}

impl NetdState {
    fn new() -> Self {
        Self {
            selection: SelectableList::new(Self::get_actions()),
            config_cursor: 0,
        }
    }

    fn get_actions() -> Vec<NetdSelection> {
        vec![NetdSelection::Configs, NetdSelection::Create]
    }
}

#[derive(Debug)]
pub struct WpaInterfacePrompt {
    pub interface_cursor: usize,
    pub on_choice: bool,
}

impl WpaInterfacePrompt {
    fn new() -> Self {
        Self {
            interface_cursor: 0,
            on_choice: true,
        }
    }
}

impl Component for WpaInterfacePrompt {
    fn on_key(&mut self, key: &KeyAction, ctx: &AppContext) -> StateResult {
        if let Some(ifaces) = &ctx.net_ctx.interfaces {
            match key {
                KeyAction::Enter => {
                    if self.on_choice {
                        return StateResult::Command(StateCommand::NetworkAction(
                            NetworkAction::ToggleWpaPersist,
                        ));
                    } else {
                        if let Some(iface) = ifaces.wpa_get(self.interface_cursor) {
                            return StateResult::Command(StateCommand::NetworkAction(
                                NetworkAction::CreateWpaInterface(iface.into()),
                            ));
                        }
                    }
                }
                KeyAction::Up => {
                    if self.on_choice {
                        self.interface_cursor = ifaces.len() - 1;
                        self.on_choice = false;
                    } else {
                        if self.interface_cursor == 0 {
                            self.interface_cursor = 0;
                            self.on_choice = true;
                        } else {
                            self.interface_cursor = self.interface_cursor.saturating_sub(1);
                        }
                    }
                }
                KeyAction::Down => {
                    if self.on_choice {
                        self.on_choice = false;
                        self.interface_cursor = 0;
                    } else {
                        self.interface_cursor = { self.interface_cursor + 1 } % ifaces.len();
                    }
                }
                _ => {}
            };
        }

        StateResult::Consumed
    }
}
