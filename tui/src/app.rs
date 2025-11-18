use std::time::Duration;
use tracing::info;

use com::{
    controller::Controller,
    state::{
        network::{handle_action, NetUpdate, NetworkAction},
        signals::{SignalManager, SignalUpdate},
    },
};
use crossterm::event::{self, Event};
use tokio::sync::mpsc::{self, Receiver};

use crate::{
    error::TuiError,
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
        let (state_update_tx, mut state_update_rx) = mpsc::channel::<NetUpdate>(32);
        let signal_net_action = net_action_tx.clone();

        let mut terminal = ratatui::init();

        // networking thread
        // Signal <-> Network -> UI Update
        //
        // A signal update leads to a network update i.e. if networks are
        // loaded and we get a signal then this leads to getting the
        // network values via a network update
        tokio::spawn(async move {
            let (signal_tx, mut signal_rx) = mpsc::channel::<SignalUpdate>(32);
            let mut signal_manager = SignalManager::new();

            if let Ok(mut controller) = Controller::new().await {
                controller.determine_adapter().await;
                let daemon = controller.wifi.as_ref().unwrap().daemon_type();
                loop {
                    tokio::select! {
                        Some(action) = net_action_rx.recv() => {
                            if handle_action(&mut controller, state_update_tx.clone(), signal_tx.clone(), action).await {
                                break;
                            }
                        }
                        // add new signals to listen for
                        Some(update) = signal_rx.recv() => {
                            signal_manager.handle_update(update);
                        }
                        Some(msg) = signal_manager.recv() => {
                            match daemon {
                                // iwd
                                1 => {
                                    signal_manager.process_iwd_msg(msg, signal_net_action.clone()).await;
                                }
                                // wpa
                                2 => {
                                    signal_manager.process_wpa_msg(msg, signal_net_action.clone()).await;
                                }
                                _ => {
                                    break;
                                }
                            }
                        }
                    };
                }
            }
        });

        terminal.draw(|f| render(f, &state))?;

        while state.is_running {
            handle_net_state_msg(&mut state, &mut state_update_rx);

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

fn handle_net_state_msg(state: &mut AppState, net_update_rx: &mut Receiver<NetUpdate>) {
    if let Ok(msg) = net_update_rx.try_recv() {
        match msg {
            NetUpdate::UpdateAps(aps) => {
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
            NetUpdate::AddKnownNetworks(aps) => {
                // state.view_state = State::Connection(state.)
            }
            NetUpdate::UpdateApsHidden(aps) => {}
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
