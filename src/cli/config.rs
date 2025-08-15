use std::{
    fs::{self, DirEntry},
    io,
    path::PathBuf,
};

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
    pub fn new() -> io::Result<Self> {
        // TODO: Check for /etc/systemd/network/, if !exists prompt for location possibly panic
        let config = Self::default();
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
        if let Err(e) = fs::File::create_new(&config_path) {
            if e.kind() != io::ErrorKind::AlreadyExists {
                return Err(e);
            }
        }

        // Self::create_default_config(config_path);
        // TODO: update wireguard_folders, network_folder based on config

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
        let mut files: Vec<DirEntry> = vec![];
        for dir in &self.wireguard.folders {
            for file in fs::read_dir(dir)? {
                files.push(file?);
            }
        }
        Ok(files)
    }
}

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

struct Interface {
    /// Name of the active interface
    active_interface: String,
    /// Should the `active_interface` start on boot
    start_on_boot: bool,
    /// Network config path
    path: PathBuf,
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            active_interface: "".to_string(),
            start_on_boot: false,
            path: PathBuf::from("/etc/systemd/network/"),
        }
    }
}
