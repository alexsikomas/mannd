use std::{sync::Arc, time::Duration};

use com::{
    controller::{self, Controller, WirelessAdapter},
    wireless::common::AccessPoint,
};
use crossterm::event::{self, Event, EventStream, KeyCode};
use futures::stream::StreamExt;
use ratatui::crossterm::event::poll;
use tokio::sync::{
    RwLock,
    mpsc::{self},
};
use tracing::info;

use crate::{error::TuiError, ui::render};

pub struct AppState {
    pub view: View,
    is_running: bool,
    network: NetworkState,
}

impl AppState {
    async fn new() -> Self {
        Self {
            view: View::new(),
            is_running: true,
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
            _ => None,
        }
    }

    fn on_main_menu_select(&mut self, selection: Selection) -> Option<NetworkAction> {
        match selection {
            Selection::Exit => self.is_running = false,
            Selection::Connection => {
                self.view.active = ViewId::Connection;
                self.view.selections = View::connection();
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

/// Handles input events, mutates the values in the state
fn event(event: Event) -> Action {
    info!("Event: {:?}", event);
    if let Event::Key(key) = event {
        match key.code {
            KeyCode::Up => Action::SelectUp,
            KeyCode::Down => Action::SelectDown,
            KeyCode::Enter => Action::Enter,
            KeyCode::Esc => Action::Exit,
            _ => Action::NoOp,
        }
    } else {
        Action::NoOp
    }
}

pub enum Action {
    NoOp,
    Update,
    Event(Event),

    SelectUp,
    SelectDown,
    SelectLeft,
    SelectRight,
    Enter,

    Exit,
}

#[derive(Debug)]
pub enum NetworkAction {
    Scan,
    ForceIwd,
    ForceWpa,
    ForceWifiNetlink,
}

pub struct App;

impl App {
    pub async fn run() -> Result<(), TuiError> {
        let mut state = AppState::new().await;
        let (event_tx, mut event_rx) = mpsc::channel::<NetworkAction>(32);
        let (net_state_tx, mut net_state_rx) = mpsc::channel::<NetworkUpdate>(32);

        let mut terminal = ratatui::init();

        // networking thread
        tokio::spawn(async move {
            if let Ok(mut controller) = Controller::new().await {
                controller.determine_adapter().await;
                while let Some(action) = event_rx.recv().await {
                    match action {
                        NetworkAction::Scan => {
                            if let Ok(aps) = controller.scan().await {
                                net_state_tx.send(NetworkUpdate::UpdateAps(aps));
                            }
                        }
                        NetworkAction::ForceIwd => {}
                        NetworkAction::ForceWpa => {}
                        NetworkAction::ForceWifiNetlink => {}
                        _ => {}
                    };
                }
            };
        });

        terminal.draw(|f| render(f, &state))?;

        while state.is_running {
            match event::poll(Duration::from_millis(100)) {
                Ok(v) => match v {
                    false => {
                        continue;
                    }
                    _ => {}
                },
                _ => {
                    continue;
                }
            }

            if let Ok(evt) = event::read() {
                match event(evt) {
                    Action::SelectUp => state.view.selections.change_selection(Action::SelectUp),
                    Action::SelectDown => {
                        state.view.selections.change_selection(Action::SelectDown)
                    }
                    Action::Enter => {
                        if let Some(action) = state.on_select() {
                            info!("{:?}", action);
                            event_tx.send(action).await;
                        }
                    }
                    Action::Exit => {
                        state.is_running = false;
                    }
                    Action::NoOp => {}
                    _ => {}
                }
            };
            terminal.draw(|f| render(f, &state))?;
        }

        Ok(())
    }
}

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

    fn connection() -> SelectableList<Selection> {
        SelectableList::new(vec![
            Selection::Network(1),
            Selection::Scan,
            Selection::Connect,
            Selection::Edit,
            Selection::Remove,
        ])
    }
}

#[derive(Clone)]
pub enum Selection {
    // main menu
    Connection,
    Vpn,
    Config,
    Exit,

    // connection
    /// Network is selected if we are on a wifi network, u16 is for the index
    /// once we exceed the amount of networks we go to the other options
    Network(u16),
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

#[derive(Clone)]
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
    fn change_selection(&mut self, action: Action) {
        match action {
            Action::SelectUp => {
                self.selected = self.prev();
            }
            Action::SelectDown => {
                self.selected = self.next();
            }
            Action::SelectLeft => {}
            Action::SelectRight => {}
            // ignore unrealated action
            _ => {}
        }
    }

    fn next(&self) -> usize {
        if self.items.len() > self.selected + 1 {
            self.selected + 1
        } else {
            0
        }
    }

    fn prev(&self) -> usize {
        if self.selected == 0 {
            self.items.len() - 1
        } else {
            self.selected - 1
        }
    }

    fn get_selected(&self) -> &Selection {
        &self.items[self.selected]
    }
}

pub struct NetworkState {
    selected: Option<usize>,
    aps: Vec<AccessPoint>,
}

enum NetworkUpdate {
    Select(usize),
    Deselect,
    UpdateAps(Vec<AccessPoint>),
}
