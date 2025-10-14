use crossterm::event::{Event, KeyCode};
use tracing::info;

/// Handles input events, mutates the values in the state
pub fn event(event: Event) -> Action {
    info!("Event: {:?}", event);
    if let Event::Key(key) = event {
        match key.code {
            KeyCode::Up => Action::SelectUp,
            KeyCode::Down => Action::SelectDown,
            KeyCode::Right => Action::SelectRight,
            KeyCode::Left => Action::SelectLeft,
            KeyCode::Enter => Action::Enter,
            KeyCode::Esc => Action::Exit,
            _ => Action::NoOp,
        }
    } else {
        Action::NoOp
    }
}

pub enum Action {
    NoOp,
    Update,
    Event(Event),

    SelectUp,
    SelectDown,
    SelectLeft,
    SelectRight,
    Enter,

    Exit,
}
