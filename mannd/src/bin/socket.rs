//! # Socket
//!
//! Facilitates communication between the backend and the frontend.
//!
//! A UNIX socket architecture was chosen for this binary as it allows for
//! more decoupling between the frontend and the backend. It also allows the
//! user, if they desire, to run the socket on startup. As long as the frontend
//! handles this appropriately they aren't hassled with entering their sudo
//! password each time.
use std::{
    error::Error,
    fs::{self, Permissions},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use futures::{SinkExt, StreamExt};
use mannd::{
    SETTINGS, UNIX_SOCK_PATH,
    controller::WifiDaemonType,
    error::ManndError,
    geteuid, init_home_path,
    state::{
        network::{NetworkAction, NetworkActor, NetworkState},
        signals::SignalUpdate,
    },
    utils::setup_logging,
};
use postcard::to_stdvec_cobs;
use tokio::{net::UnixListener, sync::mpsc};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::{Level, info, instrument};

#[derive(Parser, Debug)]
#[command(version, about = "mannd socket")]
struct Args {
    /// Determines where the $HOME directory is
    #[arg(long)]
    target_uid: Option<u32>,
}

#[tokio::main]
#[instrument(err)]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    init_home_path(args.target_uid);

    // root check
    let euid = unsafe { geteuid() };
    if euid != 0 {
        return Err(ManndError::NotRoot)?;
    }

    let max_log_level = Level::from_str(&SETTINGS.get("debug", "max_log_level")?)?;
    let mut socket_log = PathBuf::from(SETTINGS.get("storage", "state")?.clone());
    socket_log.push("mannd/logs/socket.log");
    setup_logging(socket_log, max_log_level, args.target_uid)?;

    let guard = UnixSocketGuard::new(UNIX_SOCK_PATH).await?;
    let (mut sock, _) = guard.listener.accept().await?;
    let (sock_reader, sock_writer) = sock.split();

    let (sock_tx, mut sock_rx) = mpsc::channel::<NetworkState>(32);
    let (signal_tx, mut signal_rx) = mpsc::channel::<SignalUpdate>(32);

    let mut actor = NetworkActor::new(signal_tx, sock_tx).await?;
    let mut writer = FramedWrite::new(sock_writer, LengthDelimitedCodec::new());
    let mut reader = FramedRead::new(sock_reader, LengthDelimitedCodec::new());

    let daemon = actor.controller.get_wifi_daemon_type();

    loop {
        tokio::select! {
            Some(msg) = sock_rx.recv() => {
                if let Ok(res) = to_stdvec_cobs(&msg) {
                    if writer.send(res.into()).await.is_err() {
                        info!("Could not write to socket, disconnecting");
                        return Ok(());
                    }
                }
            },
            frame_opt = reader.next() => {
                let Some(frame_res) = frame_opt else {
                    return Ok(());
                };

                let mut frame = frame_res?;
                let action = postcard::from_bytes_cobs::<NetworkAction>(&mut frame)?;
                let res = actor.handle_action(action).await?;
                if res == true {
                    return Ok(());
                }
            }
            Some(update) = signal_rx.recv() => {
                actor.signal_manager.handle_update(update);
            }
            Some(msg) = actor.signal_manager.recv() => {
                let action: Option<NetworkAction> = match daemon {
                    Some(WifiDaemonType::Iwd) => {
                        actor.signal_manager.process_iwd_msg(msg).await
                    }
                    Some(WifiDaemonType::Wpa) => {
                        actor.signal_manager.process_wpa_msg(msg).await
                    }
                    _ => {
                        None
                    }
                };

                if let Some(act) = action {
                    actor.handle_action(act).await?;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting down...");
                return Ok(());
            }
        };
    }
}

struct UnixSocketGuard {
    path: PathBuf,
    listener: UnixListener,
}

impl UnixSocketGuard {
    async fn new<P: AsRef<Path>>(path: P) -> tokio::io::Result<Self> {
        let path = path.as_ref().to_owned();

        let _ = tokio::fs::remove_file(&path).await;

        let listener = UnixListener::bind(&path)?;
        let perms = Permissions::from_mode(0o777);
        tokio::fs::set_permissions(&path, perms).await?;
        Ok(Self { path, listener })
    }
}

impl Drop for UnixSocketGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
