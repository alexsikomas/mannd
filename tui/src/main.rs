use mannd::{
    GlobalStateGuard, UNIX_SOCK_PATH, context, error::ManndError, state::messages::NetworkAction,
    utils::setup_logging,
};
use postcard::to_stdvec_cobs;
use std::{path::PathBuf, process::Stdio, str::FromStr, time::Duration};
use tokio::{fs, io::AsyncWriteExt, net::UnixStream, process::Command, time::timeout};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::{Level, instrument};
use tui::{app::App, ui::Theme};

const DEBUG_SOCK_BIN: &str = "target/debug/socket";
const RELEASE_SOCK_BIN: &str = "/usr/libexec/mannd-socket";

#[tokio::main]
#[instrument(err)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uid = unsafe { libc::geteuid() };
    let stream = get_unix_socket(uid).await?;

    GlobalStateGuard::init(Some(uid))?;
    let settings = &context().settings;

    let max_log_level = Level::from_str(&settings.get("debug", "max_log_level")?)?;
    let mut tui_log = PathBuf::from(settings.get("storage", "state")?);
    tui_log.push("mannd/logs/tui.log");

    setup_logging(tui_log, max_log_level, Some(uid))?;

    let _ = Theme::new();
    let () = App::new(stream).run().await?;
    Ok(())
}

#[instrument(err)]
/// Attempts to get the backend UNIX socket, first tries to connect
/// if no connection requests password to start either debug or
/// release service.
async fn get_unix_socket(uid: u32) -> Result<UnixStream, ManndError> {
    if let Ok(stream) = UnixStream::connect(UNIX_SOCK_PATH).await {
        // stale backend may still be bound to socket path deadlocked
        if is_backend_healthy(stream).await {
            if let Ok(s) = UnixStream::connect(UNIX_SOCK_PATH).await {
                return Ok(s);
            }
        } else {
            kill_spawned_socket_proc().await;
            wait_for_socket_death().await;
        }
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
        .args(["-S", bin_path, &format!("--target-uid={uid}"), "--spawned"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(format!("{password}\n").as_bytes()).await?;
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

async fn is_backend_healthy(mut stream: UnixStream) -> bool {
    let (r, w) = stream.split();
    let mut writer = FramedWrite::new(w, LengthDelimitedCodec::new());
    let mut reader = FramedRead::new(r, LengthDelimitedCodec::new());

    let Ok(probe) = to_stdvec_cobs(&NetworkAction::GetCapabilities) else {
        return false;
    };

    if futures::SinkExt::send(&mut writer, probe.into())
        .await
        .is_err()
    {
        return false;
    }

    timeout(
        Duration::from_secs(1),
        futures::StreamExt::next(&mut reader),
    )
    .await
    .is_ok_and(|opt| opt.is_some_and(|r| r.is_ok()))
}

async fn kill_spawned_socket_proc() {
    let Ok(mut entries) = fs::read_dir("/proc").await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name = name.to_string_lossy();

        // numeric = pid
        if !name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let cmd_path = format!("/proc/{name}/cmdline");
        let Ok(cmdline) = fs::read(&cmd_path).await else {
            continue;
        };
        let cmdline_str = String::from_utf8_lossy(&cmdline);
        if cmdline_str.contains("socket") && cmdline_str.contains("--spawned") {
            let pid: i32 = name.parse().unwrap_or(0);
            if pid > 0 {
                tracing::info!("Killing stale spawned socket, pid={pid}");
                unsafe { libc::kill(pid, libc::SIGTERM) };
            }
        }
    }
}

async fn wait_for_socket_death() {
    // 2s
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if !std::path::Path::new(UNIX_SOCK_PATH).exists() {
            break;
        }
    }
}
