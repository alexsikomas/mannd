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

impl AppConfig {
    pub fn load(path: PathBuf, home: Option<&Path>) -> Result<Self, ManndError> {
        let conf_str = read_to_string(&path)?;
        let mut config: AppConfig = ron::from_str(&conf_str)
            .map_err(|e| ManndError::InvalidPropertyFormat(e.to_string()))?;
        config.expand_vars(home)?;

        config.storage.state.push_str("/mannd");

        Ok(config)
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
