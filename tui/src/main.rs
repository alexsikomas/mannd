use mannd::{
    SETTINGS, UNIX_SOCK_PATH, error::ManndError, geteuid, init_home_path, utils::setup_logging,
};
use std::{path::PathBuf, process::Stdio, str::FromStr};
use tokio::{io::AsyncWriteExt, net::UnixStream, process::Command};
use tracing::{Level, instrument};
use tui::{app::App, ui::Theme};

const DEBUG_SOCK_BIN: &str = "target/debug/socket";
const RELEASE_SOCK_BIN: &str = "/usr/libexec/mannd-socket";

#[tokio::main]
#[instrument(err)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uid = unsafe { geteuid() };
    let stream = get_unix_socket(uid).await?;

    let max_log_level = Level::from_str(&SETTINGS.get("debug", "max_log_level")?)?;
    let mut tui_log = PathBuf::from(SETTINGS.get("storage", "state")?.clone());
    tui_log.push("mannd/logs/tui.log");

    setup_logging(tui_log, max_log_level, Some(uid))?;

    let _ = Theme::new();
    let _ = App::new(stream).run().await?;
    Ok(())
}

#[instrument(err)]
/// Attempts to get the backend UNIX socket, first tries to connect
/// if no connection requests password to start either debug or
/// release service.
async fn get_unix_socket(uid: u32) -> Result<UnixStream, ManndError> {
    init_home_path(Some(uid));

    if let Ok(stream) = UnixStream::connect(UNIX_SOCK_PATH).await {
        return Ok(stream);
    }

    println!("Privileged access required for the socket service.");
    let password =
        tokio::task::spawn_blocking(|| rpassword::prompt_password("Enter sudo password: "))
            .await
            .map_err(|_| ManndError::OperationFailed("Task joined failed".to_string()))??;

    let bin_path = if cfg!(debug_assertions) {
        DEBUG_SOCK_BIN
    } else {
        RELEASE_SOCK_BIN
    };

    let mut child = Command::new("sudo")
        .args([
            "-S",
            bin_path,
            &format!("--target-uid={}", uid),
            "--spawned",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(format!("{}\n", password).as_bytes())
            .await?;
        stdin.flush().await?;
    }

    // 3s
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if let Ok(stream) = UnixStream::connect(UNIX_SOCK_PATH).await {
            return Ok(stream);
        }
    }

    Err(ManndError::ConnectionFailed(
        "Timed out waiting for socket...".to_string(),
    ))
}
