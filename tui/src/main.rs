use std::fs::OpenOptions;

use color_eyre::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
    prelude::Backend,
};
use serde::Deserialize;
use tokio::io;
use toml::Value;
use tracing::{Level, instrument::WithSubscriber};
use tracing_error::ErrorLayer;
use tracing_subscriber::{FmtSubscriber, Registry, layer::SubscriberExt};
use tui::{
    App,
    event::{Action, kbd_events},
    ui::{Theme, render},
};

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
    let result = run(&mut state, terminal);
    ratatui::restore();
    result
}

fn run(state: &mut App, mut terminal: DefaultTerminal) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, state))?;
        let key = event::read()?;
        match kbd_events(key) {
            Action::Increment => match state.views.selected {
                0 => state.main_menu.next(),
                _ => {}
            },
            Action::Decrement => match state.views.selected {
                0 => state.main_menu.prev(),
                _ => {}
            },
            Action::Quit => break Ok(()),
            _ => (),
        }
    }
}
