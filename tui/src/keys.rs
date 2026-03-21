use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mannd::{SETTINGS, error::ManndError};

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
        let conf = &SETTINGS;
        let mut bindings: HashMap<KeyEvent, KeyAction> = HashMap::default();

        if let Some(keybinds) = conf.sections.get("keybinds") {
            let keys = keybinds.keys();
            for key in keys {
                let event = key_str_to_event(key);
                let action: KeyAction = keybinds.get(key).unwrap().clone().into();
                bindings.insert(event?, action);
            }
        }
        Ok(Self { bindings })
    }
}

/// Follows <https://vimhelp.org/intro.txt.html#key-notation> if
/// there is a direct match to a keycode, keypad unimplemented
fn key_str_to_event(key: &str) -> Result<KeyEvent, ManndError> {
    // remove "<>"
    let key = &key[2..key.len() - 2];
    let mut modifier: KeyModifiers = KeyModifiers::NONE;

    // removes all the modifiers from keys
    let mut key_to_read: String = String::new();

    // modifiers
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

    let key_code: KeyCode;

    // then this is a char
    if key_to_read.len() == 1 {
        key_code = KeyCode::Char(key_to_read.chars().next().expect("Unexpected..."));
    } else {
        match key_to_read.as_str() {
            "Up" => key_code = KeyCode::Up,
            "Down" => key_code = KeyCode::Down,
            "Left" => key_code = KeyCode::Left,
            "Right" => key_code = KeyCode::Right,
            "Home" => key_code = KeyCode::Home,
            "Insert" => key_code = KeyCode::Insert,
            "End" => key_code = KeyCode::End,
            "PageUp" => key_code = KeyCode::PageUp,
            "PageDown" => key_code = KeyCode::PageDown,
            "Del" => key_code = KeyCode::Delete,
            "Tab" => key_code = KeyCode::Tab,
            "Return" | "CR" | "Enter" => key_code = KeyCode::Enter,
            "BS" => key_code = KeyCode::Backspace,
            "Esc" => key_code = KeyCode::Esc,
            "F1" => key_code = KeyCode::F(1),
            "F2" => key_code = KeyCode::F(2),
            "F3" => key_code = KeyCode::F(3),
            "F4" => key_code = KeyCode::F(4),
            "F5" => key_code = KeyCode::F(5),
            "F6" => key_code = KeyCode::F(6),
            "F7" => key_code = KeyCode::F(7),
            "F8" => key_code = KeyCode::F(8),
            "F9" => key_code = KeyCode::F(9),
            "F10" => key_code = KeyCode::F(10),
            "F11" => key_code = KeyCode::F(11),
            "F12" => key_code = KeyCode::F(12),
            _ => {
                tracing::error!("Key: {key_to_read} does not correspond to a valid keycode");
                return Err(ManndError::InputKey);
            }
        }
    }

    Ok(KeyEvent::new(key_code, modifier))
}
