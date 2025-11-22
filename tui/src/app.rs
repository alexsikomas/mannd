use futures::StreamExt;
use std::time::Duration;
use tracing::info;

use com::{
    controller::{Controller, DaemonType},
    state::{
        network::{handle_action, NetUpdate, NetworkAction, NetworkActor},
        signals::{SignalManager, SignalUpdate},
    },
};
use crossterm::event::{self, Event, EventStream};
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
    fn new() -> Self {
        Self {
            view_state: State::main_menu(),
            is_running: true,
            redraw: true,
            prompt_view: None,
        }
    }
}

impl App {
    pub async fn run() -> Result<(), TuiError> {
        let mut state = AppState::new();
        // to network thread
        let (action_tx, mut action_rx) = mpsc::channel::<NetworkAction>(32);
        // from network thread
        let (state_tx, mut state_rx) = mpsc::channel::<NetUpdate>(32);

        let net_thread_act_tx = action_tx.clone();
        tokio::spawn(async move {
            NetworkActor::new()
                .run(action_rx, net_thread_act_tx, state_tx)
                .await;
        });

        let mut terminal = ratatui::init();
        let mut events = EventStream::new();

        while state.is_running {
            if state.redraw {
                terminal.draw(|f| render(f, &state))?;
                state.redraw = false;
            }

            tokio::select! {
                Some(msg) = state_rx.recv() => {
                    handle_net_state_msg(&mut state, msg);
                    state.redraw = true;
                }
                Some(Ok(event)) = events.next() => {
                    state.redraw = true;
                    match event {
                        Event::Key(key) => {
                            handle_key_event(&mut state, key, &action_tx).await;
                        }
                        _ => {}
                    }
                }
                else => break,
            }
        }

        action_tx.send(NetworkAction::Exit).await;
        Ok(())
    }
}

async fn handle_key_event(
    state: &mut AppState,
    key: event::KeyEvent,
    action_tx: &mpsc::Sender<NetworkAction>,
) {
    // are we dealing with prompt or normal menu?
    let action = match &mut state.prompt_view {
        Some(prompt) => prompt.handle_input(key.code),
        None => state.view_state.handle_input(key.code),
    };

    match action {
        Some(UpdateAction::Network(action)) => {
            if let Err(e) = action_tx.send(action).await {
                tracing::error!("Failed to send network action! {e}");
                state.is_running = false;
            }
        }
        Some(UpdateAction::Exit) => state.is_running = false,
        Some(UpdateAction::OpenPrompt(prompt)) => state.prompt_view = Some(prompt),
        Some(UpdateAction::ExitPrompt) => state.prompt_view = None,
        None => {}
    }

    if let State::Connection(conn_state) = &mut state.view_state {
        conn_state.refresh_actions();
    }
}

fn handle_net_state_msg(state: &mut AppState, msg: NetUpdate) {
    match msg {
        NetUpdate::UpdateAps(aps) => {
            // state.network.aps = aps.clone();
            info!("{:?}", aps);
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
}

pub enum UpdateAction {
    Network(NetworkAction),
    OpenPrompt(PromptState),
    ExitPrompt,
    Exit,
}
