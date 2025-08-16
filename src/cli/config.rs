use std::{
    fmt::write,
    fs::{self, DirEntry},
    io,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    wireguard: Wireguard,
    interface: Interface,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wireguard: Wireguard::default(),
            interface: Interface::default(),
        }
    }
}

impl Config {
    /// Creates a new `Folders` instance
    ///
    /// Ensures that configuration directory and files exist
    /// Panics if the `interface` path cannot be found
    pub fn new() -> io::Result<Self> {
        let mut config = Self::default();
        let mut config_path: PathBuf;

        if let Some(path) = dirs::config_dir() {
            config_path = path;
        } else {
            println!("Could not find config directory from dirs");
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Could not find config directory",
            ));
        }

        config_path.push("networkd-wireguard-manager");
        fs::create_dir_all(&config_path)?;

        config_path.push("config.toml");
        let toml_file = fs::read_to_string(&config_path);
        config = match toml_file {
            Ok(data) => toml::from_str(&data).unwrap_or_default(),
            Err(_) => {
                let default_config = Self::default();
                // BUG: will make an empty config if serialisation fails
                fs::write(
                    &config_path,
                    toml::to_string(&default_config).unwrap_or_default(),
                )?;
                default_config
            }
        };

        if let Err(e) = fs::exists(&config.interface.path) {
            // possibly use channels to prompt for location in event it is somewhere else
            panic!("Cannot find systemd network folder!");
        }

        println!("{:?}", config);
        Ok(config)
    }

    /// Reads the network files in `network_folder` and returns a vector of
    /// directory entries
    ///
    /// Entries which return `io::Error` are ignored
    pub fn get_network_files(&self) -> io::Result<Vec<DirEntry>> {
        Ok(fs::read_dir(&self.interface.path)?
            .filter_map(Result::ok)
            .collect())
    }

    /// Returns a list of files found in the `wireguard_folders` non-recursively
    pub fn get_wg_files(&self) -> io::Result<Vec<DirEntry>> {
        let mut files = Vec::new();
        for dir in &self.wireguard.folders {
            files.extend(fs::read_dir(dir)?.filter_map(Result::ok));
        }
        Ok(files)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Wireguard {
    /// Folders containing wireguard configuration files
    folders: Vec<PathBuf>,
    /// Path to a wireguard configuration file
    selected_file: PathBuf,
    /// Should the wireguard config of `selected_file` be active
    enabled: bool,
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

#[derive(Serialize, Deserialize, Debug)]
struct Interface {
    /// Name of the active interface
    active_interface: Option<String>,
    /// Should the `active_interface` start on boot
    start_on_boot: bool,
    /// Network config path
    path: PathBuf,
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            active_interface: None,
            start_on_boot: false,
            path: PathBuf::from("/etc/systemd/network/"),
        }
    }
}
