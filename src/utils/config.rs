use std::{
    ffi::OsStr, io, path::{self, PathBuf}
};
use tokio::fs::{self, DirEntry};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum ConfigError {
    Io(String),
    TomlSerialize(String),
    TomlDeserialize(String),
    ConfigDirNotFound,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    pub async fn new() -> Result<Self, ConfigError> {
        let mut config_path = dirs::config_dir().ok_or(ConfigError::ConfigDirNotFound)?;

        config_path.push("networkd-wireguard-manager");
        fs::create_dir_all(&config_path).await?;
        config_path.push("config.toml");

        let toml_file = match fs::read_to_string(&config_path).await {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let default_config = Self::default();
                let default_toml = toml::to_string(&default_config)?;
                fs::write(&config_path, default_toml).await?;
                return Ok(default_config);
            }
            Err(e) => return Err(e.into())
        };


        let mut config: Config = toml::from_str(&toml_file)?;
        config.config_path = config_path.clone();

        if fs::metadata(&config.network.path).await.is_ok() {
            config.network.update_interfaces().await?;
            Ok(config)
        } else {
            Err(ConfigError::Io(
                "Cannot find systemd network folder!".to_string(),
            ))
        }
    }

    pub async fn update_config(&self) -> Result<(), ConfigError> {
        let content = toml::to_string(&self)?;
        fs::write(&self.config_path, content).await?;
        Ok(())
    }

    /// Returns a list of files found in the `wireguard_folders` non-recursively
    pub async fn get_wg_files(&self) -> io::Result<Vec<DirEntry>> {
        let mut files = Vec::new();
        for dir in &self.wireguard.folders {
            let mut entries = fs::read_dir(dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                files.push(entry);
            }
        }
        Ok(files)
    }

    pub fn get_network_config(&self) -> &Network {
        &self.network
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Network {
    pub active_interface: String,
    pub start_on_boot: bool,
    pub path: PathBuf,
    pub interfaces: Vec<String>,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            active_interface: "".to_string(),
            start_on_boot: false,
            path: PathBuf::from("/etc/systemd/network/"),
            interfaces: vec![],
        }
    }
}

impl Network {
    async fn update_active(&mut self, cur: String) -> Result<(), ConfigError> {
        if fs::metadata(&self.path.join(&cur)).await.is_ok() {
            self.active_interface = cur;
            Ok(())
        } else {
            Err(ConfigError::Io("Could not find the specified interface in the network folder".to_string(),
            ))
        }
    }

    fn update_boot(&mut self, boot: bool) {
        self.start_on_boot = boot;
    }

    async fn update_path(&mut self, path: PathBuf) -> Result<(), ConfigError> {
        if fs::metadata(&path).await.is_ok() {
            self.path = path;
            Ok(())
        } else {
            Err(ConfigError::Io(
                "Could not locate provided path, check that the application has permissions!".to_string(),
            ))
        }
    }

    async fn update_interfaces(&mut self) -> Result<(), ConfigError> {
        self.interfaces = vec![];
        let mut entries = fs::read_dir(&self.path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().unwrap_or_else(||OsStr::new("")) == "network" {
                self.interfaces.push(entry.file_name().into_string().unwrap());
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
        ConfigError::Io(value.to_string())
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        ConfigError::TomlDeserialize(value.to_string())
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(value: toml::ser::Error) -> Self {
        ConfigError::TomlSerialize(value.to_string())
    }
}
