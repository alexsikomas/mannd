use com::{
    SETTINGS, UNIX_SOCK_PATH, error::ManndError, geteuid, init_home_path, utils::setup_logging,
};
use std::{path::PathBuf, process::Stdio, str::FromStr};
use tokio::{
    io::AsyncWriteExt,
    net::{UnixSocket, UnixStream},
    process::{Child, Command},
    time,
};
use tracing::Level;
use tui::{app::App, ui::Theme};

const DEBUG_SOCK_BIN: &str = "target/debug/socket";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stream = get_unix_socket().await?;

    let max_log_level = Level::from_str(&SETTINGS.get("debug", "max_log_level")?)?;
    let mut tui_log = PathBuf::from(SETTINGS.get("storage", "state")?.clone());
    tui_log.push("mannd/logs/tui.log");

    setup_logging(tui_log, max_log_level);

    let _ = Theme::new();
    let _ = App::new(stream).run().await?;
    Ok(())
}

async fn get_unix_socket() -> Result<UnixStream, ManndError> {
    let uid = unsafe { geteuid() };
    init_home_path(Some(uid));

    let socket = UnixSocket::new_stream()?;

    // check if socket already live
    match socket.connect(UNIX_SOCK_PATH).await {
        Ok(stream) => return Ok(stream),
        Err(_) => {
            println!("Privileged access required for the socket service.");
            let password = rpassword::prompt_password("Enter sudo password: ")?;

            match cfg!(debug_assertions) {
                true => {
                    // TODO: arg for being tied to socket
                    // so only then deletes itself not if
                    // matched on Ok() cond
                    let mut child = Command::new("sudo")
                        .arg("-S")
                        .arg(PathBuf::from(DEBUG_SOCK_BIN))
                        .arg(format!("--parent-uid={}", uid))
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

                    return try_connect_socket().await;
                }
                false => {
                    let socket = UnixSocket::new_stream()?;
                    return Ok(socket.connect(UNIX_SOCK_PATH).await?);
                }
            };
        }
    };
}

async fn try_connect_socket() -> Result<UnixStream, ManndError> {
    let mut interval = time::interval(time::Duration::from_millis(100));
    let mut attempts = 0;
    let max_attempts = 20; // 2s

    loop {
        interval.tick().await;
        let socket = UnixSocket::new_stream()?;
        let connect = socket.connect(UNIX_SOCK_PATH).await;
        match connect {
            Ok(s) => {
                return Ok(s);
            }
            _ => {
                attempts += 1;
                if attempts >= max_attempts {
                    panic!("Fatal Error: Cannot connect to socket.");
                }
                continue;
            }
        };
    }
}
