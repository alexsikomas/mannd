use std::fs::OpenOptions;

use color_eyre::Result;
use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame, prelude::Backend};
use serde::Deserialize;
use tokio::io;
use toml::Value;
use tracing::{Level, instrument::WithSubscriber};
use tracing_error::ErrorLayer;
use tracing_subscriber::{FmtSubscriber, Registry, layer::SubscriberExt};
use tui::ui::{UiState, ui};

fn main() -> Result<()> {
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

    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default subscriber failed!");

    if let Ok(ui_state) = UiState::new() {
        let result = run(terminal, &ui_state);
        ratatui::restore();
        result
    } else {
        ratatui::restore();
        let ui_k = UiState::new().err();
        println!("{:?}", ui_k);
        Ok(())
    }
}

fn run(mut terminal: DefaultTerminal, ui_state: &UiState) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, ui_state))?;
        if matches!(event::read()?, Event::Key(_)) {
            break Ok(());
        }
    }
}
