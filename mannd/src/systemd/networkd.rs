use std::path::PathBuf;

use tokio::fs::read_dir;
use tracing::instrument;

use crate::error::ManndError;

const NETWORK_FOLDER: &str = "/etc/systemd/network/";

struct Section {
    name: String,
    props: Vec<(String, String)>,
}

#[instrument(err)]
pub async fn get_netd_files() -> Result<Vec<String>, ManndError> {
    let extensions = ["netdev", "network"];
    let mut dirs: Vec<PathBuf> = vec![PathBuf::from(NETWORK_FOLDER)];
    let mut files: Vec<String> = vec![];

    while let Some(path) = dirs.pop() {
        let mut cur_dir = read_dir(path).await?;
        while let Some(entry) = cur_dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if let Some(ext) = path.extension() {
                let ext_str = ext.to_str().ok_or_else(|| {
                    ManndError::OperationFailed("Converting ext to string".to_string())
                })?;
                if extensions.contains(&ext_str) {
                    files.push(path.to_string_lossy().into_owned());
                }
            }
        }
    }

    Ok(files)
}
