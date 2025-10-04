use std::{sync::Arc, time::Duration};

use color_eyre::eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent};
use futures::{FutureExt, select, stream::StreamExt};
use futures_timer::Delay;
use ratatui::crossterm::{
    event::{self},
    terminal,
};
use tokio::sync::{
    RwLock,
    mpsc::{self, Sender, UnboundedReceiver, UnboundedSender},
    oneshot,
};
use tracing::{error, info};

use crate::{app, ui::render};

#[derive(Clone)]
pub enum ActiveView {
    MainMenu,
    Connection,
    Vpn,
    Config,
}

// state
#[derive(Clone)]
pub struct AppState {
    pub active_view: ActiveView,
    pub views: SelectableList<&'static str>,
    pub main_menu: SelectableList<&'static str>,
    is_running: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_view: ActiveView::MainMenu,
            // Would have prefered to use enums instead of hardcoded
            // strs but this requires polymorphism, avoided in non
            // io-blocking code. Might think of a simpler solution later.
            // -----------------------------------------------------------
            // we do not use index property for views instead active_view is used
            // this is the only exception to this
            views: SelectableList::new(vec!["Main", "Connection", "VPN", "Config"]),
            main_menu: SelectableList::new(vec!["Connection", "VPN", "Config", "Exit"]),
            is_running: true,
        }
    }
}

#[derive(Clone)]
pub struct SelectableList<T> {
    pub items: Vec<T>,
    pub selected: usize,
}

// operations
impl SelectableList<&'static str> {
    pub fn new(v: Vec<&'static str>) -> Self {
        Self {
            items: v,
            selected: 0,
        }
    }

    /// Works like a saturating add for a range between 0 to
    /// the length of the list
    pub fn next(&mut self) {
        if self.items.len() > (self.selected + 1) {
            self.selected += 1;
            return;
        }
        self.selected = 0;
    }

    /// Works as a saturating sub for a range between 0 to
    /// the length of the list
    pub fn prev(&mut self) {
        if self.selected == 0 {
            self.selected = self.items.len() - 1;
            return;
        }
        self.selected -= 1;
    }

    fn get_selected(&self) -> &'static str {
        self.items[self.selected]
    }
}

impl AppState {
    /// Handles all message events, delagates work to `self.event()` and `self.query()`.
    ///
    /// Takes in an event reciever `rx`, and a quit sender `q_tx`
    pub async fn handle(mut self, mut rx: UnboundedReceiver<Action>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                Action::Event(e) => {
                    self.event(e);
                }
                Action::GetState => {}
                Action::Update => {}
            }
        }
    }

    /// Handles input events, mutates the values in the state
    fn event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up => {
                    self.get_active_list_mut().map(|v| v.prev());
                }
                KeyCode::Down => {
                    self.get_active_list_mut().map(|v| v.next());
                }
                KeyCode::Enter => {
                    let selected = self.get_active_list().and_then(|v| Some(v.get_selected()));
                    if let Some(v) = selected {
                        if v == "Exit" {
                            self.is_running = false;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn get_active_list_mut(&mut self) -> Option<&mut SelectableList<&'static str>> {
        match self.active_view {
            ActiveView::MainMenu => Some(&mut self.main_menu),
            _ => None,
        }
    }

    fn get_active_list(&self) -> Option<&SelectableList<&'static str>> {
        match self.active_view {
            ActiveView::MainMenu => Some(&self.main_menu),
            _ => None,
        }
    }

    /// Handles the selection of items based on the current view.
    ///
    /// Takes in quit sender `q_tx` to be able to send messages
    /// to main for exiting the application
    fn handle_selection(&mut self, selected: &'static str) {
        match self.active_view {
            ActiveView::MainMenu => match selected {
                "Connection" => self.active_view = ActiveView::Connection,
                "VPN" => self.active_view = ActiveView::Vpn,
                "Config" => self.active_view = ActiveView::Config,
                "Exit" => {
                    self.is_running = false;
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// Message type used for any messages travelling on the event
/// mpsc channel. Messages are sent from external functions
/// and are handled by either changing application state or
/// returning a value through `oneshot`
pub enum Action {
    Event(Event),
    GetState,
    Update,
}

pub struct App;

impl App {
    pub async fn run() -> color_eyre::Result<()> {
        let mut state = AppState::default();
        let (tx, mut rx) = mpsc::unbounded_channel::<Action>();
        let action_tx = tx.clone();
        let is_alive = true;

        let mut terminal = ratatui::init();

        tokio::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                let mut delay = Delay::new(Duration::from_millis(1_000)).fuse();
                let mut event = reader.next().fuse();
                select! {
                    _ = delay => {},
                    m_evt = event => {
                        match m_evt {
                            Some(Ok(evt)) => {
                                tx.send(Action::Event(evt));
                            },
                            Some(Err(e)) => {},
                            None => break,
                        }
                    }
                };
            }
        });

        while state.is_running {
            terminal.draw(|f| render(f, &state))?;

            if let Some(action) = rx.recv().await {
                match action {
                    Action::Event(evt) => {
                        if let Event::Key(key) = evt {
                            if key.code == KeyCode::Esc {
                                state.is_running = false;
                            }
                        }
                        state.event(evt);
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
