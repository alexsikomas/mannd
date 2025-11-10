use std::time::Duration;

use crossterm::event::{self, Event};
use tokio::sync::mpsc::{self, Receiver};

use crate::{
    error::TuiError,
    network::{network_handle, NetworkAction, NetworkUpdate},
    state::{
        ConnectionAction, ConnectionState, FocusedConnection, PromptState, SelectableList, State,
    },
    ui::render,
};

pub struct App;

pub struct AppState {
    is_running: bool,
    redraw: bool,
    pub view_state: State,
    // for prompts inside of a view state
    pub prompt_view: Option<PromptState>,
}

impl AppState {
    async fn new() -> Self {
        Self {
            view_state: State::main_menu(),
            is_running: true,
            redraw: false,
            prompt_view: None,
        }
    }
}

impl App {
    pub async fn run() -> Result<(), TuiError> {
        let mut state = AppState::new().await;
        // to network thread
        let (net_action_tx, mut net_action_rx) = mpsc::channel::<NetworkAction>(32);
        // from network thread
        let (net_update_tx, mut net_update_rx) = mpsc::channel::<NetworkUpdate>(32);

        let mut terminal = ratatui::init();

        // networking thread
        tokio::spawn(async move {
            network_handle(&mut net_action_rx, net_update_tx).await;
        });

        terminal.draw(|f| render(f, &state))?;

        while state.is_running {
            handle_net_state_msg(&mut state, &mut net_update_rx);

            if event::poll(Duration::from_millis(100))? {
                if let Ok(Event::Key(key)) = event::read() {
                    let mut action: Option<UpdateAction> = None;

                    // are we dealing with prompt or normal menu?
                    match &mut state.prompt_view {
                        Some(prompt) => {
                            action = prompt.handle_input(key.code);
                        }
                        None => {
                            action = state.view_state.handle_input(key.code);
                        }
                    };

                    match action {
                        Some(UpdateAction::Network(action)) => {
                            let _ = net_action_tx.send(action).await;
                        }
                        Some(UpdateAction::Exit) => {
                            state.is_running = false;
                        }
                        Some(UpdateAction::OpenPrompt(prompt)) => {
                            state.prompt_view = Some(prompt);
                        }
                        Some(UpdateAction::ExitPrompt) => {
                            state.prompt_view = None;
                        }
                        None => {}
                    };
                };

                match &mut state.view_state {
                    State::Connection(conn_state) => {
                        if matches!(conn_state.focused_list, FocusedConnection::Networks) {
                            let mut action_list = vec![ConnectionAction::Scan];

                            if let Some(network) =
                                &conn_state.networks.items.get(conn_state.networks.selected)
                            {
                                if !network.connected {
                                    action_list.push(ConnectionAction::Connect);
                                } else {
                                    // action_list.push(ConnectionAction::Info);
                                    action_list.push(ConnectionAction::Disconnect);
                                }
                                if network.known {
                                    action_list.push(ConnectionAction::Forget);
                                }
                            }
                            conn_state.actions = SelectableList::new(action_list);
                        }
                    }
                    _ => {}
                };

                state.redraw = true;
            }

            if state.redraw {
                terminal.draw(|f| render(f, &state))?;
                state.redraw = false;
            }
        }

        net_action_tx.send(NetworkAction::Exit).await;
        Ok(())
    }
}

fn handle_net_state_msg(state: &mut AppState, net_update_rx: &mut Receiver<NetworkUpdate>) {
    if let Ok(msg) = net_update_rx.try_recv() {
        match msg {
            NetworkUpdate::Select(i) => {
                // state.network.selected = Some(i);
            }
            NetworkUpdate::Deselect => {
                // state.network.selected = None;
            }
            NetworkUpdate::UpdateAps(aps) => {
                // state.network.aps = aps.clone();
                match &state.view_state {
                    State::Connection(conn_state) => {
                        if conn_state.networks.items.is_empty() {
                            state.view_state = State::Connection(ConnectionState::new(aps));
                        } else {
                            let selected_network = conn_state.networks.get_selected_value();
                            let cached_actions = conn_state.actions.clone();

                            let mut new_state = ConnectionState::new(aps);

                            let index = new_state
                                .networks
                                .items
                                .iter()
                                .position(|v| v.ssid == selected_network.ssid);

                            match index {
                                Some(val) => {
                                    new_state.networks.selected = val;
                                    new_state.actions = cached_actions;
                                }
                                None => {
                                    // since non-empty
                                    new_state.networks.selected = 0;
                                }
                            }

                            state.view_state = State::Connection(new_state);
                        }
                    }
                    _ => {}
                }
            }
            NetworkUpdate::AddKnownNetworks(aps) => {
                // state.view_state = State::Connection(state.)
            }
            NetworkUpdate::Update => {}
        };
        state.redraw = true;
    };
}

pub enum UpdateAction {
    Network(NetworkAction),
    OpenPrompt(PromptState),
    ExitPrompt,
    Exit,
}
