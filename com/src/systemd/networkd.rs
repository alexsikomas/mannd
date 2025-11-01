use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use tokio::fs::{read_dir, ReadDir};

use crate::error::ComError;

const NETWORK_FOLDER: &'static str = "/etc/systemd/network/";

pub async fn get_netd_files() -> Result<Vec<PathBuf>, ComError> {
    let mut dirs: Vec<PathBuf> = vec![PathBuf::from(NETWORK_FOLDER)];
    let mut files: Vec<PathBuf> = vec![];

    while let Some(path) = dirs.pop() {
        let mut cur_dir = read_dir(path).await?;
        while let Some(entry) = cur_dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }

    Ok(files)
}

pub async fn init_network_file() -> Result<(), ComError> {
    Ok(())
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_netd_files() -> Result<(), ComError> {
        let files = get_netd_files().await?;
        println!("{:?}", files);
        Ok(())
    }
}
