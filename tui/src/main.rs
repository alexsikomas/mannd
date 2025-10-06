use std::{fs::OpenOptions, time::Duration};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode},
};
use tokio::sync::mpsc::{self, Receiver, UnboundedSender};
use tracing::Level;
use tracing_error::ErrorLayer;
use tracing_subscriber::{FmtSubscriber, layer::SubscriberExt};
use tui::{
    app::App,
    error::TuiError,
    ui::{Theme, render},
};

#[tokio::main]
async fn main() -> Result<(), TuiError> {
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
        .with_max_level(Level::TRACE)
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
