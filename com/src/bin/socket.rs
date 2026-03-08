use std::{
    error::Error,
    fs::{self, Permissions},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use com::{
    SETTINGS, UNIX_SOCK_PATH,
    controller::DaemonType,
    error::ManndError,
    geteuid, init_home_path,
    state::{
        network::{NetworkAction, NetworkActor, NetworkState},
        signals::SignalUpdate,
    },
    utils::setup_logging,
};
use futures::{SinkExt, StreamExt};
use postcard::to_stdvec_cobs;
use tokio::{net::UnixListener, sync::mpsc};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::Level;

struct UnixSocketGuard {
    path: PathBuf,
    listener: UnixListener,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let uid = parse_args();
    init_home_path(uid);
    let uid = unsafe { geteuid() };
    if uid != 0 {
        return Err(ManndError::NotRoot)?;
    }

    let max_log_level = Level::from_str(&SETTINGS.get("debug", "max_log_level")?)?;
    let mut socket_log = PathBuf::from(SETTINGS.get("storage", "state")?.clone());
    socket_log.push("mannd/logs/socket.log");
    setup_logging(socket_log, max_log_level);

    let guard = UnixSocketGuard::new(UNIX_SOCK_PATH).await?;
    let (mut sock, _) = guard.listener.accept().await?;
    let (sock_reader, sock_writer) = sock.split();

    let (sock_tx, mut sock_rx) = mpsc::channel::<NetworkState>(32);
    let (signal_tx, mut signal_rx) = mpsc::channel::<SignalUpdate>(32);

    let mut actor = NetworkActor::new(signal_tx, sock_tx).await?;
    let mut writer = FramedWrite::new(sock_writer, LengthDelimitedCodec::new());
    let mut reader = FramedRead::new(sock_reader, LengthDelimitedCodec::new());

    let daemon = actor.controller.daemon_type();

    loop {
        tokio::select! {
            // write message for tui to read
            Some(msg) = sock_rx.recv() => {
                if let Ok(res) = to_stdvec_cobs(&msg) {
                    writer.send(res.into()).await.map_err(|_| ManndError::SocketWrite)?;
                }
            },
            Some(frame_res) = reader.next() => {
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
                // this is used, not sure why compiler disagrees
                #[allow(unused_assignments)]
                let mut action: Option<NetworkAction> = None;
                match daemon {
                    // iwd
                    Some(DaemonType::Iwd) => {
                        action = actor.signal_manager.process_iwd_msg(msg).await;
                    }
                    // wpa
                    Some(DaemonType::Wpa) => {
                        action = actor.signal_manager.process_wpa_msg(msg).await;
                    }
                    _ => {
                        continue;
                    }
                };

                if let Some(act) = action {
                    actor.handle_action(act).await?;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down...");
                return Ok(());
            }
        };
    }
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

fn parse_args() -> Option<u32> {
    let args: Vec<String> = std::env::args().collect();

    let parent_uid = args
        .iter()
        .find(|arg| arg.starts_with("--parent-uid="))
        .and_then(|arg| arg.split('=').nth(1))
        .and_then(|val| val.parse::<u32>().ok());
    return parent_uid;
}
