use postcard::{from_bytes_cobs, to_stdvec_cobs};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::info;

use crate::{
    state::{AppContext, InfoPrompt, PopupType, PromptState, StateCommand, UiState, View},
    ui::{UiContext, UiMessage},
};
use com::{
    controller::DaemonType,
    error::ManndError,
    state::network::{NetFailure, NetStart, NetSuccess, NetworkAction, NetworkActor, NetworkState},
    wireguard::store::WgMeta,
    wireless::common::AccessPoint,
};
use crossterm::event::EventStream;
use futures::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixSocket, UnixStream},
    process::Child,
    sync::mpsc::{self, Sender},
};

pub struct App {
    stream: UnixStream,
    child: Option<Child>,
}

pub struct AppState {
    // only used in main loop
    should_quit: bool,
    redraw: bool,
    ui: UiState,

    networks: Vec<AccessPoint>,
    daemon_type: Option<DaemonType>,
    wg_info: (Vec<String>, Vec<WgMeta>),
}

impl AppState {
    fn new() -> Self {
        AppState {
            should_quit: false,
            redraw: true,
            ui: UiState::new(),
            networks: vec![],
            daemon_type: None,
            wg_info: (vec![], vec![]),
        }
    }
}

impl App {
    pub fn new(stream: UnixStream, child: Option<Child>) -> Self {
        Self { stream, child }
    }

    pub async fn run(&mut self) -> Result<(), ManndError> {
        let mut state = AppState::new();

        // to network thread
        // let (action_tx, action_rx) = mpsc::channel::<NetworkAction>(32);
        // from network thread
        // let (state_tx, mut state_rx) = mpsc::channel::<NetworkState>(32);

        let (reader, writer) = self.stream.split();
        let mut reader = FramedRead::new(reader, LengthDelimitedCodec::new());
        let mut writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

        let (sock_tx, mut sock_rx) = mpsc::channel::<Vec<u8>>(32);

        let mut terminal = ratatui::init();
        let mut events = EventStream::new();

        let mut ui_context = UiContext::new();

        while !state.should_quit {
            let mut data = vec![0u8; 1024];
            if state.redraw {
                let context = AppContext::create(
                    &state.networks,
                    &state.daemon_type,
                    (&state.wg_info.0, &state.wg_info.1),
                    state.ui.vpn_cols,
                );
                let _ = terminal.draw(|f| ui_context.render(f, &state.ui, &context));
                state.redraw = false;
                match &ui_context.message {
                    Some(msg) => {
                        handle_ui_message(&mut state, msg);
                    }
                    None => {}
                };
            }

            tokio::select! {
                Some(msg) = sock_rx.recv() => {
                    writer.send(msg.into()).await.map_err(|_| ManndError::SocketWrite)?;
                }
                Some(frame_res) = reader.next() => {
                    let mut frame = frame_res?;
                    let msg = from_bytes_cobs::<NetworkState>(&mut frame)?;
                    match msg {
                        NetworkState::CallAction(action) => {
                        }
                        _ => {
                            if let Some(cmd) = handle_state_update(&mut state, msg).await {
                                state.ui.process_command(cmd);
                            }
                        }
                    };
                    state.redraw = true;
                }
                Some(Ok(event)) = events.next() => {
                    state.redraw = true;
                    let context = AppContext::create(&state.networks,
                        &state.daemon_type,
                        (&state.wg_info.0, &state.wg_info.1),
                        state.ui.vpn_cols,
                    );
                    if let Some(action) = state.ui.handle_event(event, &context) {
                        handle_app_action(action, &mut state, &sock_tx).await;
                    }
                }
                else => break,
            }
        }

        let exit_msg = to_stdvec_cobs(&NetworkAction::Exit)?;
        writer
            .send(exit_msg.into())
            .await
            .map_err(|_| ManndError::SocketWrite)?;
        Ok(())
    }
}

async fn handle_state_update(state: &mut AppState, msg: NetworkState) -> Option<StateCommand> {
    match msg {
        NetworkState::UpdateNetworks(aps) => {
            state.networks = aps;
            match &mut state.ui.current_view {
                View::Connection(conn_state) => {
                    conn_state.refresh_available_actions(&state.networks);
                }
                _ => {}
            }
        }
        NetworkState::SetDaemon(daemon) => {
            state.daemon_type = Some(daemon);
        }
        NetworkState::UpdateWgDb((names, meta)) => {
            state.wg_info.0 = names;
            state.wg_info.1 = meta;
        }
        NetworkState::Start(started) => return handle_start(state, started),
        NetworkState::Success(succeeded) => return handle_success(state, succeeded),
        NetworkState::Failed(failure) => return handle_failure(state, failure),
        _ => {}
    };
    None
}

fn handle_start(state: &mut AppState, started: NetStart) -> Option<StateCommand> {
    match started {
        NetStart::Connection => {
            state.ui.should_block = true;
            Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
                "Connecting...".to_string(),
                PopupType::General,
            ))))
        }
        NetStart::Scan => Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
            "Scanning...".to_string(),
            PopupType::General,
        )))),
    }
}

fn handle_success(state: &mut AppState, succeeded: NetSuccess) -> Option<StateCommand> {
    match succeeded {
        NetSuccess::Connection => {
            state.ui.should_block = false;
            return Some(StateCommand::ClearPrompts);
        }
        NetSuccess::Scan => {
            return Some(StateCommand::ClearPrompts);
        }
        _ => {}
    };
    None
}

fn handle_failure(state: &mut AppState, failed: NetFailure) -> Option<StateCommand> {
    match failed {
        NetFailure::Connection(err) => {
            state.ui.should_block = false;
            return Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
                err,
                PopupType::Error,
            ))));
        }
    }
}

async fn handle_app_action(action: AppAction, state: &mut AppState, net_tx: &Sender<Vec<u8>>) {
    match action {
        AppAction::Network(action) => {
            let res = to_stdvec_cobs(&action).unwrap();
            let _ = net_tx.send(res).await;
        }
        AppAction::AddPrompt(prompt) => {
            state.ui.prompt_stack.push(prompt);
        }
        AppAction::Exit => {
            state.should_quit = true;
        }
    }
}

fn handle_ui_message(state: &mut AppState, msg: &UiMessage) {
    match msg {
        UiMessage::SetVpnCols(cols) => {
            state.ui.vpn_cols = *cols;
        }
    }
}

#[derive(Debug)]
pub enum AppAction {
    Network(NetworkAction),
    AddPrompt(PromptState),
    Exit,
}
