//! Order in which wireguard is enabled:
//! Create wg virt interface -> assign vpn ip ->
//! link iface to conf file -> bring iface up ->
//! route all traffic through ip

use crate::{
    error::ManndError,
    ini_parse::IniConfig,
    netlink::{NetlinkHandle, NlRouterWrapper},
    utils::{get_index, str_to_ip},
};
use neli::{consts::socket::NlFamily, router::asynchronous::NlRouter, utils::Groups};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    net::IpAddr,
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
        let conf = IniConfig::new(path.into())?;

        // get ips, possibly multiple split on ,
        let mut ips: Vec<IpAddr> = vec![];
        match conf.sections.get("Interface") {
            Some(iface) => match iface.get("Address") {
                Some(addrs) => {
                    for addr in addrs.split(',') {
                        let ip = str_to_ip(addr)?;
                        ips.push(ip);
                    }
                }
                None => return Err(ManndError::WgIps),
            },
            None => return Err(ManndError::SectionNotFound("Interface".to_string())),
        }

        Self::set_conf(path)?;
        Ok(())
    }

    /// `wg` util can't understand full .conf file so needs pruning
    #[instrument(err)]
    fn prune_write_conf(path: &str) -> Result<String, ManndError> {
        let mut filter: BTreeMap<String, Vec<String>> = BTreeMap::new();
        filter.insert(
            "Interface".into(),
            vec!["PrivateKey".into(), "ListenPort".into()],
        );
        filter.insert(
            "Peer".into(),
            vec!["PublicKey".into(), "Endpoint".into(), "AllowedIPs".into()],
        );

        let conf = IniConfig::new(path.into())?;
        let conf = conf.get_partial(filter)?;

        let write_path = format!("{path}.mannd.tmp");
        conf.write_file(Some(write_path.clone().into()))?;
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

    /// Prevents routing loop
    ///
    /// Applies firewall mark for port 51820 to it's outgoing
    /// packets
    #[instrument(err)]
    async fn add_wg_fwmark(&self) -> Result<(), ManndError> {
        Command::new("wg")
            .args(vec!["set", INTERFACE, "fwmark", "51820"])
            .output()?;
        Ok(())
    }

    #[instrument(err)]
    async fn add_ip_fwmark(&self) -> Result<(), ManndError> {
        Command::new("sudo")
            .args(vec![
                "ip", "-6", "rule", "del", "not", "fwmark", "51820", "table", "51820",
            ])
            .output()?;

        Command::new("sudo")
            .args(vec![
                "ip", "-6", "rule", "add", "not", "fwmark", "51820", "table", "51820",
            ])
            .output()?;

        Command::new("sudo")
            .args(vec![
                "ip", "-4", "rule", "del", "not", "fwmark", "51820", "table", "51820",
            ])
            .output()?;

        Command::new("sudo")
            .args(vec![
                "ip", "-4", "rule", "add", "not", "fwmark", "51820", "table", "51820",
            ])
            .output()?;

        Ok(())
    }

    #[instrument(err)]
    async fn prevent_default_route(&self) -> Result<(), ManndError> {
        // neli also doesn't implement FRA_SUPPRESS_PREFIXLEN
        Command::new("sudo")
            .args(vec![
                "ip",
                "-6",
                "rule",
                "del",
                "table",
                "main",
                "suppress_prefixlength",
                "0",
            ])
            .output()?;

        Command::new("sudo")
            .args(vec![
                "ip",
                "-6",
                "rule",
                "add",
                "table",
                "main",
                "suppress_prefixlength",
                "0",
            ])
            .output()?;

        Command::new("sudo")
            .args(vec![
                "ip",
                "-4",
                "rule",
                "del",
                "table",
                "main",
                "suppress_prefixlength",
                "0",
            ])
            .output()?;

        Command::new("sudo")
            .args(vec![
                "ip",
                "-4",
                "rule",
                "add",
                "table",
                "main",
                "suppress_prefixlength",
                "0",
            ])
            .output()?;

        Ok(())
    }
}

impl Debug for Wireguard<NlRouterWrapper> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}
