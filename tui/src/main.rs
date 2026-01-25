use com::{error::ManndError, UNIX_SOCK_PATH};
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
    println!("Privileged access required for the socket service.");
    let password = rpassword::prompt_password("Enter sudo password: ")?;

    let (stream, child): (UnixStream, Option<Child>) = match cfg!(debug_assertions) {
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

            let mut interval = time::interval(time::Duration::from_secs(1));
            let mut attempts = 0;

            loop {
                interval.tick().await;
                let socket = UnixSocket::new_stream()?;
                let connect = socket.connect(UNIX_SOCK_PATH).await;
                match connect {
                    Ok(s) => {
                        break (s, Some(child));
                    }
                    _ => {
                        attempts += 1;
                        if attempts >= 5 {
                            panic!("Fatal Error: Cannot connect to socket.");
                        }
                        continue;
                    }
                };
            }
        }
        false => {
            let socket = UnixSocket::new_stream()?;
            (socket.connect(UNIX_SOCK_PATH).await?, None)
        }
    };

    let subscriber = FmtSubscriber::builder()
        .compact()
        .with_file(true)
        .with_writer(
            OpenOptions::new()
                .append(true)
                .create(true)
                .open("./.logs/tui.log")
                .unwrap(),
        )
        .with_ansi(true)
        .with_line_number(true)
        .with_max_level(Level::INFO)
        .finish();

    let subscriber = subscriber.with(ErrorLayer::default());

    match tracing::subscriber::set_global_default(subscriber) {
        Err(e) => {
            tracing::error!(
                "{e}\nCould not set the default subscriber! Continuing without logging."
            )
        }
        _ => {}
    }

    let _ = Theme::new();
    let _ = App::new(stream, child).run().await?;
    ratatui::restore();
    Ok(())
}
