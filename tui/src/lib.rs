use ratatui::crossterm::event::Event;
use tokio::sync::{mpsc::UnboundedReceiver, oneshot};
use tracing::info;

pub mod components;
pub mod ui;

// state
#[derive(Clone)]
pub struct App {
    pub views: SelectableList<&'static str>,
    pub main_menu: SelectableList<&'static str>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            // index not value
            views: SelectableList::views(),
            main_menu: SelectableList::main_menu(),
        }
    }
}

#[derive(Clone)]
pub struct SelectableList<T> {
    pub items: Vec<T>,
    pub selected: usize,
}

// initialisations
impl SelectableList<&'static str> {
    pub fn main_menu() -> Self {
        Self {
            items: vec!["Connection", "VPN", "Config", "Exit"],
            selected: 0,
        }
    }

    // not something you select in the same way but works the same way
    pub fn views() -> Self {
        Self {
            items: vec!["Main Menu", "Connection", "VPN", "Config"],
            selected: 0,
        }
    }
}

// operations
impl SelectableList<&'static str> {
    pub fn next(&mut self) {
        if self.items.len() > (self.selected + 1) {
            self.selected += 1;
            return;
        }
        self.selected = 0;
    }

    pub fn prev(&mut self) {
        if self.selected == 0 {
            self.selected = self.items.len() - 1;
            return;
        }
        self.selected -= 1;
    }
}

// events
impl App {
    /// Handles queries and actions
    pub async fn handle(mut self, mut rx: UnboundedReceiver<AppMessage>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                AppMessage::Event(a) => {}
                AppMessage::Query(q) => {
                    self.query(q);
                }
            }
        }
    }

    pub fn query(&mut self, query: Query) {
        match query {
            Query::GetView { res } => {
                res.send(self.views.clone());
            }
            Query::GetMainMenu { res } => {
                res.send(self.main_menu.clone());
            }
        }
    }
}

pub enum AppMessage {
    Event(Event),
    Query(Query),
}

pub enum Query {
    GetView {
        res: oneshot::Sender<SelectableList<&'static str>>,
    },
    GetMainMenu {
        res: oneshot::Sender<SelectableList<&'static str>>,
    },
}
