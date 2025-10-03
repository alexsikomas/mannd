use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use tokio::sync::{mpsc::UnboundedReceiver, oneshot};
use tracing::info;

pub mod components;
pub mod ui;

#[derive(Clone)]
pub enum ActiveView {
    MainMenu,
    Connection,
    Vpn,
    Config,
}

// state
#[derive(Clone)]
pub struct App {
    pub active_view: ActiveView,
    pub views: SelectableList<&'static str>,
    pub main_menu: SelectableList<&'static str>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            active_view: ActiveView::MainMenu,
            views: SelectableList::new(vec!["Connection", "VPN", "Config", "Exit"]),
            main_menu: SelectableList::new(vec!["Main", "Connection", "VPN", "Config"]),
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
                AppMessage::Event(e) => {
                    self.event(e);
                }
                AppMessage::Query(q) => {
                    self.query(q);
                }
            }
        }
    }

    fn query(&mut self, query: Query) {
        match query {
            Query::View { res } => {
                res.send(self.views.clone());
            }
            Query::MainMenu { res } => {
                res.send(self.main_menu.clone());
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Up => match self.get_active_list() {
                    Some(v) => {
                        v.prev();
                    }
                    _ => {}
                },
                KeyCode::Down => match self.get_active_list() {
                    Some(v) => {
                        v.next();
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
    }

    fn get_active_list(&mut self) -> Option<&mut SelectableList<&'static str>> {
        match self.active_view {
            ActiveView::MainMenu => Some(&mut self.main_menu),
            _ => None,
        }
    }
}

pub enum AppMessage {
    Event(Event),
    Query(Query),
}

pub enum Query {
    View {
        res: oneshot::Sender<SelectableList<&'static str>>,
    },
    MainMenu {
        res: oneshot::Sender<SelectableList<&'static str>>,
    },
}
