use std::{
    error::Error,
    fs::{self, Permissions},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use com::{
    controller::DaemonType,
    error::ManndError,
    state::{
        network::{handle_action, Capability, NetworkAction, NetworkActor, NetworkState},
        signals::SignalUpdate,
    },
    utils::{list_interfaces, setup_logging},
    UNIX_SOCK_PATH,
};
use futures::{SinkExt, StreamExt};
use postcard::to_stdvec_cobs;
use tokio::{net::UnixListener, sync::mpsc};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::info;

struct UnixSocketGuard {
    path: PathBuf,
    listener: UnixListener,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let uid = unsafe { geteuid() };
    if uid != 0 {
        return Err(ManndError::NotRoot)?;
    }
    setup_logging("./.logs/com.log");

    let guard = UnixSocketGuard::new(UNIX_SOCK_PATH).await?;
    let (mut sock, _) = guard.listener.accept().await?;
    let (sock_reader, sock_writer) = sock.split();

    let (sock_tx, mut sock_rx) = mpsc::channel::<NetworkState>(32);
    let (signal_tx, mut signal_rx) = mpsc::channel::<SignalUpdate>(32);

    let mut actor = NetworkActor::new().await?;
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
                let res = handle_action(&mut actor.controller, action, sock_tx.clone(), signal_tx.clone()).await?;
                if res == true {
                    return Ok(());
                }
            }
            Some(update) = signal_rx.recv() => {
                actor.signal_manager.handle_update(update);
            }
            Some(msg) = actor.signal_manager.recv() => {
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
                    handle_action(&mut actor.controller, act, sock_tx.clone(), signal_tx.clone()).await?;
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

#[link(name = "c")]
unsafe extern "C" {
    fn geteuid() -> u32;
}
