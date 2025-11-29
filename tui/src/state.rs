use std::{fmt::Debug, usize};

use com::{
    controller::DaemonType,
    wireless::common::{AccessPoint, NetworkFlags, Security},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use derive_builder::Builder;
use tracing::info;

use com::state::network::NetworkAction;

use crate::app::AppAction;

pub enum StateResult {
    Consumed,
    Command(AppCommand),
    Ignored,
}

pub enum AppCommand {
    Exit,
    ChangeView(View),
    Back,
    NetworkAction(NetworkAction),
    Prompt(PromptState),
}

pub struct AppContext<'a> {
    pub networks: &'a [AccessPoint],
    pub wifi_daemon: &'a Option<DaemonType>,
}

/// Data used for UI, may be sent to threads through
/// channels
pub struct UiState {
    pub current_view: View,
    pub prompt_stack: Vec<PromptState>,
}

impl<'a> AppContext<'a> {
    pub fn create(networks: &'a [AccessPoint], wifi_daemon: &'a Option<DaemonType>) -> Self {
        Self {
            networks,
            wifi_daemon,
        }
    }
}

trait Component {
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult;
}

impl UiState {
    pub fn new() -> Self {
        UiState {
            current_view: View::main_menu(),
            prompt_stack: vec![],
        }
    }

    pub fn handle_event(&mut self, event: Event, ctx: &AppContext) -> Option<AppAction> {
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

    fn process_command(&mut self, cmd: AppCommand) -> Option<AppAction> {
        match cmd {
            AppCommand::Exit => Some(AppAction::Exit),
            AppCommand::ChangeView(view) => {
                self.current_view = view;
                None
            }
            AppCommand::Prompt(prompt) => {
                self.prompt_stack.push(prompt);
                None
            }
            AppCommand::Back => {
                self.current_view = View::main_menu();
                None
            }
            AppCommand::NetworkAction(action) => Some(AppAction::Network(action)),
        }
    }
}

#[derive(Debug)]
pub enum View {
    MainMenu(SelectableList<MainMenuSelection>),
    Connection(ConnectionState),
    Vpn,
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
                View::MainMenu(_) => StateResult::Command(AppCommand::Exit),
                _ => StateResult::Command(AppCommand::Back),
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
            View::Vpn => {}
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
                            StateResult::Command(AppCommand::NetworkAction(NetworkAction::Scan))
                        }
                        Some(ConnectionAction::Connect) => {
                            if let Some(network) = ctx.networks.get(self.network_cursor) {
                                if network.flags.contains(NetworkFlags::KNOWN) {
                                    return StateResult::Command(AppCommand::NetworkAction(
                                        NetworkAction::ConnectKnown(
                                            network.ssid.clone(),
                                            network.security.clone(),
                                        ),
                                    ));
                                }

                                match network.security {
                                    Security::Psk => {
                                        let prompt = PskConnectionPrompt::new(network.ssid.clone());
                                        StateResult::Command(AppCommand::Prompt(
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
                            AppCommand::NetworkAction(NetworkAction::Disconnect),
                        ),
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
}

impl Component for PromptState {
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult {
        match self {
            PromptState::PskConnect(prompt) => {
                // movement is more granular here
                return prompt.on_key(key, ctx);
            }
        };
        StateResult::Ignored
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
}

impl PskConnectionPrompt {
    fn new(ssid: String) -> Self {
        Self {
            ssid,
            password: String::new(),
            select: SelectableList::new(PskPromptSelect::as_vec()),
        }
    }
}

impl Component for PskConnectionPrompt {
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult {
        let Some(selected) = self.select.selected() else {
            return StateResult::Ignored;
        };

        match key.code {
            KeyCode::Up | KeyCode::Down => match selected {
                PskPromptSelect::Password => {
                    self.select.set(PskPromptSelect::Connect);
                }
                PskPromptSelect::Connect | PskPromptSelect::Back => {
                    self.select.set(PskPromptSelect::Password);
                }
                _ => {}
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
    fn on_key(&mut self, key: &KeyEvent, ctx: &AppContext) -> StateResult {
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
            Self::Connection => StateResult::Command(AppCommand::ChangeView(View::Connection(
                ConnectionState::new(),
            ))),
            Self::Config => StateResult::Command(AppCommand::ChangeView(View::Config)),
            Self::Vpn => StateResult::Command(AppCommand::ChangeView(View::Vpn)),
            Self::Exit => StateResult::Command(AppCommand::Exit),
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
