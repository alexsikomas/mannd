use futures::StreamExt;
use tracing::info;

use com::{
    controller::DaemonType,
    state::network::{NetworkAction, NetworkActor, NetworkState},
    wireless::common::AccessPoint,
};
use crossterm::event::EventStream;
use tokio::sync::mpsc::{self, Sender};

use crate::{
    error::TuiError,
    state::{AppContext, PromptState, UiState, View},
    ui::render,
};

pub struct App;

pub struct AppState {
    // only used in main loop
    should_quit: bool,
    redraw: bool,
    ui: UiState,

    // non-ui state,
    networks: Vec<AccessPoint>,
    daemon_type: Option<DaemonType>,
}

impl AppState {
    fn new() -> Self {
        AppState {
            should_quit: false,
            redraw: true,
            ui: UiState::new(),
            networks: vec![],
            daemon_type: None,
        }
    }
}

impl App {
    pub async fn run() -> Result<(), TuiError> {
        let mut state = AppState::new();

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
                info!("Draw Started");
                let context = AppContext::create(&state.networks, &state.daemon_type);
                terminal.draw(|f| render(f, &state.ui, &context))?;
                state.redraw = false;
                info!("Draw finished");
            }

            tokio::select! {
                Some(msg) = state_rx.recv() => {
                    if let Some(action) = handle_state_update(&mut state, msg).await {
                        let _ = action_tx.send(action).await;
                    }
                    state.redraw = true;
                }
                Some(Ok(event)) = events.next() => {
                    info!("Redraw enabled");
                    state.redraw = true;
                    let context = AppContext::create(&state.networks, &state.daemon_type);
                    if let Some(action) = state.ui.handle_event(event, &context) {
                        handle_app_action(action, &mut state, &action_tx).await;
                    }
                }
                else => break,
            }
        }

        let _ = action_tx.send(NetworkAction::Exit).await;
        Ok(())
    }
}

// Network thread may optionally ask us to perform a network action
async fn handle_state_update(state: &mut AppState, msg: NetworkState) -> Option<NetworkAction> {
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
        NetworkState::ConnectFailed(_reason) => {
            todo!()
        }
        NetworkState::CallAction(action) => {
            return Some(action);
        }
    };
    None
}

async fn handle_app_action(
    action: AppAction,
    state: &mut AppState,
    net_tx: &Sender<NetworkAction>,
) {
    match action {
        AppAction::Network(action) => {
            let _ = net_tx.send(action).await;
        }
        AppAction::AddPrompt(prompt) => {
            state.ui.prompt_stack.push(prompt);
        }
        AppAction::Exit => {
            state.should_quit = true;
        }
    }
}

pub enum AppAction {
    Network(NetworkAction),
    AddPrompt(PromptState),
    Exit,
}
