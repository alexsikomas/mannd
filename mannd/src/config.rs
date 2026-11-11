use serde::Deserialize;
use std::{collections::HashMap, fs, io::ErrorKind, os::unix::fs::chown, path::Path};

use crate::error::ManndError;

/// Location of the configuration directory relative to the home directory
pub const CONFIG_DIR: &str = ".config/mannd";
/// Location of default settings in mannd repo since we run from target/debug/tui
const DEFAULT_SETTINGS: &str = include_str!("../../etc/settings.conf");

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub storage: StorageConfig,
    pub debug: DebugConfig,
    pub theme: ThemeConfig,
    pub keybinds: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    pub path: String,
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
    /// Loads `settings.conf` from the configuration directory under `home`.
    ///
    /// A missing file is created from the defaults shipped in the binary and
    /// handed to `uid`. A malformed file is an error rather than a silent
    /// fallback, so a typo cannot quietly reset the theme and keybinds.
    pub fn load(home: &Path, uid: u32) -> Result<Self, ManndError> {
        let dir = home.join(CONFIG_DIR);
        let path = dir.join("settings.conf");

        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                fs::create_dir_all(&dir)?;
                fs::write(&path, DEFAULT_SETTINGS)?;
                chown(&dir, Some(uid), None)?;
                chown(&path, Some(uid), None)?;
                DEFAULT_SETTINGS.to_owned()
            }
            Err(e) => return Err(e.into()),
        };

        let mut conf: Self = ron::from_str(&raw)
            .map_err(|e| ManndError::InvalidPropertyFormat(format!("{}: {e}", path.display())))?;

        // only ${HOME} is expanded for now
        if let Some(rest) = conf.storage.path.strip_prefix("${HOME}") {
            let home = home.to_str().ok_or_else(|| {
                ManndError::OperationFailed("HOME path is not valid UTF-8".into())
            })?;
            conf.storage.path = format!("{home}{rest}");
        }
        conf.storage.path.push_str("/mannd");
        Ok(conf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uid() -> u32 {
        unsafe { libc::geteuid() }
    }

    #[test]
    fn missing_file_is_created_from_defaults() {
        let home = tempfile::tempdir().unwrap();

        let conf = AppConfig::load(home.path(), uid()).unwrap();

        let written = fs::read_to_string(home.path().join(CONFIG_DIR).join("settings.conf"));
        assert_eq!(written.unwrap(), DEFAULT_SETTINGS);
        assert_eq!(
            conf.storage.path,
            format!("{}/.local/state/mannd", home.path().display())
        );
    }

    #[test]
    fn malformed_file_is_an_error() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join(CONFIG_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.conf"), "(storage: (path: 3").unwrap();

        let err = AppConfig::load(home.path(), uid()).unwrap_err();

        assert!(matches!(err, ManndError::InvalidPropertyFormat(_)), "{err}");
        assert!(err.to_string().contains("settings.conf"), "{err}");
    }

    #[test]
    fn embedded_default_binds_backspace() {
        let conf: AppConfig = ron::from_str(DEFAULT_SETTINGS).unwrap();

        assert_eq!(conf.keybinds["<BS>"], "backspace");
        assert!(conf.theme.palettes.contains_key(&conf.theme.selected));
    }
}
