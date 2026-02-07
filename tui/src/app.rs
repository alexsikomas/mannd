use postcard::{from_bytes_cobs, to_stdvec_cobs};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::{
    state::{AppContext, InfoPrompt, PopupType, PromptState, StateCommand, UiState, View},
    ui::{UiContext, UiMessage},
};
use com::{
    error::ManndError,
    state::network::{
        Capability, NetFailure, NetStart, NetSuccess, NetworkAction, NetworkContext, NetworkState,
    },
};
use crossterm::event::EventStream;
use futures::{SinkExt, StreamExt};
use tokio::{
    net::{
        unix::{ReadHalf, WriteHalf},
        UnixStream,
    },
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

    caps: Capability,
    net_ctx: NetworkContext,
}

impl AppState {
    fn new() -> Self {
        AppState {
            should_quit: false,
            redraw: true,
            caps: Capability::default(),
            net_ctx: NetworkContext::default(),
        }
    }
}

impl App {
    pub fn new(stream: UnixStream, child: Option<Child>) -> Self {
        Self { stream, child }
    }

    pub async fn run(&mut self) -> Result<(), ManndError> {
        let mut state = AppState::new();

        let (reader, writer) = self.stream.split();
        let mut reader = FramedRead::new(reader, LengthDelimitedCodec::new());
        let mut writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

        let (sock_tx, mut sock_rx) = mpsc::channel::<Vec<u8>>(32);

        let mut terminal = ratatui::init();
        let mut events = EventStream::new();

        let mut ui_context = UiContext::new();

        let caps = init_request(&mut writer, &mut reader).await?;
        let mut ui = UiState::new(caps.clone());
        state.caps = caps;

        while !state.should_quit {
            if state.redraw {
                let context = AppContext::create(&state.net_ctx, &state.caps, ui.vpn_cols);
                let _ = terminal.draw(|f| ui_context.render(f, &ui, &context));
                state.redraw = false;
                match &ui_context.message {
                    Some(msg) => {
                        handle_ui_message(&mut ui, msg);
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
                            let _ = sock_tx.send(to_stdvec_cobs(&action)?).await;
                        }
                        _ => {
                            if let Some(cmd) = handle_state_update(&mut state, &mut ui, msg).await {
                                ui.process_commands([cmd]);
                            }
                        }
                    };
                    state.redraw = true;
                }
                Some(Ok(event)) = events.next() => {
                    state.redraw = true;
                    let context = AppContext::create(&state.net_ctx,
                        &state.caps,
                        ui.vpn_cols,
                    );
                    for action in ui.handle_event(event, &context) {
                        handle_app_action(action, &mut state, &mut ui, &sock_tx).await;
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

/// Requests the capabilities of the system to know what
/// to display on screen i.e. wifi, wireguard, networkd
async fn init_request(
    writer: &mut FramedWrite<WriteHalf<'_>, LengthDelimitedCodec>,
    reader: &mut FramedRead<ReadHalf<'_>, LengthDelimitedCodec>,
) -> Result<Capability, ManndError> {
    let req = to_stdvec_cobs(&NetworkAction::GetCapabilities)?;
    writer
        .send(req.into())
        .await
        .map_err(|_| ManndError::SocketWrite)?;

    let max_tries = 10;
    let mut tries = 0;
    while let Some(frame_res) = reader.next().await {
        if tries > max_tries {
            return Err(ManndError::Timeout);
        }

        let mut frame = frame_res?;
        let msg = from_bytes_cobs::<NetworkState>(&mut frame)?;
        match msg {
            NetworkState::SetCapabilities(caps) => return Ok(caps),
            _ => {
                tries += 1;
            }
        }
    }

    Err(ManndError::Timeout)
}

async fn handle_state_update(
    state: &mut AppState,
    ui: &mut UiState,
    msg: NetworkState,
) -> Option<StateCommand> {
    match msg {
        NetworkState::SetNetworks(aps) => {
            state.net_ctx.networks = aps;
            match &mut ui.current_view {
                View::Wifi(wifi_state) => {
                    wifi_state.refresh_available_actions(&state.net_ctx.networks);
                }
                _ => {}
            }
        }
        NetworkState::SetWireguardInfo((names, meta)) => {
            state.net_ctx.wg_info.0 = names;
            state.net_ctx.wg_info.1 = meta;
        }
        NetworkState::SetInterfaces(ifaces) => {
            state.net_ctx.interfaces = ifaces;
        }
        NetworkState::Start(started) => return handle_start(ui, started),
        NetworkState::Success(succeeded) => return handle_success(ui, succeeded),
        NetworkState::Failed(failure) => return handle_failure(ui, failure),
        _ => {}
    };
    None
}

fn handle_start(ui: &mut UiState, started: NetStart) -> Option<StateCommand> {
    match started {
        NetStart::Wifi => {
            ui.should_block = true;
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

fn handle_success(ui: &mut UiState, succeeded: NetSuccess) -> Option<StateCommand> {
    match succeeded {
        NetSuccess::Wifi => {
            ui.should_block = false;
            return Some(StateCommand::ClearPrompts);
        }
        NetSuccess::Scan => {
            return Some(StateCommand::ClearPrompts);
        }
        _ => {}
    };
    None
}

fn handle_failure(ui: &mut UiState, failed: NetFailure) -> Option<StateCommand> {
    match failed {
        NetFailure::Wifi(err) => {
            ui.should_block = false;
            return Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
                err,
                PopupType::Error,
            ))));
        }
    }
}

async fn handle_app_action(
    action: AppAction,
    state: &mut AppState,
    ui: &mut UiState,
    net_tx: &Sender<Vec<u8>>,
) {
    match action {
        AppAction::Network(action) => {
            let res = to_stdvec_cobs(&action).unwrap();
            let _ = net_tx.send(res).await;
        }
        AppAction::AddPrompt(prompt) => {
            ui.prompt_stack.push(prompt);
        }
        AppAction::Exit => {
            state.should_quit = true;
        }
    }
}

fn handle_ui_message(ui: &mut UiState, msg: &UiMessage) {
    match msg {
        UiMessage::SetVpnCols(cols) => {
            ui.vpn_cols = *cols;
        }
    }
}

#[derive(Debug)]
pub enum AppAction {
    Network(NetworkAction),
    AddPrompt(PromptState),
    Exit,
}
