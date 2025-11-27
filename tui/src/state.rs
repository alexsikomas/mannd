//!
//! State makes heavy use of on_key() functions,
//! these functions process a KeyEvent and some
//! may return a boolean representing if they
//! should exit the handle loop early to stop
//! unexpected beahaviour
//!

use std::{fmt::Debug, marker::PhantomData, usize};

use com::{
    controller::DaemonType,
    state::network::{ApConnectInfo, ApConnectInfoBuilder},
    wireless::common::{AccessPoint, NetworkFlags, Security},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use derive_builder::Builder;
use tracing::info;

use com::state::network::NetworkAction;

use crate::app::AppAction;

/// Data used for UI, may be sent to threads through
/// channels
#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct UiData {
    // Allows saving network list when leaving menu
    #[builder(default = "SelectableList::new(vec![])")]
    pub networks: SelectableList<AccessPoint>,

    #[builder(default = "View::main_menu()")]
    pub view: View,

    // for prompts inside of a view state
    #[builder(default = "vec![]")]
    pub prompt_stack: Vec<PromptState>,
    #[builder(setter(into, strip_option), default)]
    pub wifi_daemon: Option<DaemonType>,
}

pub fn handle_event(event: Event, data: &mut UiData) -> Option<AppAction> {
    // Priority: Prompt, Back (must not be in prompt), View
    if let Event::Key(key) = event {
        if key.code.is_esc() {
            if !data.prompt_stack.is_empty() {
                data.prompt_stack.pop();
                return None;
            }

            let back = data.view.handle_back(&key);
            if back.is_some() {
                return back;
            }
        }

        if let Some(top) = data.prompt_stack.last_mut() {
            if top.on_key(&key) {
                return None;
            }
        }

        match &mut data.view {
            View::MainMenu(list) => {
                list.on_key(&key);
                if key.code.is_enter() {
                    let selected = list.get_selected_value();
                    if selected == &MainMenuSelection::Exit {
                        return Some(AppAction::Exit);
                    } else {
                        data.view = selected.to_view();
                    }
                }
            }
            View::Connection(state) => {
                // mut ref to networks so we can move up/down
                if let Some(action) = state.on_key(&key, &mut data.networks) {
                    return Some(action);
                }
            }
            View::Vpn => {}
            View::Config => {}
        };
    }
    None
}

#[derive(Debug, PartialEq)]
pub enum MainMenuSelection {
    Connection,
    Vpn,
    Config,
    Exit,
}

