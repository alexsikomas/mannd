use std::{
    io::{self},
    net::IpAddr,
    path::PathBuf,
};

use tokio::{
    fs::{File, read_dir},
    io::AsyncWriteExt,
};

use crate::error::ComError;

const NETWORK_FOLDER: &'static str = "/etc/systemd/network/";
const VIRTUAL_INTERFACE: &'static str = "mannd";

struct Section {
    name: String,
    props: Vec<(String, String)>,
}

// used for each section of .network/.netdev files
impl Section {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            props: vec![],
        }
    }

    /// returns &mut self to allow for chaining
    fn set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.props.push((key.into(), value.into()));
        self
    }

    async fn write(&self, file: &mut File) -> io::Result<()> {
        file.write(format!("[{}]\n", self.name).as_bytes()).await?;
        for (key, val) in &self.props {
            file.write(format!("{}={}\n", key, val).as_bytes()).await?;
            if &self.props[self.props.len() - 1].0 == key {
                file.write(b"\n").await?;
            }
        }
        Ok(())
    }
}

pub async fn get_netd_files() -> Result<Vec<PathBuf>, ComError> {
    let extensions = vec!["netdev", "network"];
    let mut dirs: Vec<PathBuf> = vec![PathBuf::from(NETWORK_FOLDER)];
    let mut files: Vec<PathBuf> = vec![];

    while let Some(path) = dirs.pop() {
        let mut cur_dir = read_dir(path).await?;
        while let Some(entry) = cur_dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                if let Some(ext) = path.extension() {
                    if extensions.contains(&ext.to_str().unwrap()) {
                        files.push(path);
                    }
                }
            }
        }
    }

    Ok(files)
}

pub async fn init_virtual_interface(mut ips: Vec<IpAddr>, dns: IpAddr) -> Result<(), ComError> {
    // Since virt interface made by netlink we use .network file
    let path = PathBuf::from(format!("{}/30-mannd.network", NETWORK_FOLDER));
    let mut file = File::create(&path).await?;

    let match_section = Section::new("Match").set("Name", "wg-mannd");
    match_section.write(&mut file).await?;

    // route all traffic through wg-mannd
    let ipv4_route = Section::new("Route").set("Destination", "0.0.0.0/0");
    ipv4_route.write(&mut file).await?;
    let ipv6_route = Section::new("Route").set("Destination", "::/0");
    ipv6_route.write(&mut file).await?;

    let mut network_section = Section::new("Network");

    while let Some(ip) = ips.pop() {
        network_section = network_section.set("Address", ip.to_string());
    }

    network_section = network_section.set("DNS", dns.to_string());
    network_section.write(&mut file).await?;
    Ok(())
}

mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_get_netd_files() -> Result<(), ComError> {
        let _ = get_netd_files().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_init_virt_interface() -> Result<(), ComError> {
        init_virtual_interface(
            vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
        )
        .await?;
        Ok(())
    }
}
