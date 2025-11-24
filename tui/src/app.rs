use derive_builder::Builder;
use futures::StreamExt;
use std::time::Duration;
use tracing::info;

use com::{
    controller::{Controller, DaemonType},
    state::{
        network::{handle_action, NetUpdate, NetworkAction, NetworkActor},
        signals::{SignalManager, SignalUpdate},
    },
    wireless::common::{AccessPoint, AccessPointBuilderError},
};
use crossterm::event::{self, Event, EventStream};
use tokio::sync::mpsc::{self, Receiver};

use crate::{
    error::TuiError,
    state::{
        ConnectionAction, ConnectionState, FocusedConnection, PromptState, SelectableList, UiData,
        UiDataBuilder, View,
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

        while !state.should_quit {
            if state.redraw {
                terminal.draw(|f| render(f, &state.ui_data))?;
                state.redraw = false;
            }

            tokio::select! {
                Some(msg) = state_rx.recv() => {
                    state.redraw = true;
                }
                Some(Ok(event)) = events.next() => {
                    state.redraw = true;
                    match event {
                        Event::Key(key) => {
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

pub enum UpdateAction {
    Network(NetworkAction),
    OpenPrompt(PromptState),
    ExitPrompt,
    Exit,
}
