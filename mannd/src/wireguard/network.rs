//! Order in which wireguard is enabled:
//! Create wg virt interface -> assign vpn ip ->
//! link iface to conf file -> bring iface up ->
//! route all traffic through ip

use crate::{
    error::ManndError,
    netlink::{NetlinkHandle, NlRouterWrapper},
    utils::get_index,
    wireguard::wg_ini::WgConfig,
};
use neli::{consts::socket::NlFamily, router::asynchronous::NlRouter, utils::Groups};
use std::{collections::HashMap, fmt::Debug, path::Path, process::Command};
use tempfile::NamedTempFile;
use tracing::instrument;

pub const INTERFACE: &str = "wg-mannd";

pub struct Wireguard<H: NetlinkHandle> {
    pub handle: H,
    pub index: u32,
}

impl Wireguard<NlRouterWrapper> {
    /// Creates or connects to the netlink [`INTERFACE`], setting the status
    /// to be down
    pub async fn new() -> Result<Self, ManndError> {
        let (router, _receiver) = NlRouter::connect(NlFamily::Route, None, Groups::empty()).await?;
        let router = NlRouterWrapper::new(router);
        router.start_wireguard_interface(INTERFACE).await?;
        let index = get_index(INTERFACE).await?;
        router.set_interface_state(index, false).await?;
        Ok(Self {
            handle: router,
            index,
        })
    }

    pub async fn delete_interface(&self) -> Result<(), ManndError> {
        self.handle.delete_interface(INTERFACE, self.index).await?;
        Ok(())
    }

    /// Attempts to connect to a wireguard configuration
    #[instrument(err)]
    pub fn connect_conf(&self, path: &Path) -> Result<(), ManndError> {
        // holds the private key, removed again once it goes out of scope
        let pruned = Self::prune_conf(path)?;
        let output = Command::new("wg")
            .args(["setconf", INTERFACE])
            .arg(pruned.path())
            .output()?;

        if !output.status.success() {
            return Err(ManndError::OperationFailed(format!(
                "wg setconf: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }

    /// `wg` util can't understand full .conf file so needs pruning
    fn prune_conf(path: &Path) -> Result<NamedTempFile, ManndError> {
        let mut filter: HashMap<String, Vec<String>> = HashMap::new();
        filter.insert(
            "Interface".into(),
            vec!["PrivateKey".into(), "ListenPort".into()],
        );
        filter.insert(
            "Peer".into(),
            vec!["PublicKey".into(), "Endpoint".into(), "AllowedIPs".into()],
        );

        let conf = WgConfig::parse(path)?.get_partial(&filter)?;
        let mut pruned = NamedTempFile::new()?;
        conf.write_to(&mut pruned)?;
        Ok(pruned)
    }
}

impl Debug for Wireguard<NlRouterWrapper> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}
