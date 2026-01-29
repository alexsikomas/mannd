use std::path::PathBuf;
use std::{fmt::Debug, usize};

use com::state::network::NetworkAction;
use com::wireguard::store::WgMeta;
use com::{
    controller::DaemonType,
    state::network::ApConnectInfoBuilder,
    wireless::common::{AccessPoint, NetworkFlags, Security},
};
use crossterm::event::{Event, KeyCode, KeyEvent};

use crate::app::AppAction;

#[derive(Debug)]
pub enum StateResult {
    Consumed,
    Command(StateCommand),
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
    pub networks: &'a [AccessPoint],
    pub wg_files: (&'a Vec<String>, &'a [WgMeta]),
    pub wifi_daemon: &'a Option<DaemonType>,
    pub vpn_cols: usize,
}

/// Data used for UI, may be sent to threads through
/// channels
pub struct UiState {
    // kbd input block
    pub should_block: bool,
    pub current_view: View,
    pub prompt_stack: Vec<PromptState>,
    pub vpn_cols: usize,
}

impl<'a> AppContext<'a> {
    pub fn create(
        networks: &'a [AccessPoint],
        wifi_daemon: &'a Option<DaemonType>,
        // don't take as a tuple with a name here
        // because meta index is direct map to name
        // index, vice versa
        wg_files: (&'a Vec<String>, &'a [WgMeta]),
        vpn_cols: usize,
    ) -> Self {
        Self {
            networks,
            wifi_daemon,
            wg_files,
            vpn_cols,
        }
    }
}

trait Component {
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult;
}

impl UiState {
    pub fn new() -> Self {
        UiState {
            should_block: false,
            current_view: View::main_menu(),
            prompt_stack: vec![],
            vpn_cols: 0,
        }
    }

    pub fn handle_event(&mut self, event: Event, ctx: &AppContext) -> Option<AppAction> {
        if self.should_block {
            return None;
        }

        if let Event::Key(key) = event {
            if key.code == KeyCode::Esc {
                if !self.prompt_stack.is_empty() {
                    self.prompt_stack.pop();
                    return None;
                }
            }

            if let Some(prompt) = self.prompt_stack.last_mut() {
                match prompt.on_key(&key, ctx) {
                    StateResult::Command(cmd) => return self.process_command(cmd),
                    _ => return None,
                }
            }

            let result = self.current_view.on_key(&key, ctx);
            if let StateResult::Command(cmd) = result {
                return self.process_command(cmd);
            }
        }
        None
    }

    /// May be used by external files to make something happen in the UI
    /// like adding a prompt based on something the UiState doesn't know
    /// directly
    pub fn process_command(&mut self, cmd: StateCommand) -> Option<AppAction> {
        match cmd {
            StateCommand::Exit => Some(AppAction::Exit),
            StateCommand::ChangeView(view) => {
                self.current_view = view;
                match self.current_view {
                    // improves usability by fetching any networks
                    // that are currently known in connection
                    View::Connection(_) => Some(AppAction::Network(NetworkAction::GetNetworks)),
                    View::Vpn(_) => Some(AppAction::Network(NetworkAction::InitWireguard)),
                    _ => None,
                }
            }
            StateCommand::Prompt(prompt) => {
                match &prompt {
                    PromptState::Info(info) => {
                        // removes clutter of general messages if we have error
                        if info.kind == PopupType::Error {
                            self.remove_general_prompts();
                        }
                    }
                    _ => {}
                };
                self.prompt_stack.push(prompt);
                None
            }
            StateCommand::Back => {
                self.current_view = View::main_menu();
                None
            }
            StateCommand::NetworkAction(action) => Some(AppAction::Network(action)),
            StateCommand::PopPrompt => {
                if !self.prompt_stack.is_empty() {
                    self.prompt_stack.pop();
                }
                None
            }
            StateCommand::ClearPrompts => {
                self.prompt_stack = vec![];
                None
            }
        }
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
    Connection(ConnectionState),
    Vpn(VpnState),
    Config,
}

impl View {
    pub fn main_menu() -> Self {
        View::MainMenu(SelectableList::new(vec![
            MainMenuSelection::Connection,
            MainMenuSelection::Vpn,
            MainMenuSelection::Config,
            MainMenuSelection::Exit,
        ]))
    }
}

impl Component for View {
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult {
        if key.code == KeyCode::Esc {
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

                if key.code == KeyCode::Enter {
                    if let Some(selection) = list.selected() {
                        return selection.execute();
                    }
                    return StateResult::Consumed;
                }
            }
            View::Connection(state) => return state.on_key(key, ctx),
            View::Vpn(state) => return state.on_key(key, ctx),
            View::Config => {}
        };
        StateResult::Ignored
    }
}

// Connection
//
// Possible actions the user can take in the connection
// menu and how the data should be stored
#[derive(Debug)]
pub struct ConnectionState {
    pub focused_area: ConnectionFocus,
    pub actions: SelectableList<ConnectionAction>,
    // selected network
    pub network_cursor: usize,
}

#[derive(Clone, Debug)]
pub enum ConnectionAction {
    Scan,
    Connect,
    Disconnect,
    // Info,
    Forget,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionFocus {
    Networks,
    Actions,
}

impl ConnectionState {
    pub fn new() -> Self {
        Self {
            focused_area: ConnectionFocus::Actions,
            actions: SelectableList::new(vec![ConnectionAction::Scan]),
            network_cursor: 0,
        }
    }

