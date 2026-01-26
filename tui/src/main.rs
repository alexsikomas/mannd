use com::{error::ManndError, utils::setup_logging, UNIX_SOCK_PATH};
use std::{env, fs::OpenOptions, path::PathBuf, process::Stdio};
use tokio::{
    io::AsyncWriteExt,
    net::{UnixSocket, UnixStream},
    process::{Child, Command},
    time,
};
use tracing::Level;
use tracing_error::ErrorLayer;
use tracing_subscriber::{layer::SubscriberExt, FmtSubscriber};
use tui::{app::App, ui::Theme};

const DEBUG_SOCK_BIN: &str = "target/debug/socket";

#[tokio::main]
async fn main() -> Result<(), ManndError> {
    let (stream, child) = get_unix_socket().await?;
    setup_logging("./.logs/tui.log");

    let _ = Theme::new();
    let _ = App::new(stream, child).run().await?;
    ratatui::restore();
    Ok(())
}

async fn get_unix_socket() -> Result<(UnixStream, Option<Child>), ManndError> {
    println!("Privileged access required for the socket service.");
    let password = rpassword::prompt_password("Enter sudo password: ")?;

    match cfg!(debug_assertions) {
        true => {
            let mut child = Command::new("sudo")
                .arg("-S")
                .arg(PathBuf::from(DEBUG_SOCK_BIN))
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

            let mut interval = time::interval(time::Duration::from_millis(100));
            let mut attempts = 0;
            let max_attempts = 20; // 2s

            loop {
                interval.tick().await;
                let socket = UnixSocket::new_stream()?;
                let connect = socket.connect(UNIX_SOCK_PATH).await;
                match connect {
                    Ok(s) => {
                        return Ok((s, Some(child)));
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
        false => {
            let socket = UnixSocket::new_stream()?;
            return Ok((socket.connect(UNIX_SOCK_PATH).await?, None));
        }
    };
}
