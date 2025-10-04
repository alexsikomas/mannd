use std::{fs::OpenOptions, time::Duration};

use color_eyre::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
};
use tokio::sync::{
    mpsc::{self, Receiver, UnboundedSender},
    oneshot,
};
use tracing::{Level, info, instrument::WithSubscriber};
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

    /*
     * I'd prefer to use a oneshot here but it causes a problem
     * with the borrow checker so we have two mpsc channels instead.
     * first is for quitting second is for events to handle
     */
    let (q_tx, mut q_rx) = mpsc::channel::<()>(1);
    let (tx, mut rx) = mpsc::unbounded_channel::<AppMessage>();

    tokio::spawn(state.handle(rx, q_tx));
    let result = run(tx, q_rx, terminal).await;
    ratatui::restore();
    result
}

async fn run(
    tx: UnboundedSender<AppMessage>,
    mut q_rx: Receiver<()>,
    mut terminal: DefaultTerminal,
) -> Result<()> {
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

            // outside quit events
            match q_rx.try_recv() {
                Ok(()) => {
                    break Ok(());
                }
                _ => {}
            }
        }
    }
}
