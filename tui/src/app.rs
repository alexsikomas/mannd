use std::{cmp::min, time::Duration};

use postcard::{from_bytes_cobs, to_stdvec_cobs};
use ratatui::layout::Rect;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::instrument;

use crate::{
    components::{layout::get_inner_area, wireguard_ui::WireguardMenu},
    state::{AppContext, PopupType, PromptState, StateCommand, UiState, View, prompts::InfoPrompt},
    ui::UiContext,
};
use crossterm::event::EventStream;
use futures::{SinkExt, StreamExt};
use mannd::{
    error::ManndError,
    state::messages::{
        Capability, Failure, NetworkAction, NetworkState, Process, Started, Success,
    },
    store::WgMeta,
    wireless::{common::AccessPoint, wpa_supplicant::WpaInterface},
};
use tokio::{
    net::{
        UnixStream,
        unix::{ReadHalf, WriteHalf},
    },
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

        let caps = init_request(&mut writer, &mut reader).await?;
        let mut ui = UiState::new(caps.clone())?;
        state.caps = caps;

        while !state.should_quit {
            if state.redraw {
                if let View::Vpn(_) = &ui.current_view {
                    let terminal_size = terminal.size()?;
                    let terminal_area = Rect::from(terminal_size);
                    let inner_area = get_inner_area(terminal_area);
                    let mut cols: usize = 0;
                    let _ = WireguardMenu::build_layout_no_render(inner_area, &mut cols);
                    ui.vpn_cols = cols;
                }
                let context =
                    AppContext::create(&state.net_ctx, &state.caps.wifi_daemon, ui.vpn_cols);
                let _ = terminal.draw(|f| UiContext::render(f, &ui, &context));
                state.redraw = false;
            }

            tokio::select! {
                Some(msg) = sock_rx.recv() => {
                    writer.send(msg.into()).await.map_err(|_| ManndError::SocketWrite)?;
                }
                Some(frame_res) = reader.next() => {
                    let mut frame = frame_res?;
                    let msg = from_bytes_cobs::<NetworkState>(&mut frame)?;
                    if let Some(cmd) = handle_state_update(&mut state, &mut ui, msg).await {
                        ui.process_commands([cmd], &state.caps);
                    }
                    state.redraw = true;
                }
                Some(Ok(event)) = events.next() => {
                    state.redraw = true;
                    let context = AppContext::create(&state.net_ctx,
                        &state.caps.wifi_daemon, ui.vpn_cols,
                    );
                    for action in ui.handle_event(event, &context, &state.caps) {
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
            if let View::Wifi(wifi_state) = &mut ui.current_view {
                wifi_state.refresh_available_actions(&state.net_ctx.networks);
            }
        }
        NetworkState::ToggleWpaPesist => {
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
    }
    None
}

fn handle_start(_state: &mut AppState, ui: &mut UiState, started: Started) -> Option<StateCommand> {
    match started {
        Started(Process::WifiConnect) => {
            ui.should_block = true;
            Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
                "Connecting...".to_string(),
                PopupType::General,
            ))))
        }
        Started(Process::WifiScan) => Some(StateCommand::Prompt(PromptState::Info(
            InfoPrompt::new("Scanning...".to_string(), PopupType::General),
        ))),
        Started(Process::Wireguard) => todo!(),
    }
}

fn handle_success(
    state: &mut AppState,
    ui: &mut UiState,
    succeeded: Success,
) -> Option<StateCommand> {
    match succeeded {
        Success::Generic => {
            ui.should_block = false;
            return Some(StateCommand::ClearPrompts);
        }
        Success::EnableWireguard => {
            state.net_ctx.wg_ctx.is_on = true;
        }
        Success::DisableWireguard => {
            state.net_ctx.wg_ctx.is_on = false;
        }
    }
    None
}

fn handle_failure(
    _state: &mut AppState,
    ui: &mut UiState,
    failed: Failure,
) -> Option<StateCommand> {
    match failed.process {
        Process::WifiConnect => {
            ui.should_block = false;
            Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
                failed.reason,
                PopupType::Error,
            ))))
        }
        Process::WifiScan => Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
            failed.reason,
            PopupType::Error,
        )))),
        Process::Wireguard => todo!(),
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

#[derive(Debug)]
pub enum AppAction {
    Network(NetworkAction),
    AddPrompt(PromptState),
    Exit,
}

pub struct NetworkContext {
    pub networks: Vec<AccessPoint>,
    pub interfaces: Option<InterfaceTypes>,
    pub wg_ctx: WireguardContext,
    pub persist_wpa_changes: bool,
    pub netd_files: Vec<String>,
}

pub struct WireguardContext {
    pub names: Vec<String>,
    pub meta: Vec<WgMeta>,
    pub is_on: bool,
}

impl WireguardContext {
    fn new() -> Self {
        Self {
            names: vec![],
            meta: vec![],
            is_on: false,
        }
    }

    pub fn len(&self) -> usize {
        min(self.names.len(), self.meta.len())
    }

    pub fn get_index(&self, index: usize) -> Option<(&str, &WgMeta)> {
        if let Some(name) = self.names.get(index) {
            if let Some(meta) = self.meta.get(index) {
                return Some((name, meta));
            }
        }
        None
    }
}

pub enum InterfaceTypes {
    Wpa(Vec<WpaInterface>),
    Normal(Vec<String>),
}

impl InterfaceTypes {
    pub fn len(&self) -> usize {
        match self {
            Self::Wpa(ifaces) => ifaces.len(),
            Self::Normal(ifaces) => ifaces.len(),
        }
    }

    pub fn get_wpa_index(&self, index: usize) -> Option<&WpaInterface> {
        match self {
            Self::Wpa(ifaces) => ifaces.get(index),
            _ => None,
        }
    }

    pub fn get_normal_index(&self, index: usize) -> Option<&str> {
        match self {
            Self::Normal(ifaces) => ifaces.get(index).map(|s| s.as_str()),
            _ => None,
        }
    }
}

impl Default for NetworkContext {
    fn default() -> Self {
        Self {
            networks: vec![],
            interfaces: None,
            wg_ctx: WireguardContext::new(),
            persist_wpa_changes: false,
            netd_files: vec![],
        }
    }
}
