use std::time::Duration;

use com::controller::Controller;
use crossterm::event::{self, Event, KeyCode};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::info;

use crate::{
    error::TuiError,
    network::{NetworkAction, NetworkState, NetworkUiState, NetworkUpdate, network_handle},
    ui::render,
};

pub struct App;

pub struct AppState {
    is_running: bool,
    redraw: bool,
    pub view: View,
    pub network: NetworkState,
}

impl AppState {
    async fn new() -> Self {
        Self {
            view: View::new(),
            is_running: true,
            redraw: false,
            network: NetworkState {
                selected: None,
                aps: vec![],
            },
        }
    }

    // general select function that calls all other on_*_select functions
    // Optionally returns a network action if one needs to be taken due to the input
    fn on_select(&mut self) -> Option<NetworkAction> {
        let selection = self.view.selections.get_selected();
        match self.view.active {
            ViewId::MainMenu => self.on_main_menu_select(selection.clone()),
            ViewId::Connection => match self.view.selections.get_selected() {
                Selection::Network(arr) => {
                    // pressing enter just selecting and going to the 'actions' sidebar
                    // the same as just pressing right arrow
                    self.view.selections.selected += 1;
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn on_main_menu_select(&mut self, selection: Selection) -> Option<NetworkAction> {
        match selection {
            Selection::Exit => {
                if self.view.active == ViewId::MainMenu {
                    self.is_running = false
                } else {
                    self.view.active = ViewId::MainMenu;
                    self.view.selections = View::main_menu();
                }
            }
            Selection::Connection => {
                self.view.active = ViewId::Connection;
                // aps not init yet so has to be done once scan is finished,
                // still need this as we need the values
                self.view.selections = View::connection(Some(0), 0);
                info!(
                    "Updated selections (conn): {:?}, {:?}",
                    self.view.selections,
                    self.network.aps.len()
                );
                return Some(NetworkAction::Scan);
            }
            Selection::Vpn => {
                self.view.active = ViewId::Vpn;
            }
            Selection::Config => {
                self.view.active = ViewId::Config;
            }
            _ => {}
        };
        None
    }
}

impl App {
    pub async fn run() -> Result<(), TuiError> {
        let mut state = AppState::new().await;
        let (event_tx, mut event_rx) = mpsc::channel::<NetworkAction>(32);
        let (net_state_tx, mut net_state_rx) = mpsc::channel::<NetworkUpdate>(32);

        let mut terminal = ratatui::init();

        // networking thread
        tokio::spawn(async move {
            network_handle(&mut event_rx, net_state_tx).await;
        });

        terminal.draw(|f| render(f, &state))?;

        while state.is_running {
            handle_net_state_msg(&mut state, &mut net_state_rx);

            if event::poll(Duration::from_millis(100))? {
                if let Ok(Event::Key(key)) = event::read() {
                    match key.code {
                        KeyCode::Up => state.view.selections.change_selection(key.code),
                        KeyCode::Down => state.view.selections.change_selection(key.code),
                        KeyCode::Right => {
                            state.view.selections.change_selection(key.code);
                        }
                        KeyCode::Left => {
                            state.view.selections.change_selection(key.code);
                        }
                        KeyCode::Enter => {
                            if let Some(action) = state.on_select() {
                                let _ = event_tx.send(action).await;
                            }
                        }
                        KeyCode::Esc => {
                            if state.view.active == ViewId::MainMenu {
                                state.is_running = false;
                            } else {
                                state.view.active = ViewId::MainMenu;
                                state.view.selections = View::main_menu();
                            }
                        }
                        _ => {}
                    }
                    state.redraw = true;
                };
            }

            if state.redraw {
                terminal.draw(|f| render(f, &state))?;
                state.redraw = false;
            }
        }

        Ok(())
    }
}

fn handle_net_state_msg(state: &mut AppState, net_state_rx: &mut Receiver<NetworkUpdate>) {
    if let Ok(msg) = net_state_rx.try_recv() {
        match msg {
            NetworkUpdate::Select(i) => {
                state.network.selected = Some(i);
            }
            NetworkUpdate::Deselect => {
                state.network.selected = None;
            }
            NetworkUpdate::UpdateAps(aps) => {
                state.network.aps = aps;
                let selected = state.view.selections.selected.clone();
                state.view.selections = View::connection(Some(0), state.network.aps.len());
                state.view.selections.selected = selected;
            }
        }
        state.redraw = true;
    };
}

/*
* View/ViewId/Selection Functionality below
*/

#[derive(PartialEq, Eq)]
pub enum ViewId {
    MainMenu,
    Connection,
    Vpn,
    Config,
}

pub struct View {
    pub active: ViewId,
    pub selections: SelectableList<Selection>,
}

impl View {
    fn new() -> Self {
        Self {
            active: ViewId::MainMenu,
            selections: Self::main_menu(),
        }
    }

    fn main_menu() -> SelectableList<Selection> {
        SelectableList::new(vec![
            Selection::Connection,
            Selection::Vpn,
            Selection::Config,
            Selection::Exit,
        ])
    }

    fn connection(selected: Option<usize>, max: usize) -> SelectableList<Selection> {
        SelectableList::new(vec![
            Selection::Network(NetworkUiState::new(selected, max)),
            Selection::Scan,
            Selection::Connect,
            Selection::Edit,
            Selection::Remove,
        ])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Selection {
    // main menu
    Connection,
    Vpn,
    Config,
    Exit,

    // connection
    Network(NetworkUiState),
    Scan,
    Connect,
    Edit,
    Remove,
}

impl Selection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Selection::Connection => "Connection",
            Selection::Vpn => "VPN",
            Selection::Config => "Config",
            Selection::Exit => "Exit",
            Selection::Network(_) => "Networks",
            Selection::Scan => "Scan",
            Selection::Connect => "Connect",
            Selection::Edit => "Edit",
            Selection::Remove => "Remove",
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
}

impl SelectableList<Selection> {
    fn change_selection(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => {
                // check if selection is Network
                match self.get_selected_mut() {
                    Selection::Network(state) => {
                        if state.max == 0 || state.selected.is_none() {
                            return;
                        }

                        if state.selected == Some(0) {
                            state.selected = Some(state.max - 1);
                        } else {
                            state.selected = Some(state.selected.unwrap() - 1);
                        }
                        return;
                    }
                    Selection::Scan => match &self.items[0] {
                        Selection::Network(state) => {
                            if state.selected.is_none() {
                                return;
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                };

                if self.selected == 0 {
                    self.selected = self.items.len() - 1;
                } else {
                    self.selected -= 1;
                }
            }
            KeyCode::Down => {
                match self.get_selected_mut() {
                    Selection::Network(state) => {
                        if state.max == 0 || state.selected.is_none() {
                            return;
                        }

                        if state.max > state.selected.unwrap() + 1 {
                            state.selected = Some(state.selected.unwrap() + 1);
                        } else {
                            state.selected = Some(0);
                        }
                        return;
                    }
                    Selection::Scan => match &self.items[0] {
                        Selection::Network(state) => {
                            if state.selected.is_none() {
                                return;
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                };

                if self.items.len() > self.selected + 1 {
                    self.selected += 1;
                } else {
                    self.selected = 0
                }
            }
            KeyCode::Left => match self.get_selected() {
                Selection::Scan | Selection::Connect | Selection::Edit | Selection::Remove => {
                    match &self.items[0] {
                        Selection::Network(state) => {
                            self.items = View::connection(Some(0), state.max).items;
                        }
                        _ => {}
                    }
                    self.selected = 0;
                }
                _ => {}
            },
            KeyCode::Right => {
                // check if network
                match self.get_selected() {
                    Selection::Network(state) => {
                        self.items = View::connection(None, state.max).items;
                        self.selected += 1;
                    }
                    _ => {}
                }
            }
            // ignore unrealated action
            _ => {}
        }
    }

    fn get_selected(&self) -> &Selection {
        &self.items[self.selected]
    }

    fn get_selected_mut(&mut self) -> &mut Selection {
        &mut self.items[self.selected]
    }
}
