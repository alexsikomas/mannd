use com::wireless::common::AccessPoint;
use crossterm::event::KeyCode;

use crate::{app::UpdateAction, network::NetworkAction};

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

pub enum ConnectionAction {
    Scan,
    Connect,
    Add,
    Remove,
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
            actions: SelectableList::new(vec![
                ConnectionAction::Scan,
                ConnectionAction::Connect,
                ConnectionAction::Add,
                ConnectionAction::Remove,
            ]),
            focused_list: FocusedConnection::Networks,
        }
    }
}

impl ConnectionAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scan => "Scan",
            Self::Connect => "Connect",
            Self::Add => "Add",
            Self::Remove => "Remove",
        }
    }
}

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

    fn get_selected_value(&self) -> &T {
        &self.items[self.selected]
    }
}

impl State {
    pub fn main_menu() -> Self {
        State::MainMenu(SelectableList::new(vec![
            MainMenuSelection::Connection,
            MainMenuSelection::Vpn,
            MainMenuSelection::Connection,
            MainMenuSelection::Exit,
        ]))
    }

    pub fn connection() -> Self {
        State::Connection(ConnectionState::new(vec![]))
    }

    pub fn handle_input(&mut self, key: KeyCode) -> Option<UpdateAction> {
        if key == KeyCode::Esc {
            match &self {
                State::MainMenu(_) => {
                    return Some(UpdateAction::Exit);
                }
                State::Connection(_) => {
                    *self = State::main_menu();
                }
                _ => {}
            }
        }

        match self {
            Self::MainMenu(list) => {
                if key == KeyCode::Enter {
                    match list.get_selected_value() {
                        MainMenuSelection::Connection => {
                            *self = State::connection();
                            return Some(UpdateAction::Network(NetworkAction::Scan));
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
                Self::handle_connection_input(conn_state, key);
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

    fn handle_connection_input(conn_state: &mut ConnectionState, key: KeyCode) {
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
        }
    }
}

// impl ConnectionState {
//     pub fn next(&mut self) {
//         match self.focused_list {
//             FocusedConnection::Networks => {
//                 self.networks.next();
//             }
//             FocusedConnection::Actions => {
//                 self.actions.next();
//             }
//         }
//     }
//
//     pub fn previous(&mut self) {
//         match self.focused_list {
//             FocusedConnection::Networks => {
//                 self.networks.prev();
//             }
//             FocusedConnection::Actions => {
//                 self.actions.prev();
//             }
//         }
//     }
// }
