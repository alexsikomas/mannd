use futures::{SinkExt, StreamExt};
use mannd::{
    UNIX_SOCK_PATH, context,
    error::ManndError,
    init_ctx,
    state::messages::{Capability, NetworkAction, NetworkState},
    utils::setup_logging,
};
use postcard::{from_bytes_cobs, to_stdvec_cobs};
use std::{
    error::Error,
    io::ErrorKind,
    path::PathBuf,
    process::{ExitCode, Stdio},
    str::FromStr,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    process::{Child, Command},
    time::{sleep, timeout},
};
use tokio_util::{
    bytes::Bytes,
    codec::{Framed, LengthDelimitedCodec},
};
use tracing::Level;
use tui::{
    SETTINGS,
    app::{App, Backend},
    ui::Theme,
};

/// Where `run.sh -i` puts the daemon
const INSTALLED_DAEMON_BIN: &str = "/usr/local/libexec/mannd-socket";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const SPAWN_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// 5s
const SPAWN_POLL_TRIES: u32 = 25;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            eprintln!("mannd: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let uid = unsafe { libc::geteuid() };
    let settings = init_ctx(uid)?;

    // create log dir before backend can, preventing them from being owned by root
    let max_log_level = Level::from_str(&settings.debug.max_log_level)?;
    setup_logging(context().storage.join("logs/tui.log"), max_log_level, uid)?;

    SETTINGS
        .set(settings)
        .expect("SETTINGS already initialised");
    Theme::new()?;

    let (backend, caps) = connect_backend(uid).await?;
    App::new(backend, caps).run().await?;
    Ok(())
}

/// Connects to the running daemon, or if there is none starts one for the
/// lifetime of this frontend. A daemon that is there but can't be used is
/// reported and left alone.
async fn connect_backend(uid: u32) -> Result<(Backend, Capability), ManndError> {
    match UnixStream::connect(UNIX_SOCK_PATH).await {
        Ok(stream) => return handshake(stream).await,
        // no daemon, or only the socket one left behind
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) => {}
        Err(e) => {
            return Err(ManndError::ConnectionFailed(format!(
                "{UNIX_SOCK_PATH}: {e}"
            )));
        }
    }

    let mut child = spawn_daemon(uid).await?;
    for _ in 0..SPAWN_POLL_TRIES {
        sleep(SPAWN_POLL_INTERVAL).await;

        if let Some(status) = child.try_wait()? {
            let mut reason = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut reason).await;
            }
            return Err(ManndError::ConnectionFailed(format!(
                "could not start the daemon ({status}). {}",
                reason.trim()
            )));
        }

        if let Ok(stream) = UnixStream::connect(UNIX_SOCK_PATH).await {
            return handshake(stream).await;
        }
    }

    // sudo runs as root and is not ours to kill, the daemon gives up on its
    // own when nobody has connected
    Err(ManndError::ConnectionFailed(
        "timed out waiting for the daemon to start".into(),
    ))
}

/// Asks for the sudo password and starts a daemon that exits with us
async fn spawn_daemon(uid: u32) -> Result<Child, ManndError> {
    println!("Privileged access required for the socket service.");
    let password =
        tokio::task::spawn_blocking(|| rpassword::prompt_password("Enter sudo password: "))
            .await
            .map_err(|_| ManndError::OperationFailed("Task joined failed".to_string()))??;

    let mut child = Command::new("sudo")
        // no prompt, so that stderr only ever holds a reason for failing
        .args(["-S", "-p", ""])
        .arg(daemon_bin())
        .args([&format!("--target-uid={uid}"), "--spawned"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    // dropped afterwards, a wrong password must not leave sudo waiting
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(format!("{password}\n").as_bytes()).await?;
        stdin.flush().await?;
    }
    Ok(child)
}

/// The daemon built alongside this binary if there is one, which is the
/// case when running out of `target/`, otherwise the installed one
fn daemon_bin() -> PathBuf {
    std::env::current_exe()
        .ok()
        .map(|exe| exe.with_file_name("socket"))
        .filter(|sibling| sibling.is_file())
        .unwrap_or_else(|| PathBuf::from(INSTALLED_DAEMON_BIN))
}

/// Requests the capabilities of the system to know what to display on
/// screen for example: Wi-Fi, wireguard, networkd. Doubles as the check
/// that the daemon is alive and free for current client
async fn handshake(stream: UnixStream) -> Result<(Backend, Capability), ManndError> {
    let mut backend = Framed::new(stream, LengthDelimitedCodec::new());
    let request = to_stdvec_cobs(&NetworkAction::GetCapabilities)?;
    let _ = backend.send(Bytes::from(request)).await;

    let reply = timeout(HANDSHAKE_TIMEOUT, async {
        while let Some(frame) = backend.next().await {
            match from_bytes_cobs::<NetworkState>(&mut frame?)? {
                NetworkState::SetCapabilities(caps) => return Ok(caps),
                NetworkState::Busy => {
                    return Err(ManndError::ConnectionFailed(
                        "another mannd TUI is already connected".into(),
                    ));
                }
                _ => {}
            }
        }
        Err(ManndError::ConnectionFailed(
            "the daemon closed the connection".into(),
        ))
    })
    .await;

    match reply {
        Ok(Ok(caps)) => Ok((backend, caps)),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(ManndError::ConnectionFailed(
            "daemon not responding; restart it".into(),
        )),
    }
}
