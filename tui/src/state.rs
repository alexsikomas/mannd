use com::{
    controller::DaemonType,
    wireless::common::{AccessPoint, Security},
};
use crossterm::event::{Event, KeyCode, KeyEvent};
use derive_builder::Builder;
use tracing::info;

use crate::app::UpdateAction;
use com::state::network::NetworkAction;

/// Data used for UI, may be sent to threads through
/// channels
#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct UiData {
    // Only ConnectionState needs the selected part but
    // it might cause less headache being able to access
    // here
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
    if let Event::Key(key) = event {
        match &mut data.view {
            View::MainMenu(list) => {
                if key.code.is_down() {
                    list.next();
                }
                if key.code.is_up() {
                    list.prev();
                }
                if key.code.is_enter() {
                    let selected = list.get_selected_value();
                    if selected == &MainMenuSelection::Exit {
                        return Some(AppAction::Exit);
                    } else {
                        data.view = selected.to_view();
                    }
                }
            }
            View::Connection(state) => {}
            View::Vpn => {}
            View::Config => {}
        };
    }
    None
}

pub enum AppAction {
    Exit,
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

//* View
//*
//* Controls what should happen on keypress and
//* what the viewer should see
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
}

//* Prompt
//*
//* Displays a prompt visually on
//* top of the current view
#[derive(Debug)]
pub enum PromptState {
    Connect(ConnectionPrompt),
}

#[derive(Debug)]
pub enum ConnectionPromptSelect {
    Password,
    Connect,
    Back,
}

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct ConnectionPrompt {
    pub ssid: String,
    #[builder(default = "String::new()")]
    pub password: String,
    #[builder(default = "ConnectionPromptSelect::Password")]
    pub select: ConnectionPromptSelect,
}

impl ConnectionPrompt {
    fn new(ssid: String) -> Self {
        ConnectionPromptBuilder::default().build().unwrap()
    }
}

//* Connection
//*
//* Possible actions the user can take in the connection
//* menu and how the data should be stored
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
    #[builder(default = "FocusedConnection::Actions")]
    pub focused_list: FocusedConnection,
}

impl ConnectionState {
    pub fn new(aps: Vec<AccessPoint>) -> Self {
        ConnectionStateBuilder::default().build().unwrap()
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

//* Generic data structure used to keep
//* track of menu items
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
}
