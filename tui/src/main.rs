use std::{fs::OpenOptions, time::Duration};

use color_eyre::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
};
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{Level, instrument::WithSubscriber};
use tracing_error::ErrorLayer;
use tracing_subscriber::{FmtSubscriber, Registry, layer::SubscriberExt};
use tui::{
    App, AppMessage,
    ui::{Theme, render},
};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();

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
                "Could not set the default subscriber! Continuing without proper logging"
            )
        }
        _ => {}
    }

    let _ = Theme::new();

    let mut state = App::default();

    let (tx, mut rx) = mpsc::unbounded_channel::<AppMessage>();
    tokio::spawn(state.handle(rx));
    let result = run(tx, terminal).await;
    ratatui::restore();
    result
}

async fn run(tx: UnboundedSender<AppMessage>, mut terminal: DefaultTerminal) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, tx.clone()))?;
        if let Ok(exp) = event::poll(Duration::from_millis(100)) {
            if exp {
                // this should exist might revist though
                let evt = event::read().unwrap();
                match evt {
                    Event::Key(key) => {
                        if key.code == KeyCode::Esc {
                            break Ok(());
                        }
                    }
                    _ => {}
                }

                tx.send(AppMessage::Event(evt));
            }
        }
    }
}