impl MainMenuSelection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connection => "Connection",
            Self::Vpn => "VPN",
            Self::Config => "Config",
            Self::Exit => "Exit",
        }
    }

    fn to_view(&self) -> View {
        match self {
            MainMenuSelection::Connection => View::Connection(ConnectionState::new(vec![])),
            MainMenuSelection::Vpn => View::Vpn,
            MainMenuSelection::Config => View::Config,
            // okay because exit code checked first so this shouldn't be run
            MainMenuSelection::Exit => View::MainMenu(SelectableList::new(vec![])),
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

    pub fn connection() -> Self {
        View::Connection(ConnectionState::new(vec![]))
    }

    fn handle_back(&mut self, key: &KeyEvent) -> Option<AppAction> {
        if key.code.is_esc() {
            match &self {
                View::MainMenu(_) => {
                    return Some(AppAction::Exit);
                }
                _ => {
                    let mut new = Self::main_menu();
                    *self = new;
                }
            };
        }
        None
    }
}

// Connection
//
// Possible actions the user can take in the connection
// menu and how the data should be stored
#[derive(Clone, Debug)]
pub enum ConnectionAction {
    Scan,
    Connect,
    Disconnect,
    // Info,
    Forget,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FocusedConnection {
    Networks,
    Actions,
}

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct ConnectionState {
    #[builder(default = "SelectableList::new(vec![ConnectionAction::Scan])")]
    pub actions: SelectableList<ConnectionAction>,
    pub focused_list: SelectableList<FocusedConnection>,
}

impl ConnectionState {
    pub fn new(aps: Vec<AccessPoint>) -> Self {
        ConnectionStateBuilder::default()
            .focused_list(SelectableList::new(vec![
                FocusedConnection::Actions,
                FocusedConnection::Networks,
            ]))
            .build()
            .unwrap()
    }

    pub fn update_action_from_network(&mut self, networks: &SelectableList<AccessPoint>) {
        if !networks.items.is_empty() {
            let selected_flags = networks.get_selected_value().flags;
            self.actions = SelectableList::new(vec![ConnectionAction::Scan]);

            if selected_flags.contains(NetworkFlags::NEARBY)
                && !selected_flags.contains(NetworkFlags::CONNECTED)
            {
                self.actions.items.push(ConnectionAction::Connect);
            }

            if selected_flags.contains(NetworkFlags::CONNECTED) {
                self.actions.items.push(ConnectionAction::Disconnect);
            }

            if selected_flags.contains(NetworkFlags::KNOWN) {
                self.actions.items.push(ConnectionAction::Forget);
            }
        }
    }

    fn on_key(
        &mut self,
        key: &KeyEvent,
        networks: &mut SelectableList<AccessPoint>,
    ) -> Option<AppAction> {
        // check if up or down
        match self.focused_list.get_selected_value() {
            FocusedConnection::Actions => {
                self.actions.on_key(&key);
                match key.code {
                    KeyCode::Enter => match self.actions.get_selected_value() {
                        ConnectionAction::Scan => {
                            return Some(AppAction::Network(NetworkAction::Scan));
                        }
                        ConnectionAction::Connect => {
                            // start connect prompt
                            match networks.get_selected_value().security {
                                Security::Ieee8021x => {}
                                Security::Psk => {
                                    return Some(AppAction::AddPrompt(PromptState::PskConnect(
                                        PskConnectionPromptBuilder::default()
                                            .ssid(networks.get_selected_value().ssid.clone())
                                            .build()
                                            .unwrap(),
                                    )));
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            FocusedConnection::Networks => {
                networks.on_key(&key);
                Self::update_action_from_network(self, networks);
            }
        }

        match key.code {
            // since only two left/right functionally eq. to down
            KeyCode::Right | KeyCode::Left => {
                self.focused_list
                    .on_key(&KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
            }
            _ => {}
        };
        None
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

impl PromptState {
    fn on_key(&mut self, key: &KeyEvent) -> bool {
        match self {
            PromptState::PskConnect(prompt) => {
                // movement is more granular here
                prompt.on_key(key);
                true
            }
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

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct PskConnectionPrompt {
    pub ssid: String,
    #[builder(default = "String::new()")]
    pub password: String,
    #[builder(default = "SelectableList::new(PskPromptSelect::as_vec())")]
    pub select: SelectableList<PskPromptSelect>,
}

impl PskConnectionPrompt {
    fn new(ssid: String) -> Self {
        PskConnectionPromptBuilder::default().build().unwrap()
    }

    fn on_key(&mut self, key: &KeyEvent) {
        let selected = self.select.get_selected_value();
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
            _ => {}
        };
    }
}

// Generic data structure used to keep
// track of menu items
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SelectableList<T> {
    pub items: Vec<T>,
    pub selected: usize,
}

impl<T> SelectableList<T> {
    pub fn new(v: Vec<T>) -> Self {
        Self {
            items: v,
            selected: 0,
        }
    }

    fn next(&mut self) {
        if self.items.len() == 0 {
            return;
        }

        if self.selected == self.items.len() - 1 {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    fn prev(&mut self) {
        if self.items.len() == 0 {
            return;
        }

        if self.selected == 0 {
            self.selected = self.items.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    pub fn get_selected_value(&self) -> &T {
        &self.items[self.selected]
    }

    fn on_key(&mut self, key: &KeyEvent) {
        if key.code.is_down() {
            self.next();
        }
        if key.code.is_up() {
            self.prev();
        }
    }
}

impl<T> SelectableList<T>
where
    T: PartialEq,
{
    // sets val as the currently selected
    // item
    fn set(&mut self, val: T) {
        match self.items.iter().position(|v| *v == val) {
            Some(pos) => {
                self.selected = pos;
            }
            None => {}
        }
    }
}
