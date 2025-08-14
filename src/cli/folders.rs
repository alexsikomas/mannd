use std::{
    fs::{self, DirEntry},
    io,
    path::PathBuf,
};

/// A collection paths to various configuration folders
pub struct Folders {
    /// A vector of paths pointing to Wireguard configuration folders
    wireguard_folders: Vec<PathBuf>,
    /// Defaults to `/etc/systemd/network/`
    network_folder: PathBuf,
}

impl Default for Folders {
    fn default() -> Self {
        Self {
            wireguard_folders: Vec::new(),
            network_folder: PathBuf::from("/etc/systemd/network/"),
        }
    }
}

impl Folders {
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
}