    pub fn refresh_available_actions(&mut self, networks: &[AccessPoint]) {
        self.actions = SelectableList::new(vec![ConnectionAction::Scan]);

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

impl Component for ConnectionState {
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult {
        // check if up or down
        match key.code {
            KeyCode::Right | KeyCode::Left => {
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
                if key.code == KeyCode::Enter {
                    return match self.actions.selected() {
                        Some(ConnectionAction::Scan) => {
                            StateResult::Command(StateCommand::NetworkAction(NetworkAction::Scan))
                        }
                        Some(ConnectionAction::Connect) => {
                            if let Some(network) = ctx.networks.get(self.network_cursor) {
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
                            if let Some(network) = ctx.networks.get(self.network_cursor) {
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
                        _ => StateResult::Consumed,
                    };
                }
            }
            ConnectionFocus::Networks => match key.code {
                KeyCode::Down => {
                    if !ctx.networks.is_empty() {
                        self.network_cursor = (self.network_cursor + 1) % ctx.networks.len();
                        self.refresh_available_actions(ctx.networks);
                    }
                }
                KeyCode::Up => {
                    if !ctx.networks.is_empty() {
                        if self.network_cursor == 0 {
                            self.network_cursor = ctx.networks.len() - 1;
                        } else {
                            self.network_cursor -= 1;
                        }
                        self.refresh_available_actions(ctx.networks);
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
            // Self::Info => "Info",
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
    /// Displays general info or errors to the user
    Info(InfoPrompt),
}

impl Component for PromptState {
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult {
        match self {
            PromptState::PskConnect(prompt) => {
                return prompt.on_key(key, ctx);
            }
            PromptState::Info(prompt) => {
                return prompt.on_key(key, ctx);
            }
        };
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
    fn on_key(&mut self, key: &KeyEvent, _ctx: &AppContext) -> StateResult {
        let Some(selected) = self.select.selected() else {
            return StateResult::Ignored;
        };

        match key.code {
            KeyCode::Up | KeyCode::Down => match selected {
                PskPromptSelect::Password | PskPromptSelect::Show => {
                    self.select.set(PskPromptSelect::Connect);
                }
                PskPromptSelect::Connect | PskPromptSelect::Back => {
                    self.select.set(PskPromptSelect::Password);
                }
            },
            KeyCode::Left | KeyCode::Right => match selected {
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
            KeyCode::Backspace => match selected {
                PskPromptSelect::Password => {
                    self.password.pop();
                }
                _ => {}
            },
            KeyCode::Enter => match selected {
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
            KeyCode::Char(c) => match selected {
                PskPromptSelect::Password => {
                    self.password.push(c);
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
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult {
        match key.code {
            KeyCode::Enter | KeyCode::Backspace => StateResult::Command(StateCommand::PopPrompt),
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
    fn on_key(&mut self, key: &KeyEvent, _ctx: &AppContext) -> StateResult {
        match key.code {
            KeyCode::Up => {
                self.prev();
                StateResult::Consumed
            }
            KeyCode::Down => {
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

#[derive(Debug, PartialEq)]
pub enum MainMenuSelection {
    Connection,
    Vpn,
    Config,
    Exit,
}

impl MainMenuSelection {
    fn execute(&self) -> StateResult {
        match self {
            Self::Connection => StateResult::Command(StateCommand::ChangeView(View::Connection(
                ConnectionState::new(),
            ))),
            Self::Config => StateResult::Command(StateCommand::ChangeView(View::Config)),
            Self::Vpn => StateResult::Command(StateCommand::ChangeView(View::Vpn(VpnState::new()))),
            Self::Exit => StateResult::Command(StateCommand::Exit),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connection => "Connection",
            Self::Vpn => "VPN",
            Self::Config => "Config",
            Self::Exit => "Exit",
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
            // Self::Connect => "Connect",
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
    pub wg_on: bool,
}

impl VpnState {
    fn new() -> Self {
        Self {
            selection: Self::get_actions(),
            file_cursor: 0,
            wg_on: false,
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
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult {
        if let Some(selected) = self.selection.selected() {
            match key.code {
                KeyCode::Enter => match selected {
                    VpnSelection::Toggle => {
                        self.wg_on = !self.wg_on;
                    }
                    VpnSelection::Files => {
                        let mut wg_path = PathBuf::from("/etc/wireguard");
                        wg_path.push(format!("{}.conf", &ctx.wg_files.0[self.file_cursor]));
                        return StateResult::Command(StateCommand::NetworkAction(
                            NetworkAction::ConnectWireguard(wg_path),
                        ));
                    }
                    _ => {}
                },
                KeyCode::Left => match selected {
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
                KeyCode::Right => {
                    match selected {
                        VpnSelection::Files => {
                            self.file_cursor = self.file_cursor.saturating_add(1);
                        }
                        VpnSelection::Filter => {
                            self.selection.selected_index = 0;
                        }
                        _ => {
                            self.selection.next();
                        }
                    };
                }
                KeyCode::Down => {
                    if selected == &VpnSelection::Files {
                        self.file_cursor = self.file_cursor.saturating_add(ctx.vpn_cols);
                    } else {
                        self.selection.set(VpnSelection::Files);
                    }
                }
                KeyCode::Up => {
                    if self.file_cursor < ctx.vpn_cols {
                        self.selection.selected_index = 0;
                    } else {
                        self.file_cursor = self.file_cursor.saturating_sub(ctx.vpn_cols);
                    }
                    // BUG: you must be at first file to go back instead
                    // it should work for the entire top row
                    // if selected == &VpnSelection::Files && self.file_cursor == 0 {
                    //     self.selection.selected_index = 0;
                    // }
                }
                _ => {}
            }
        }
        StateResult::Consumed
    }
}
