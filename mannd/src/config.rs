use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::read_to_string,
    path::{Path, PathBuf},
};

use crate::{context, error::ManndError};

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub storage: StorageConfig,
    pub debug: DebugConfig,
    pub theme: ThemeConfig,
    pub keybinds: HashMap<String, String>,
}

impl Default for AppConfig {
    // include settings.conf in binary instead of this?
    fn default() -> Self {
        let storage = StorageConfig {
            state: "${HOME}/.local/state".into(),
        };

        let debug = DebugConfig {
            max_log_level: "info".into(),
        };

        let mut palettes: HashMap<String, ThemePalette> = HashMap::new();
        palettes.insert("light".into(), ThemePalette::light());
        palettes.insert("dark".into(), ThemePalette::dark());
        let theme = ThemeConfig {
            selected: "dark".into(),
            palettes,
        };

        let mut keybinds: HashMap<String, String> = HashMap::new();
        keybinds.insert("<Up>".into(), "up".into());
        keybinds.insert("<Down>".into(), "down".into());
        keybinds.insert("<Left>".into(), "left".into());
        keybinds.insert("<Right>".into(), "right".into());
        keybinds.insert("<CR>".into(), "enter".into());
        keybinds.insert("<Esc>".into(), "esc".into());
        keybinds.insert("<BS>".into(), "backscape".into());

        return AppConfig {
            storage,
            debug,
            theme,
            keybinds,
        };
    }
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct DebugConfig {
    pub max_log_level: String,
}

#[derive(Debug, Deserialize)]
pub struct ThemeConfig {
    pub selected: String,
    pub palettes: HashMap<String, ThemePalette>,
}

#[derive(Debug, Deserialize)]
pub struct ThemePalette {
    pub background: String,
    pub foreground: String,
    pub muted: String,
    pub error: String,
    pub warning: String,
    pub success: String,
    pub info: String,
    pub primary: String,
    pub secondary: String,
    pub tertiary: String,
    pub accent: String,
}

impl ThemePalette {
    fn light() -> Self {
        Self {
            background: "#fbf1c7".into(),
            foreground: "#3c3836".into(),
            muted: "#a89984".into(),
            error: "#9d0006".into(),
            warning: "#b57614".into(),
            success: "#79740e".into(),
            info: "#076678".into(),
            primary: "#458588".into(),
            secondary: "#689d6a".into(),
            tertiary: "#8f3f71".into(),
            accent: "#d79921".into(),
        }
    }

    fn dark() -> Self {
        Self {
            background: "#282828".into(),
            foreground: "#ebdbb2".into(),
            muted: "#928374".into(),
            error: "#cc241d".into(),
            warning: "#d79921".into(),
            success: "#98971a".into(),
            info: "#458588".into(),
            primary: "#83a598".into(),
            secondary: "#8ec07c".into(),
            tertiary: "#d3869b".into(),
            accent: "#fabd2f".into(),
        }
    }
}

impl AppConfig {
    pub fn load(path: PathBuf, home: Option<&Path>) -> Result<Self, ManndError> {
        let conf_str = read_to_string(&path)?;

        match ron::from_str::<AppConfig>(&conf_str) {
            Ok(mut conf) => {
                conf.expand_vars(home)?;
                conf.storage.state.push_str("/mannd");
                Ok(conf)
            }
            Err(_) => {
                let mut conf = AppConfig::default();
                conf.expand_vars(home)?;
                conf.storage.state.push_str("/mannd");
                Ok(conf)
            }
        }
    }

    /// Currently only ${HOME} is expanded possibly more if the future so
    /// function name is more generic.
    ///
    /// Global variables may not be defined therefore home can be passed in
    /// otherwise pass None to use globals
    fn expand_vars(&mut self, home: Option<&Path>) -> Result<(), ManndError> {
        let var_name = self
            .storage
            .state
            .split_once("${")
            .and_then(|(_, rest)| rest.split_once("}"))
            .map(|(content, _)| content);

        if let Some(var) = var_name {
            if var.to_uppercase() != "HOME" {
                return Err(ManndError::InvalidPropertyFormat(format!(
                    "Unkown variable: £{{{var}}}"
                )));
            }

            let home_path = match home {
                Some(h) => h,
                None => &context().home,
            };

            let home_str = home_path.to_str().ok_or_else(|| {
                ManndError::OperationFailed("HOME path is not valid UTF-8".into())
            })?;

            self.storage.state = self.storage.state.replace(&format!("${{{var}}}"), home_str);
        }

        Ok(())
    }
}
