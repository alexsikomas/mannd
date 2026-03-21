use std::time::Duration;

use postcard::{from_bytes_cobs, to_stdvec_cobs};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::{info, instrument};

use crate::{
    state::{AppContext, InfoPrompt, PopupType, PromptState, StateCommand, UiState, View},
    ui::{UiContext, UiMessage},
};
use crossterm::event::{EventStream, read};
use futures::{SinkExt, StreamExt};
use mannd::{
    error::ManndError,
    state::network::{
        Capability, Failure, InterfaceTypes, NetCtx, NetworkAction, NetworkState, Start, Success,
    },
};
use tokio::{
    net::{
        UnixStream,
        unix::{ReadHalf, WriteHalf},
    },
    process::Child,
    sync::mpsc::{self, Sender},
    time::timeout,
};

pub struct App {
    stream: UnixStream,
}

pub struct AppState {
    should_quit: bool,
    redraw: bool,

    caps: Capability,
    net_ctx: NetCtx,
}

impl AppState {
    fn new() -> Self {
        AppState {
            should_quit: false,
            redraw: true,
            caps: Capability::default(),
            net_ctx: NetCtx::default(),
        }
    }
}

impl App {
    pub fn new(stream: UnixStream) -> Self {
        Self { stream }
    }

    #[instrument(err, skip(self))]
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
        let mut ui = UiState::new(caps.clone())?;
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

        ratatui::restore();
        Ok(())
    }
}

/// Requests the capabilities of the system to know what to
/// display on screen for example: Wi-Fi, wireguard, networkd
#[instrument(err, skip_all)]
async fn init_request(
    writer: &mut FramedWrite<WriteHalf<'_>, LengthDelimitedCodec>,
    reader: &mut FramedRead<ReadHalf<'_>, LengthDelimitedCodec>,
) -> Result<Capability, ManndError> {
    let req = to_stdvec_cobs(&NetworkAction::GetCapabilities)?;
    writer
        .send(req.into())
        .await
        .map_err(|_| ManndError::SocketWrite)?;

    let timeout_duration = Duration::from_secs(5);

    let read_future = async {
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
    };

    match timeout(timeout_duration, read_future).await {
        Ok(res) => res,
        Err(_) => Err(ManndError::Timeout),
    }
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
        NetworkState::ToggleWpaPersist => {
            state.net_ctx.persist_wpa_changes = !state.net_ctx.persist_wpa_changes;
        }
        NetworkState::SetWireguardInfo((names, meta)) => {
            state.net_ctx.wg_ctx.names = names;
            state.net_ctx.wg_ctx.meta = meta;
        }
        NetworkState::SetInterfaces(ifaces) => {
            state.net_ctx.interfaces = Some(InterfaceTypes::Normal(ifaces));
        }
        NetworkState::SetWpaInterfaces(ifaces) => {
            state.net_ctx.interfaces = Some(InterfaceTypes::Wpa(ifaces));
        }
        NetworkState::Start(started) => return handle_start(state, ui, started),
        NetworkState::Success(succeeded) => return handle_success(state, ui, succeeded),
        NetworkState::Failed(failure) => return handle_failure(state, ui, failure),
        _ => {}
    };
    None
}

fn handle_start(_state: &mut AppState, ui: &mut UiState, started: Start) -> Option<StateCommand> {
    match started {
        Start::Wifi => {
            ui.should_block = true;
            Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
                "Connecting...".to_string(),
                PopupType::General,
            ))))
        }
        Start::Scan => Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
            "Scanning...".to_string(),
            PopupType::General,
        )))),
    }
}

fn handle_success(
    state: &mut AppState,
    ui: &mut UiState,
    succeeded: Success,
) -> Option<StateCommand> {
    match succeeded {
        Success::Wifi => {
            ui.should_block = false;
            return Some(StateCommand::ClearPrompts);
        }
        Success::Scan => {
            return Some(StateCommand::ClearPrompts);
        }
        Success::EnableWireguard => {
            state.net_ctx.wg_ctx.is_on = true;
        }
        Success::DisableWireguard => {
            state.net_ctx.wg_ctx.is_on = false;
        }
    };
    None
}

fn handle_failure(
    _state: &mut AppState,
    ui: &mut UiState,
    failed: Failure,
) -> Option<StateCommand> {
    match failed {
        Failure::Wifi(err) => {
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
