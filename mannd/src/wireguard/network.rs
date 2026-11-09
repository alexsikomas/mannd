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
use std::{
    collections::HashMap,
    fmt::Debug,
    path::{Path, PathBuf},
    process::Command,
};
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

    #[instrument(err)]
    /// Attempts to connect to a wireguard configuration
    pub fn connect_conf(&self, path: &Path) -> Result<(), ManndError> {
        Self::set_conf(path)?;
        Ok(())
    }

    /// `wg` util can't understand full .conf file so needs pruning
    #[instrument(err)]
    fn prune_write_conf(path: &str) -> Result<String, ManndError> {
        let mut filter: HashMap<String, Vec<String>> = HashMap::new();
        filter.insert(
            "Interface".into(),
            vec!["PrivateKey".into(), "ListenPort".into()],
        );
        filter.insert(
            "Peer".into(),
            vec!["PublicKey".into(), "Endpoint".into(), "AllowedIPs".into()],
        );

        let conf = WgConfig::parse(&Path::new(path))?;
        let conf = conf.get_partial(&filter)?;

        let write_path = format!("{path}.mannd.tmp");
        conf.write_file(&Path::new(&write_path))?;
        Ok(write_path)
    }

    #[instrument(err)]
    fn set_conf<T: Into<PathBuf> + Debug>(path: T) -> Result<(), ManndError> {
        let conf_path = Self::prune_write_conf(path.into().to_str().unwrap())?;
        Command::new("wg")
            .args(vec!["setconf", INTERFACE, &conf_path])
            .output()?;

        Ok(())
    }

}

impl Debug for Wireguard<NlRouterWrapper> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}
