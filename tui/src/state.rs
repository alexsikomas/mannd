use com::wireless::common::{AccessPoint, Security};
use crossterm::event::KeyCode;
use tracing::info;

use crate::app::UpdateAction;
use com::state::network::NetworkAction;

//* State
//*
//* Controls what should happen on keypress and
//* what the viewer should see
pub enum State {
    MainMenu(SelectableList<MainMenuSelection>),
    Connection(ConnectionState),
    Vpn,
    Config,
}

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
}

impl State {
    pub fn main_menu() -> Self {
        State::MainMenu(SelectableList::new(vec![
            MainMenuSelection::Connection,
            MainMenuSelection::Vpn,
            MainMenuSelection::Config,
            MainMenuSelection::Exit,
        ]))
    }

    pub fn connection() -> Self {
        State::Connection(ConnectionState::new(vec![]))
    }

    /// Main handler for input, delagates some work to helper
    /// functions to contain menu specific logic
    pub fn handle_input(&mut self, key: KeyCode) -> Option<UpdateAction> {
        if key == KeyCode::Esc {
            match &self {
                State::MainMenu(_) => {
                    return Some(UpdateAction::Exit);
                }
                State::Connection(conn_state) => {
                    *self = State::main_menu();
                }
                _ => {}
            }
            return None;
        }

        match self {
            Self::MainMenu(list) => {
                if key == KeyCode::Enter {
                    match list.get_selected_value() {
                        MainMenuSelection::Connection => {
                            *self = State::connection();
                            // get known networks instead
                            // return Some(UpdateAction::Network(NetworkAction::Scan));
                        }
                        MainMenuSelection::Vpn => {}
                        MainMenuSelection::Config => {}
                        MainMenuSelection::Exit => return Some(UpdateAction::Exit),
                        _ => {}
                    }
                    return None;
                }
                return Self::handle_main_menu_input(list, key);
            }
            Self::Connection(conn_state) => {
                return Self::handle_connection_input(conn_state, key);
            }
            Self::Vpn => {}
            Self::Config => {}
        }
        None
    }

    fn handle_main_menu_input(
        list: &mut SelectableList<MainMenuSelection>,
        key: KeyCode,
    ) -> Option<UpdateAction> {
        match key {
            KeyCode::Up => {
                list.prev();
            }
            KeyCode::Down => {
                list.next();
            }
            _ => {}
        };
        None
    }

    fn handle_connection_input(
        conn_state: &mut ConnectionState,
        key: KeyCode,
    ) -> Option<UpdateAction> {
        match &conn_state.focused_list {
            FocusedConnection::Actions => match key {
                KeyCode::Up => {
                    conn_state.actions.prev();
                }
                KeyCode::Down => {
                    conn_state.actions.next();
                }
                KeyCode::Left => {
                    conn_state.focused_list = FocusedConnection::Networks;
                }
                KeyCode::Enter => match conn_state.actions.get_selected_value() {
                    ConnectionAction::Scan => {
                        return Some(UpdateAction::Network(NetworkAction::Scan));
                    }
                    ConnectionAction::Connect => {
                        let selected = conn_state.networks.get_selected_value();

                        if selected.known || matches!(selected.security, Security::Open) {
                            // connect function does not need any password if already known
                            return Some(UpdateAction::Network(NetworkAction::Connect(
                                selected.ssid.clone(),
                                "".to_string(),
                                selected.security.clone(),
                            )));
                        }

                        return Some(UpdateAction::OpenPrompt(PromptState::Connect(
                            ConnectionPrompt::new(selected.ssid.clone()),
                        )));
                    }
                    // ConnectionAction::Info => {
                    // return Some(UpdateAction::Network(NetworkAction::Info));
                    // }
                    ConnectionAction::Disconnect => {
                        return Some(UpdateAction::Network(NetworkAction::Disconnect));
                    }
                    ConnectionAction::Forget => {
                        let selected = conn_state.networks.get_selected_value();
                        return Some(UpdateAction::Network(NetworkAction::Forget(
                            selected.ssid.clone(),
                            selected.security.clone(),
                        )));
                    }
                    _ => {}
                },
                _ => {}
            },
            FocusedConnection::Networks => match key {
                KeyCode::Up => {
                    conn_state.networks.prev();
                }
                KeyCode::Down => {
                    conn_state.networks.next();
                }
                KeyCode::Right | KeyCode::Enter => {
                    conn_state.focused_list = FocusedConnection::Actions;
                }
                _ => {}
            },
        };
        None
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

#[derive(Debug)]
pub struct ConnectionPrompt {
    pub ssid: String,
    pub password: String,
    pub select: ConnectionPromptSelect,
}

impl ConnectionPrompt {
    fn new(ssid: String) -> Self {
        Self {
            ssid,
            password: String::new(),
            select: ConnectionPromptSelect::Password,
        }
    }
}

impl PromptState {
    pub fn handle_input(&mut self, key: KeyCode) -> Option<UpdateAction> {
        match key {
            KeyCode::Esc => {
                return Some(UpdateAction::ExitPrompt);
            }
            _ => {}
        };

        match self {
            PromptState::Connect(conn) => match key {
                KeyCode::Enter => match conn.select {
                    ConnectionPromptSelect::Connect => {
                        return Some(UpdateAction::Network(NetworkAction::Connect(
                            conn.ssid.clone(),
                            conn.password.clone(),
                            Security::Psk,
                        )));
                    }
                    ConnectionPromptSelect::Back => {
                        return Some(UpdateAction::ExitPrompt);
                    }
                    _ => {}
                },
                KeyCode::Backspace => {
                    if conn.password.len() > 0
                        && matches!(conn.select, ConnectionPromptSelect::Password)
                    {
                        conn.password.pop();
                    }
                }
                KeyCode::Up => match conn.select {
                    ConnectionPromptSelect::Password => {
                        conn.select = ConnectionPromptSelect::Connect;
                    }
                    _ => {
                        conn.select = ConnectionPromptSelect::Password;
                    }
                },
                KeyCode::Down => match conn.select {
                    ConnectionPromptSelect::Password => {
                        conn.select = ConnectionPromptSelect::Connect;
                    }
                    _ => {
                        conn.select = ConnectionPromptSelect::Password;
                    }
                },
                KeyCode::Left | KeyCode::Right => match conn.select {
                    ConnectionPromptSelect::Password => {}
                    ConnectionPromptSelect::Connect => {
                        conn.select = ConnectionPromptSelect::Back;
                    }
                    ConnectionPromptSelect::Back => {
                        conn.select = ConnectionPromptSelect::Connect;
                    }
                },
                KeyCode::Char(c) => match conn.select {
                    ConnectionPromptSelect::Password => {
                        conn.password.push(c);
                    }
                    _ => {}
                },
                _ => {}
            },
        };
        None
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

#[derive(PartialEq, Eq)]
pub enum FocusedConnection {
    Networks,
    Actions,
}

pub struct ConnectionState {
    pub networks: SelectableList<AccessPoint>,
    pub actions: SelectableList<ConnectionAction>,
    pub focused_list: FocusedConnection,
}

impl ConnectionState {
    pub fn new(aps: Vec<AccessPoint>) -> Self {
        Self {
            networks: SelectableList::new(aps),
            actions: SelectableList::new(vec![ConnectionAction::Scan]),
            focused_list: FocusedConnection::Actions,
        }
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
