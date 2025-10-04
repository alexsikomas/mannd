use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use tokio::sync::{
    mpsc::{Sender, UnboundedReceiver, UnboundedSender},
    oneshot,
};
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
            /*
             * Would have prefered to use enums instead of hardcoded
             * strs but this requires polymorphism, avoided in non
             * io-blocking code. Might think of a simpler solution later
             *
             * we do not use index property for views instead active_view is used
             */
            views: SelectableList::new(vec!["Main", "Connection", "VPN", "Config"]),
            main_menu: SelectableList::new(vec!["Connection", "VPN", "Config", "Exit"]),
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

    fn get_selected(&self) -> &'static str {
        self.items[self.selected]
    }
}

// events
impl App {
    pub async fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    /// Handles queries and actions
    pub async fn handle(mut self, mut rx: UnboundedReceiver<AppMessage>, q_tx: Sender<()>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                AppMessage::Event(e) => {
                    self.event(e, q_tx.clone());
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

    fn event(&mut self, event: Event, q_tx: Sender<()>) {
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
                        self.handle_selection(v, q_tx.clone());
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

    fn handle_selection(&mut self, selected: &'static str, q_tx: Sender<()>) {
        match self.active_view {
            ActiveView::MainMenu => match selected {
                "Connection" => self.active_view = ActiveView::Connection,
                "VPN" => self.active_view = ActiveView::Vpn,
                "Config" => self.active_view = ActiveView::Config,
                "Exit" => {
                    tokio::task::block_in_place(|| {
                        q_tx.blocking_send(());
                    });
                }
                _ => {}
            },
            _ => {}
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

pub enum ControlFlow {
    Quit { res: oneshot::Sender<bool> },
}
