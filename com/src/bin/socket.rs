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
        network::{handle_action, NetworkAction, NetworkActor},
        signals::SignalUpdate,
    },
    UNIX_SOCK_PATH,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    sync::mpsc,
};

struct UnixSocketGuard {
    path: PathBuf,
    listener: UnixListener,
}

#[link(name = "c")]
unsafe extern "C" {
    fn geteuid() -> u32;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let uid = unsafe { geteuid() };
    if uid != 0 {
        return Err(ManndError::NotRoot)?;
    }

    let guard = UnixSocketGuard::new(UNIX_SOCK_PATH).await?;
    let (mut sock, _) = guard.listener.accept().await?;
    let (mut reader, mut writer) = sock.split();

    let (sock_tx, mut sock_rx) = mpsc::channel::<Vec<u8>>(32);
    let (signal_tx, mut signal_rx) = mpsc::channel::<SignalUpdate>(32);

    let mut actor = NetworkActor::new().await?;
    let daemon = actor.controller.daemon_type();

    loop {
        let mut data = vec![0u8; 1024];
        tokio::select! {
            // write message for tui to read
            Some(msg) = sock_rx.recv() => {
                if let Err(e) = writer.write_all(&msg).await {
                    eprintln!("Failed to write to socket: {}", e);
                    return Err(ManndError::SocketWrite)?;
                }
            },
            Ok(_) = reader.readable() => {
                reader.read(&mut data).await?;
                let action = postcard::from_bytes_cobs::<NetworkAction>(&mut data)?;
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
                println!("Shutting down...");
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
