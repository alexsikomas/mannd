use derive_builder::Builder;
use futures::StreamExt;
use std::time::Duration;
use tracing::info;

use com::{
    controller::{Controller, DaemonType},
    state::{
        network::{NetworkAction, NetworkActor, NetworkState, handle_action},
        signals::{SignalManager, SignalUpdate},
    },
    wireless::common::{AccessPoint, AccessPointBuilderError, NetworkFlags},
};
use crossterm::event::{self, Event, EventStream};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::{
    error::TuiError,
    state::{
        ConnectionAction, ConnectionState, FocusedConnection, PromptState, SelectableList, UiData,
        UiDataBuilder, View, handle_event,
    },
    ui::render,
};

pub struct App;

#[derive(Builder, Debug)]
#[builder(pattern = "owned")]
pub struct AppState {
    // only used in main loop
    #[builder(default = "false")]
    should_quit: bool,
    #[builder(default = "true")]
    redraw: bool,

    #[builder(default = "UiDataBuilder::default().build().unwrap()")]
    pub ui_data: UiData,
}

impl AppState {
    fn new() -> Result<Self, AppStateBuilderError> {
        match AppStateBuilder::default().build() {
            Ok(a) => Ok(a),
            Err(e) => {
                tracing::error!("Error occured while creating the application state: {e}");
                Err(e)
            }
        }
    }
}

impl App {
    pub async fn run() -> Result<(), TuiError> {
        let mut state = match AppState::new() {
            Ok(s) => s,
            Err(e) => {
                return Err(TuiError::StateBuilder(e));
            }
        };

        // to network thread
        let (action_tx, action_rx) = mpsc::channel::<NetworkAction>(32);
        // from network thread
        let (state_tx, mut state_rx) = mpsc::channel::<NetworkState>(32);

        // we pass the sender to network due to how signals
        // work
        let action_tx_clone = action_tx.clone();
        tokio::spawn(async move {
            NetworkActor::new()
                .run(action_rx, action_tx_clone, state_tx)
                .await;
        });

        let mut terminal = ratatui::init();
        let mut events = EventStream::new();

        while !state.should_quit {
            if state.redraw {
                terminal.draw(|f| render(f, &state.ui_data))?;
                state.redraw = false;
            }

            tokio::select! {
                Some(msg) = state_rx.recv() => {
                    handle_state_update(&mut state, msg).await;
                    state.redraw = true;
                }
                Some(Ok(event)) = events.next() => {
                    state.redraw = true;
                    if let Some(action) = handle_event(event, &mut state.ui_data) {
                        handle_app_action(action, &mut state, &action_tx).await;
                    }
                }
                else => break,
            }
        }

        action_tx.send(NetworkAction::Exit).await;
        Ok(())
    }
}

async fn handle_state_update(state: &mut AppState, msg: NetworkState) {
    match msg {
        NetworkState::UpdateNetworks(aps) => {
            info!("Updating networks: {:?}", aps);
            state.ui_data.networks = SelectableList::new(aps);
            match &mut state.ui_data.view {
                View::Connection(conn_state) => {
                    ConnectionState::update_action_from_network(
                        conn_state,
                        &state.ui_data.networks,
                    );
                }
                _ => {}
            }
        }
        NetworkState::SetDaemon(daemon) => {
            state.ui_data.wifi_daemon = Some(daemon);
        }
        NetworkState::ConnectFailed(reason) => {
            todo!()
        }
    }
}

async fn handle_app_action(
    action: AppAction,
    state: &mut AppState,
    net_tx: &Sender<NetworkAction>,
) {
    match action {
        AppAction::Network(action) => {
            net_tx.send(action).await;
        }
        AppAction::AddPrompt(prompt) => {
            state.ui_data.prompt_stack.push(prompt);
        }
        AppAction::Exit => {
            state.should_quit = true;
        }
        _ => {}
    }
}

pub enum AppAction {
    Network(NetworkAction),
    AddPrompt(PromptState),
    Exit,
}
