use std::{env, fs::OpenOptions};

use tracing::Level;
use tracing_error::ErrorLayer;
use tracing_subscriber::{FmtSubscriber, layer::SubscriberExt};
use tui::{app::App, error::TuiError, ui::Theme};

#[tokio::main]
async fn main() -> Result<(), TuiError> {
    let args: Vec<String> = env::args().collect();

    let flag = handle_args(args);
    if !flag {
        return Ok(());
    }

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

    let result = App::run().await;
    ratatui::restore();
    result
}

/// Returns true if the program should continue, false otherwise
fn handle_args(args: Vec<String>) -> bool {
    if args.len() <= 1 {
        return true;
    }

    let mut i = 1;
    match args[i].as_str() {
        "-h" | "--help" => {
            println!("Mannd Help:");
            println!(
                "--------------------------------------------------------------------------------------------------"
            );
            println!("  -l, --log-level   [trace, info]     changes the maximum log level");
            println!("  -lf --log-file    <file_path>       changes where logs are written to");
            return false;
        }
        _ => {
            println!("Invalid argument");
            return false;
        }
    }
}
