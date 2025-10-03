use std::error::Error;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use tracing::info;

pub fn kbd_events(key: Event) -> Action {
    match key {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Action::Quit,
            // since last index rendered last down is increment
            KeyCode::Down => Action::Increment,
            KeyCode::Up => Action::Decrement,
            _ => Action::NoOp,
        },
        _ => Action::NoOp,
    }
}

pub enum Action {
    // used to go up/down selection indexes
    Increment,
    Decrement,
    Quit,
    NoOp,
}
