use std::{
    fs::{self, DirEntry},
    io,
    path::PathBuf,
};

use toml_edit::{value, DocumentMut};

/// A collection paths to various configuration folders
pub struct Config {
    /// A vector of paths pointing to Wireguard configuration folders
    wireguard_folders: Vec<PathBuf>,
    /// Defaults to `/etc/systemd/network/`
    network_folder: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wireguard_folders: Vec::new(),
            network_folder: PathBuf::from("/etc/systemd/network/"),
        }
    }
}

impl Config {
    /// Creates a new `Folders` instance
    ///
    /// Ensures that configuration directory and files exist
    pub fn new() -> io::Result<Self> {
        let ins = Self::default();
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

        Self::create_default_config();
        // TODO: update wireguard_folders, network_folder based on config

        Ok(ins)
    }
    /// Reads the network files in `network_folder` and returns a vector of
    /// directory entries
    ///
    /// Entries which return `io::Error` are ignored
    pub fn get_network_files(&self) -> io::Result<Vec<DirEntry>> {
        Ok(fs::read_dir(&self.network_folder)?
            .filter_map(Result::ok)
            .collect())
    }

    /// Returns a list of files found in the `wireguard_folders` non-recursively
    pub fn get_wg_files(&self) -> io::Result<Vec<DirEntry>> {
        let mut files: Vec<DirEntry> = vec![];
        for dir in &self.wireguard_folders {
            for file in fs::read_dir(dir)? {
                files.push(file?);
            }
        }
        Ok(files)
    }

    /// Creates the default TOML config file
    fn create_default_config() {
        let toml = r#"
        [file]
        wireguard = []
        network = ""
        interface = ""

        [wireguard]
        enable = false
        selected = ""
        "#;

        let mut doc = toml.parse::<DocumentMut>().expect("Invalid TOML");
        println!("{:?}", doc["file"]["network"]);
    }
}
