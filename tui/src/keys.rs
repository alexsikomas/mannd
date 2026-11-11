use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mannd::error::ManndError;

use crate::SETTINGS;

#[derive(Debug, PartialEq, Clone)]
pub enum KeyAction {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Backspace,
    Char(char),
    Paste(String),
    None,
}

impl From<String> for KeyAction {
    fn from(val: String) -> Self {
        let res = val.as_str();
        match res {
            "up" => KeyAction::Up,
            "down" => KeyAction::Down,
            "left" => KeyAction::Left,
            "right" => KeyAction::Right,
            "enter" => KeyAction::Enter,
            "bs" | "backspace" => KeyAction::Backspace,
            "esc" | "escape" => KeyAction::Escape,
            _ => KeyAction::None,
        }
    }
}

pub struct Keymap {
    pub bindings: HashMap<KeyEvent, KeyAction>,
}

impl Keymap {
    pub fn load_keys() -> Result<Self, ManndError> {
        let conf = SETTINGS.get().ok_or(ManndError::OperationFailed(
            "SETTINGS not initialised".into(),
        ))?;
        let mut bindings: HashMap<KeyEvent, KeyAction> = HashMap::default();

        let keys = conf.keybinds.keys();
        for key in keys {
            let event = key_str_to_event(key);
            let action: KeyAction = conf.keybinds.get(key).unwrap().clone().into();
            bindings.insert(event?, action);
        }
        Ok(Self { bindings })
    }
}

/// Follows <https://vimhelp.org/intro.txt.html#key-notation> if
/// there is a direct match to a keycode, keypad unimplemented
fn key_str_to_event(key: &str) -> Result<KeyEvent, ManndError> {
    let Some(key) = key.strip_prefix('<').and_then(|k| k.strip_suffix('>')) else {
        tracing::error!("Key: {key} is not in the <...> form");
        return Err(ManndError::InputKey);
    };
    let mut modifier: KeyModifiers = KeyModifiers::NONE;

    // removes all the modifiers from keys
    let mut key_to_read: String = String::new();
    for split in key.split('-') {
        match split {
            "S" => modifier.insert(KeyModifiers::SHIFT),
            "C" => modifier.insert(KeyModifiers::CONTROL),
            "M" => modifier.insert(KeyModifiers::META),
            "A" => modifier.insert(KeyModifiers::ALT),
            "D" => modifier.insert(KeyModifiers::SUPER),
            _ => {
                key_to_read = key_to_read + &String::from(split);
            }
        }
    }

    let key_code = if key_to_read.len() == 1 {
        // then this is a char
        KeyCode::Char(key_to_read.chars().next().expect("Unexpected..."))
    } else {
        match key_to_read.as_str() {
            "Up" => KeyCode::Up,
            "Down" => KeyCode::Down,
            "Left" => KeyCode::Left,
            "Right" => KeyCode::Right,
            "Home" => KeyCode::Home,
            "Insert" => KeyCode::Insert,
            "End" => KeyCode::End,
            "PageUp" => KeyCode::PageUp,
            "PageDown" => KeyCode::PageDown,
            "Del" => KeyCode::Delete,
            "Tab" => KeyCode::Tab,
            "Return" | "CR" | "Enter" => KeyCode::Enter,
            "BS" => KeyCode::Backspace,
            "Esc" => KeyCode::Esc,
            // catch F1..F12 keys
            k if k.starts_with('F') && k.len() > 1 && k[1..].chars().all(|c| c.is_ascii_digit()) => {
                let n: u8 = k[1..].parse().map_err(|_| ManndError::InputKey)?;
                KeyCode::F(n)
            }
            _ => {
                tracing::error!("Key: {key_to_read} does not correspond to a valid keycode");
                return Err(ManndError::InputKey);
            }
        }
    };

    Ok(KeyEvent::new(key_code, modifier))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_outside_angle_brackets_are_rejected() {
        for key in ["", "<", "a", "Up", "<Up", "é"] {
            assert!(
                matches!(key_str_to_event(key), Err(ManndError::InputKey)),
                "{key:?}"
            );
        }
    }

    #[test]
    fn bracketed_keys_are_parsed() {
        assert_eq!(
            key_str_to_event("<BS>").unwrap(),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)
        );
        assert_eq!(
            key_str_to_event("<C-q>").unwrap(),
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)
        );
    }
}
