use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use tokio::sync::{
    mpsc::{Sender, UnboundedReceiver, UnboundedSender},
    oneshot,
};
use tracing::{error, info};

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
            // Would have prefered to use enums instead of hardcoded
            // strs but this requires polymorphism, avoided in non
            // io-blocking code. Might think of a simpler solution later.
            // -----------------------------------------------------------
            // we do not use index property for views instead active_view is used
            // this is the only exception to this
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

// events
impl App {
    /// Handles all message events, delagates work to `self.event()` and `self.query()`.
    ///
    /// Takes in an event reciever `rx`, and a quit sender `q_tx`
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

    /// Handles read requests for the state
    fn query(&self, query: Query) {
        let query_error = || {
            tracing::error!("Could not send the data query.");
        };

        match query {
            Query::View { res } => {
                let _ = res.send(self.views.clone()).inspect_err(|_| query_error());
            }
            Query::MainMenu { res } => {
                let _ = res
                    .send(self.main_menu.clone())
                    .inspect_err(|_| query_error());
            }
        }
    }

    /// Handles input events, mutates the values in the state
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

    /// Handles the selection of items based on the current view.
    ///
    /// Takes in quit sender `q_tx` to be able to send messages
    /// to main for exiting the application
    fn handle_selection(&mut self, selected: &'static str, q_tx: Sender<()>) {
        match self.active_view {
            ActiveView::MainMenu => match selected {
                "Connection" => self.active_view = ActiveView::Connection,
                "VPN" => self.active_view = ActiveView::Vpn,
                "Config" => self.active_view = ActiveView::Config,
                "Exit" => {
                    tokio::task::block_in_place(|| {
                        let _ = q_tx
                            .blocking_send(())
                            .inspect_err(|e| tracing::error!("{e}\nCould not send quit request."));
                    });
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
