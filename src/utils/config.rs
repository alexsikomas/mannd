use std::{
    fs::{self, DirEntry},
    io,
    path::{self, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{ConfigMessage, PathOptions};

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    TomlSerialize(toml::ser::Error),
    TomlDeserialize(toml::de::Error),
    ConfigDirNotFound,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub wireguard: Wireguard,
    pub network: Network,
    pub config_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wireguard: Wireguard::default(),
            network: Network::default(),
            config_path: PathBuf::new(),
        }
    }
}

impl Config {
    /// Creates a new `Folders` instance
    ///
    /// Ensures that configuration directory and files exist
    pub fn new() -> Result<Self, ConfigError> {
        let mut config_path = dirs::config_dir().ok_or(ConfigError::ConfigDirNotFound)?;
        println!("1: {:?}", config_path);

        config_path.push("networkd-wireguard-manager");
        fs::create_dir_all(&config_path)?;
        println!("2: Created DIR");
        config_path.push("config.toml");
        // TODO: Make write_default_config function to simplify below
        let toml_file = fs::read_to_string(&config_path)?;
        println!("3: TOML READ");

        let mut config: Config = match toml::from_str(&toml_file) {
            Ok(c) => c,
            Err(_) => {
                let default_config = Self::default();
                let default_toml = toml::to_string(&default_config)?;
                fs::write(&config_path, default_toml)?;
                println!("4: FAILED TOML PARSE");
                default_config
            }
        };
        config.config_path = config_path;
        println!("4: PASSED TOML PARSE");
        if std::path::Path::exists(&config.network.path) {
            println!("5: PASSED ALL!");
            Ok(config)
        } else {
            println!("5: FAILED SYSTEMD NETWORK!");
            Err(ConfigError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot find systemd network folder!",
            )))
        }
    }

    fn update_config(&self) -> Result<(), ConfigError> {
        let content = toml::to_string(&self)?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    pub fn handle_message(&mut self, message: ConfigMessage) -> Result<(), ConfigError> {
        match message {
            ConfigMessage::UpdateWgPath(path, wg_opt) => match wg_opt {
                PathOptions::Add => {
                    self.wireguard.add_path(path);
                }
                PathOptions::Remove => {
                    self.wireguard.remove_path(&path);
                }
                PathOptions::RemoveAll => {
                    self.wireguard.remove_all_paths();
                }
            },
            ConfigMessage::UpdateNetworkPath(path) => {
                self.network.update_path(path);
            }
            ConfigMessage::UpdateBoot(boot) => {
                self.network.update_boot(boot);
            }
            ConfigMessage::UpdateInterface(interface) => {
                self.network.update_active(interface);
            }
        }
        self.update_config();
        Ok(())
    }

    /// Returns a list of files found in the `wireguard_folders` non-recursively
    fn get_wg_files(&self) -> io::Result<Vec<DirEntry>> {
        let mut files = Vec::new();
        for dir in &self.wireguard.folders {
            files.extend(fs::read_dir(dir)?.filter_map(Result::ok));
        }
        Ok(files)
    }

    pub fn get_network_config(&self) -> &Network {
        &self.network
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Network {
    pub active_interface: Option<String>,
    pub start_on_boot: bool,
    pub path: PathBuf,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            active_interface: None,
            start_on_boot: false,
            path: PathBuf::from("/etc/systemd/network/"),
        }
    }
}

impl Network {
    fn update_active(&mut self, cur: String) -> Result<(), ConfigError> {
        if std::path::Path::exists(&self.path.join(&cur)) {
            self.active_interface = Some(cur);
            Ok(())
        } else {
            Err(ConfigError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "Could not find the specified interface in the network folder",
            )))
        }
    }

    fn update_boot(&mut self, boot: bool) {
        self.start_on_boot = boot;
    }

    fn update_path(&mut self, path: PathBuf) -> Result<(), ConfigError> {
        if path::Path::exists(&path) {
            self.path = path;
            Ok(())
        } else {
            Err(ConfigError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "Could not locate provided path, check that the application has permissions!",
            )))
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Wireguard {
    pub folders: Vec<PathBuf>,
    pub selected_file: PathBuf,
    pub enabled: bool,
}

impl Default for Wireguard {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            selected_file: PathBuf::new(),
            enabled: false,
        }
    }
}

impl Wireguard {
    /// Adds a path to the wireguard folders
    fn add_path(&mut self, path: PathBuf) {
        self.folders.push(path);
    }

    /// Removes a path from the wireguard folders
    fn remove_path(&mut self, path: &PathBuf) {
        self.folders.retain(|p| p != path);
    }

    /// Removes all the folders in the wireguard configuration
    fn remove_all_paths(&mut self) {
        self.folders = vec![];
    }
}

impl From<io::Error> for ConfigError {
    fn from(value: io::Error) -> Self {
        ConfigError::Io(value)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        ConfigError::TomlDeserialize(value)
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(value: toml::ser::Error) -> Self {
        ConfigError::TomlSerialize(value)
    }
}
