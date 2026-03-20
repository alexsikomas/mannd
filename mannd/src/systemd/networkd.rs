use std::path::PathBuf;

use tokio::fs::read_dir;
use tracing::instrument;

use crate::error::ManndError;

const NETWORK_FOLDER: &'static str = "/etc/systemd/network/";

struct Section {
    name: String,
    props: Vec<(String, String)>,
}

#[instrument(err)]
pub async fn get_netd_files() -> Result<Vec<String>, ManndError> {
    let extensions = vec!["netdev", "network"];
    let mut dirs: Vec<PathBuf> = vec![PathBuf::from(NETWORK_FOLDER)];
    let mut files: Vec<String> = vec![];

    while let Some(path) = dirs.pop() {
        let mut cur_dir = read_dir(path).await?;
        while let Some(entry) = cur_dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                if let Some(ext) = path.extension() {
                    if extensions.contains(&ext.to_str().unwrap()) {
                        files.push(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    Ok(files)
}

// likely don't need this
// pub async fn init_virtual_interface(mut ips: Vec<IpAddr>, dns: IpAddr) -> Result<(), ManndError> {
//     // Since virt interface made by netlink we use .network file
//     let path = PathBuf::from(format!("{}/30-mannd.network", NETWORK_FOLDER));
//     let mut file = File::create(&path).await?;
//
//     let match_section = Section::new("Match").set("Name", "wg-mannd");
//     match_section.write(&mut file).await?;
//
//     // route all traffic through wg-mannd
//     let ipv4_route = Section::new("Route").set("Destination", "0.0.0.0/0");
//     ipv4_route.write(&mut file).await?;
//     let ipv6_route = Section::new("Route").set("Destination", "::/0");
//     ipv6_route.write(&mut file).await?;
//
//     let mut network_section = Section::new("Network");
//
//     while let Some(ip) = ips.pop() {
//         network_section = network_section.set("Address", ip.to_string());
//     }
//
//     network_section = network_section.set("DNS", dns.to_string());
//     network_section.write(&mut file).await?;
//     Ok(())
// }

mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    #[instrument(err)]
    async fn test_get_netd_files() -> Result<(), ManndError> {
        let _ = get_netd_files().await?;
        Ok(())
    }

    // #[tokio::test]
    // async fn test_init_virt_interface() -> Result<(), ManndError> {
    //     init_virtual_interface(
    //         vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
    //         IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
    //     )
    //     .await?;
    //     Ok(())
    // }
}
