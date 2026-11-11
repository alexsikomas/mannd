use std::cmp::min;

use postcard::{from_bytes_cobs, to_stdvec_cobs};
use ratatui::{DefaultTerminal, layout::Rect};
use tokio_util::{
    bytes::Bytes,
    codec::{Framed, LengthDelimitedCodec},
};
use tracing::instrument;

use crate::{
    components::{layout::get_inner_area, wireguard_ui::VpnMenu},
    state::{AppContext, PopupType, PromptState, StateCommand, UiState, View, prompts::InfoPrompt},
    ui::UiContext,
};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, EventStream};
use futures::{SinkExt, StreamExt};
use mannd::{
    error::ManndError,
    state::messages::{
        Capability, Failure, NetworkAction, NetworkState, Process, Started, Success,
    },
    store::{NetworkInfo, WgMeta},
    wireless::{common::NetworkFlags, wpa_supplicant::WpaInterface},
};
use tokio::{
    net::UnixStream,
    sync::mpsc::{self, Sender},
};

/// Connection to the mannd daemon
pub type Backend = Framed<UnixStream, LengthDelimitedCodec>;

pub struct App {
    backend: Backend,
    caps: Capability,
}

pub struct AppState {
    should_quit: bool,
    redraw: bool,

    caps: Capability,
    net_ctx: NetworkContext,
}

impl AppState {
    fn new(caps: Capability) -> Self {
        AppState {
            should_quit: false,
            redraw: true,
            caps,
            net_ctx: NetworkContext::default(),
        }
    }
}

impl App {
    /// Backend answers with caps
    pub fn new(backend: Backend, caps: Capability) -> Self {
        Self { backend, caps }
    }

    /// Runs until the user quits or the daemon is lost, the terminal is
    /// handed back in either case
    #[instrument(err, skip(self))]
    pub async fn run(&mut self) -> Result<(), ManndError> {
        let mut terminal = ratatui::init();
        let res = self.event_loop(&mut terminal).await;

        let _ = crossterm::execute!(std::io::stdout(), DisableBracketedPaste);
        ratatui::restore();
        res
    }

    async fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<(), ManndError> {
        crossterm::execute!(std::io::stdout(), EnableBracketedPaste)?;

        let mut state = AppState::new(self.caps.clone());
        let mut ui = UiState::new(self.caps.clone())?;
        let mut events = EventStream::new();
        let (sock_tx, mut sock_rx) = mpsc::channel::<Vec<u8>>(32);

        while !state.should_quit {
            if state.redraw {
                if let View::Vpn(_) = &ui.current_view {
                    let terminal_size = terminal.size()?;
                    let terminal_area = Rect::from(terminal_size);
                    let inner_area = get_inner_area(terminal_area);
                    ui.vpn_cols = VpnMenu::build_layout_no_render(inner_area).cols;
                }
                let context = AppContext {
                    net_ctx: &state.net_ctx,
                    wifi_daemon: &state.caps.wifi_daemon,
                    vpn_cols: ui.vpn_cols,
                };
                let _ = terminal.draw(|f| UiContext::render(f, &ui, &context));
                state.redraw = false;
            }

            tokio::select! {
                Some(msg) = sock_rx.recv() => {
                    self.backend
                        .send(Bytes::from(msg))
                        .await
                        .map_err(|_| lost_connection())?;
                }
                frame = self.backend.next() => {
                    let Some(Ok(mut frame)) = frame else {
                        return Err(lost_connection());
                    };
                    let msg = from_bytes_cobs::<NetworkState>(&mut frame)?;
                    if let Some(cmd) = handle_state_update(&mut state, &mut ui, msg).await {
                        ui.process_commands([cmd], &state.caps);
                    }
                    state.redraw = true;
                }
                Some(Ok(event)) = events.next() => {
                    state.redraw = true;
                    let context = AppContext {
                        net_ctx: &state.net_ctx,
                        wifi_daemon: &state.caps.wifi_daemon,
                        vpn_cols: ui.vpn_cols,
                    };
                    for action in ui.handle_event(event, &context, &state.caps) {
                        handle_app_action(action, &mut state, &mut ui, &sock_tx).await;
                    }
                }
            }
        }

        Ok(())
    }
}

