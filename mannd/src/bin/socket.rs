//! # Socket
//!
//! Facilitates communication between the backend and the frontend.
//!
//! A UNIX socket architecture was chosen for this binary as it allows for
//! more decoupling between the frontend and the backend. It also allows the
//! user, if they desire, to run the socket on startup. As long as the frontend
//! handles this appropriately they aren't hassled with entering their sudo
//! password each time.
//!
//! One [`NetworkActor`] lives for as long as the daemon does and serves a
//! single client at a time, any other client is told the daemon is busy.

use clap::Parser;
use futures::{SinkExt, StreamExt};
use mannd::{
    GlobalStateGuard, UNIX_SOCK_PATH, context,
    error::ManndError,
    init_ctx,
    state::{
        actor::NetworkActor,
        messages::{Failure, NetworkAction, NetworkState, Process},
    },
    utils::setup_logging,
};
use postcard::to_stdvec_cobs;
use std::{
    error::Error,
    fs::{self, Permissions},
    future::pending,
    io::{self, Write},
    os::unix::fs::{PermissionsExt, chown},
    path::PathBuf,
    process::ExitCode,
    str::FromStr,
    time::Duration,
};
use tokio::{
    net::{UnixListener, UnixStream},
    signal::unix::{SignalKind, signal},
};
use tokio_util::{
    bytes::{Bytes, BytesMut},
    codec::{Framed, LengthDelimitedCodec},
};
use tracing::{Level, info};

const SPAWN_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
type Client = Framed<UnixStream, LengthDelimitedCodec>;

#[derive(Parser, Debug)]
#[command(version, about = "mannd socket")]
struct Args {
    #[arg(long)]
    target_uid: u32,
    #[arg(long)]
    spawned: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            //stderr may be gone if frontend died
            let _ = writeln!(io::stderr(), "mannd-socket: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    if unsafe { libc::geteuid() } != 0 {
        Err(ManndError::NotRoot)?;
    }

    let args = Args::parse();
    let config = init_ctx(args.target_uid)?;
    let max_log_level = Level::from_str(&config.debug.max_log_level)?;
    let socket_log = context().storage.join("logs/socket.log");
    setup_logging(socket_log, max_log_level, args.target_uid)?;

    let socket = UnixSocketGuard::new(UNIX_SOCK_PATH).await?;
    let _state = GlobalStateGuard::init()?;
    let mut actor = NetworkActor::new().await?;

    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    // non reseting timeout
    let connect_deadline = tokio::time::sleep(SPAWN_CONNECT_TIMEOUT);
    tokio::pin!(connect_deadline);

    let mut client: Option<Client> = None;
    let mut served = false;

    let res = loop {
        // client disconnect whilst life tied to it
        if args.spawned && served && client.is_none() {
            info!("Spawned backend exiting after client disconnect");
            break Ok(());
        }

        tokio::select! {
            accepted = socket.listener.accept() => {
                let stream = match accepted {
                    Ok((stream, _)) => stream,
                    Err(e) => break Err(e),
                };
                let mut new_client = Framed::new(stream, LengthDelimitedCodec::new());

                if client.is_some() {
                    info!("Turning a second client away");
                    let _ = send_state(&mut new_client, &NetworkState::Busy).await;
                    continue;
                }

                // may have launched before iwd/wpa, so check again
                if actor.controller.wifi.is_none() {
                    actor.controller.connect_wifi_adapter().await;
                }
                client = Some(new_client);
            }
            frame = next_frame(&mut client) => {
                let keep = match (decode_action(frame), client.as_mut()) {
                    (Some(action), Some(client)) => {
                        served = true;
                        dispatch(&mut actor, client, action).await
                    }
                    _ => false,
                };
                if !keep {
                    client = None;
                }
            }
            Some(msg) = actor.signal_manager.recv() => {
                if let Some(action) = actor.handle_signal(msg)
                    && let Some(current) = client.as_mut()
                    && !dispatch(&mut actor, current, action).await
                {
                    client = None;
                }
            }
            _ = sigint.recv() => break Ok(()),
            _ = sigterm.recv() => break Ok(()),
            // branch guard so others can win
            () = &mut connect_deadline, if args.spawned && !served && client.is_none() => {
                    info!("Timed out waiting for the frontend to connect, exiting.");
                    break Ok(());
                }
        }
    };

    info!("Shutting down...");
    drop(client);
    if let Err(e) = actor.controller.exit().await {
        tracing::warn!("Cleanup before exit failed: {e}");
    }

    Ok(res?)
}

/// The next frame of the client, never resolving while there is none
async fn next_frame(client: &mut Option<Client>) -> Option<io::Result<BytesMut>> {
    match client {
        Some(client) => client.next().await,
        None => pending().await,
    }
}

/// `None` means the client should be dropped (disconnect/unreadable)
fn decode_action(frame: Option<io::Result<BytesMut>>) -> Option<NetworkAction> {
    let mut frame = match frame {
        Some(Ok(frame)) => frame,
        Some(Err(e)) => {
            tracing::warn!("Could not read from the client, dropping it. {e}");
            return None;
        }
        None => {
            info!("Frontend has disconnected.");
            return None;
        }
    };

    postcard::from_bytes_cobs(&mut frame)
        .inspect_err(|e| tracing::warn!("Could not decode a client frame, dropping it. {e}"))
        .ok()
}

/// Runs `action` and reports the outcome, returning false once the client
/// can no longer be written to
async fn dispatch(actor: &mut NetworkActor, client: &mut Client, action: NetworkAction) -> bool {
    let states = actor.handle_action(action).await.unwrap_or_else(|e| {
        vec![NetworkState::Failed(Failure::new(
            Process::Generic,
            e.to_string(),
        ))]
    });

    for state in &states {
        if let Err(e) = send_state(client, state).await {
            info!("Could not write to the client, dropping it. {e}");
            return false;
        }
    }
    true
}

async fn send_state(client: &mut Client, state: &NetworkState) -> io::Result<()> {
    match to_stdvec_cobs(state) {
        Ok(bytes) => client.send(Bytes::from(bytes)).await,
        Err(e) => {
            tracing::error!("Could not serialise {state:?}. {e}");
            Ok(())
        }
    }
}

struct UnixSocketGuard {
    path: PathBuf,
    listener: UnixListener,
}

impl UnixSocketGuard {
    /// Binds `path` for the target user, gives up if
    /// client already running
    async fn new(path: &str) -> Result<Self, ManndError> {
        if UnixStream::connect(path).await.is_ok() {
            return Err(ManndError::AlreadyRunning);
        }

        // nobody is listening, anything left here is stale
        let _ = fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        chown(path, Some(context().uid), None)?;
        fs::set_permissions(path, Permissions::from_mode(0o600))?;

        Ok(Self {
            path: PathBuf::from(path),
            listener,
        })
    }
}

impl Drop for UnixSocketGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
