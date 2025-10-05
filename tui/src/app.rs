use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyCode};
use futures::{FutureExt, select, stream::StreamExt};
use futures_timer::Delay;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::info;

use crate::ui::render;

// state
#[derive(Clone)]
pub struct AppState {
    pub view: View,
    is_running: bool,
}

impl AppState {
    // general select function that calls all other on_*_select functions
    fn on_select(&mut self) {
        let selection = self.view.selections.get_selected();
        match self.view.active {
            ViewId::MainMenu => self.on_main_menu_select(selection.clone()),
            _ => {}
        }
    }

    fn on_main_menu_select(&mut self, selection: Selection) {
        match selection {
            Selection::Exit => self.is_running = false,
            Selection::Connection => {
                self.view.active = ViewId::Connection;
            }
            Selection::Vpn => {
                self.view.active = ViewId::Vpn;
            }
            Selection::Config => {
                self.view.active = ViewId::Config;
            }
            _ => {}
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            view: View::new(),
            is_running: true,
        }
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
            _ => Action::NoOp,
        }
    } else {
        Action::NoOp
    }
}

pub enum Action {
    NoOp,
    Event(Event),

    SelectUp,
    SelectDown,
    SelectLeft,
    SelectRight,
    Enter,

    Exit,
}

pub struct App;

impl App {
    pub async fn run() -> color_eyre::Result<()> {
        let mut state = AppState::default();
        let (tx, mut rx) = mpsc::unbounded_channel::<Action>();

        let mut terminal = ratatui::init();

        tokio::spawn(async move {
            let mut reader = EventStream::new();
            while let Some(Ok(evt)) = reader.next().await {
                let action = event(evt);
                tx.send(action);
            }
        });

        while state.is_running {
            terminal.draw(|f| render(f, &state))?;

            if let Some(action) = rx.recv().await {
                match action {
                    Action::SelectUp => state.view.selections.change_selection(Action::SelectUp),
                    Action::SelectDown => {
                        state.view.selections.change_selection(Action::SelectDown)
                    }
                    Action::Enter => {
                        state.on_select();
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub enum ViewId {
    MainMenu,
    Connection,
    Vpn,
    Config,
}

#[derive(Clone)]
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