fn lost_connection() -> ManndError {
    ManndError::ConnectionFailed("Lost connection to the mannd daemon".into())
}

async fn handle_state_update(
    state: &mut AppState,
    ui: &mut UiState,
    msg: NetworkState,
) -> Option<StateCommand> {
    match msg {
        NetworkState::SetNetworks(aps) => {
            if let View::Wifi(wifi_state) = &mut ui.current_view {
                state.net_ctx.networks = aps;
                wifi_state.refresh_available_actions(&state.net_ctx.networks);
            }
        }
        NetworkState::SetWireguardInfo {
            names,
            meta,
            active,
        } => {
            state.net_ctx.wg_ctx.names = names;
            state.net_ctx.wg_ctx.meta = meta;
            state.net_ctx.wg_ctx.active = active;
        }
        NetworkState::SetWpaInterfaces(ifaces) => {
            state.net_ctx.wpa_interfaces = Some(ifaces);
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
        _ => None,
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
        }
        Success::Connected(network) => {
            ui.should_block = false;
            if let Some(found) = state
                .net_ctx
                .networks
                .iter_mut()
                .position(|net| net == &network)
            {
                let mut net = state.net_ctx.networks.remove(found);
                net.flags |= NetworkFlags::CONNECTED | NetworkFlags::KNOWN;
                state.net_ctx.networks.insert(0, net);
            }
        }
        Success::Disconnected => {
            if let Some(found) = state
                .net_ctx
                .networks
                .iter_mut()
                .find(|net| net.flags.contains(NetworkFlags::CONNECTED))
            {
                found.flags &= !NetworkFlags::CONNECTED;
            }
        }
        Success::ForgotNetwork(network) => {
            if let Some(found) = state
                .net_ctx
                .networks
                .iter_mut()
                .find(|net| *net == &network)
            {
                found.flags &= !(NetworkFlags::CONNECTED | NetworkFlags::KNOWN);
            }
        }
    }

    if let View::Wifi(wifi_state) = &mut ui.current_view {
        wifi_state.refresh_available_actions(&state.net_ctx.networks);
    }

    return Some(StateCommand::ClearPrompts);
}

fn handle_failure(
    _state: &mut AppState,
    ui: &mut UiState,
    failed: Failure,
) -> Option<StateCommand> {
    if !matches!(failed.process, Process::WifiScan) {
        ui.should_block = false;
    }
    Some(StateCommand::Prompt(PromptState::Info(InfoPrompt::new(
        failed.reason,
        PopupType::Error,
    ))))
}

async fn handle_app_action(
    action: AppAction,
    state: &mut AppState,
    ui: &mut UiState,
    net_tx: &Sender<Vec<u8>>,
) {
    match action {
        AppAction::Network(action) => {
            match to_stdvec_cobs(&action) {
                Ok(res) => {
                    let _ = net_tx.send(res).await;
                }
                Err(e) => tracing::error!("Failed to serialize network action: {e}"),
            }
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
    pub networks: Vec<NetworkInfo>,
    pub wpa_interfaces: Option<Vec<WpaInterface>>,
    pub wg_ctx: WireguardContext,
    pub netd_files: Vec<String>,
}

#[derive(Default)]
pub struct WireguardContext {
    pub names: Vec<String>,
    pub meta: Vec<WgMeta>,
    pub active: bool,
}

impl WireguardContext {
    pub fn len(&self) -> usize {
        min(self.names.len(), self.meta.len())
    }

    pub fn get_index(&self, index: usize) -> Option<(&str, &WgMeta)> {
        self.names
            .get(index)
            .zip(self.meta.get(index))
            .map(|(name, meta)| (name.as_str(), meta))
    }
}

impl Default for NetworkContext {
    fn default() -> Self {
        Self {
            networks: vec![],
            wpa_interfaces: None,
            wg_ctx: WireguardContext::default(),
            netd_files: vec![],
        }
    }
}
