//! Order in which wireguard is enabled:
//! Create wg virt interface -> assign vpn ip ->
//! link iface to conf file -> bring iface up ->
//! route all traffic through ip

use crate::{
    error::ManndError,
    ini_parse::IniConfig,
    store::WgMeta,
    utils::{get_index, str_to_ip},
};
use neli::{
    consts::{
        nl::{NlTypeWrapper, NlmF},
        rtnl::{Arphrd, Ifa, Iff, Ifla, IflaInfo, RtAddrFamily, Rta, Rtm},
    },
    nl::{NlPayload, NlmsghdrBuilder},
    router::asynchronous::NlRouter,
    rtnl::{Ifaddrmsg, IfaddrmsgBuilder, Ifinfomsg, IfinfomsgBuilder, RtattrBuilder, RtmsgBuilder},
    socket::asynchronous::NlSocketHandle,
    types::RtBuffer,
    utils::Groups,
};
use redb::Database;
use std::{
    collections::BTreeMap,
    fmt::Debug,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
    process::Command,
};

const INTERFACE: &str = "wg-mannd";

pub struct Wireguard {
    router: NlRouter,
    index: u32,
}

// methods used by Network
impl Wireguard {
    /// Connects socket and sets up [`INTERFACE`] if not already done so
    pub async fn start_interface(db: Option<Database>) -> Result<Self, ManndError> {
        let (router, _) =
            NlRouter::connect(neli::consts::socket::NlFamily::Route, None, Groups::empty()).await?;

        let mut linked_attrs = RtBuffer::new();
        linked_attrs.push(
            RtattrBuilder::default()
                .rta_type(IflaInfo::Kind)
                .rta_payload("wireguard")
                .build()?,
        );

        let mut attrs = RtBuffer::new();
        attrs.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Ifname)
                .rta_payload(INTERFACE)
                .build()?,
        );

        attrs.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Linkinfo)
                .rta_payload(linked_attrs)
                .build()?,
        );

        let ifinfomsg = IfinfomsgBuilder::default()
            .ifi_family(neli::consts::rtnl::RtAddrFamily::Unspecified)
            .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
            .ifi_index(0)
            .rtattrs(attrs)
            .build()?;

        router
            .send::<Rtm, Ifinfomsg, (), ()>(
                Rtm::Newlink,
                NlmF::REQUEST | NlmF::ACK | NlmF::EXCL | NlmF::CREATE,
                NlPayload::Payload(ifinfomsg),
            )
            .await?;

        let index = get_index(INTERFACE).await?;

        let s = Self { router, index };

        Self::set_state(&s, false).await?;

        return Ok(s);
    }

    pub fn connect(&self, path: PathBuf) -> Result<(), ManndError> {
        let conf = IniConfig::new(path.clone())?;

        // get ips, possibly multiple split on ,
        let mut ips: Vec<IpAddr> = vec![];
        match conf.sections.get("Interface") {
            Some(iface) => match iface.get("Address") {
                Some(addrs) => {
                    for addr in addrs.split(",") {
                        let ip = str_to_ip(addr)?;
                        ips.push(ip);
                    }
                }
                None => return Err(ManndError::WgIps),
            },
            None => return Err(ManndError::ConfigSectionNotFound("Interface".to_string())),
        };

        Self::set_conf(path)?;
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), ManndError> {
        self.delete_interface().await?;
        Ok(())
    }
}

#[allow(dead_code)]
impl Wireguard {
    pub async fn check_state() -> Result<bool, ManndError> {
        let socket =
            NlSocketHandle::connect(neli::consts::socket::NlFamily::Route, None, Groups::empty())?;

        let mut buf = RtBuffer::new();
        buf.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Ifname)
                .rta_payload(INTERFACE)
                .build()?,
        );

        let ifimsg = IfinfomsgBuilder::default()
            .ifi_family(RtAddrFamily::Unspecified)
            .ifi_type(Arphrd::None)
            .ifi_index(0)
            .ifi_flags(0.into())
            .ifi_change(0.into())
            .rtattrs(buf)
            .build()?;

        let msg = NlmsghdrBuilder::default()
            .nl_type(Rtm::Getlink)
            .nl_flags(NlmF::REQUEST)
            .nl_payload(NlPayload::Payload(ifimsg))
            .build()?;

        socket.send(&msg).await?;

        if let Ok(msg) = socket.recv::<NlTypeWrapper, Ifinfomsg>().await {
            for msg in msg.0.into_iter() {
                if let Some(_) = msg.unwrap().get_payload() {
                    return Ok(true);
                };
                return Ok(false);
            }
        }
        Ok(false)
    }

    /// `wg` util can't understand full .conf file so needs pruning
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

        let write_path = format!("{}.mannd.tmp", path);
        conf.write_file(Some(write_path.clone().into()))?;
        Ok(write_path)
    }

    async fn delete_interface(&self) -> Result<(), ManndError> {
        let mut attrs = RtBuffer::new();
        attrs.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Ifname)
                .rta_payload(INTERFACE)
                .build()?,
        );

        let ifinfomsg = IfinfomsgBuilder::default()
            .ifi_family(neli::consts::rtnl::RtAddrFamily::Unspecified)
            .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
            .ifi_index(self.index as i32)
            .rtattrs(attrs)
            .build()?;

        self.router
            .send::<Rtm, Ifinfomsg, (), ()>(
                Rtm::Dellink,
                NlmF::REQUEST | NlmF::ACK,
                NlPayload::Payload(ifinfomsg),
            )
            .await?;

        Ok(())
    }

    fn set_conf(path: impl Into<PathBuf>) -> Result<(), ManndError> {
        let conf_path = Self::prune_write_conf(path.into().to_str().unwrap())?;
        Command::new("wg")
            .args(vec!["setconf", INTERFACE, &conf_path])
            .output()?;

        Ok(())
    }

    /// Adds the IPv4/6 address to the `INTERFACE`
    async fn set_addr(&self, ips: Vec<IpAddr>) -> Result<(), ManndError> {
        for ip in ips {
            match ip {
                IpAddr::V4(addr) => {
                    let mut attrs = RtBuffer::new();
                    attrs.push(
                        RtattrBuilder::default()
                            .rta_type(Ifa::Local)
                            .rta_payload(addr.octets())
                            .build()?,
                    );
                    attrs.push(
                        RtattrBuilder::default()
                            .rta_type(Ifa::Address)
                            .rta_payload(addr.octets())
                            .build()?,
                    );

                    let ifaddr = IfaddrmsgBuilder::default()
                        .ifa_family(neli::consts::rtnl::RtAddrFamily::Inet)
                        .ifa_index(self.index)
                        .rtattrs(attrs)
                        .ifa_prefixlen(32)
                        .ifa_scope(neli::consts::rtnl::RtScope::Universe)
                        .build()?;

                    self.router
                        .send::<_, _, Rtm, Ifaddrmsg>(
                            Rtm::Newaddr,
                            NlmF::REQUEST | NlmF::ACK | NlmF::EXCL | NlmF::CREATE,
                            NlPayload::Payload(ifaddr),
                        )
                        .await?;
                }
                IpAddr::V6(addr) => {
                    let mut attrs = RtBuffer::new();

                    attrs.push(
                        RtattrBuilder::default()
                            .rta_type(Ifa::Local)
                            .rta_payload(addr.octets())
                            .build()?,
                    );

                    let ifaddr = IfaddrmsgBuilder::default()
                        .ifa_family(neli::consts::rtnl::RtAddrFamily::Inet6)
                        .ifa_index(self.index)
                        .rtattrs(attrs)
                        .ifa_prefixlen(128)
                        .ifa_scope(neli::consts::rtnl::RtScope::Universe)
                        .build()?;

                    self.router
                        .send::<_, _, Rtm, Ifaddrmsg>(
                            Rtm::Newaddr,
                            NlmF::REQUEST | NlmF::ACK | NlmF::EXCL | NlmF::CREATE,
                            NlPayload::Payload(ifaddr),
                        )
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Sets MTU to prevent ip fragmentation
    ///
    /// MTU typically 1420 since standard ethernet = 1500,
    /// worst case overhead = 80
    async fn set_mtu(&self, mtu: u32) -> Result<(), ManndError> {
        let mut attrs = RtBuffer::new();

        attrs.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Mtu)
                .rta_payload(mtu)
                .build()?,
        );
        let ifi = IfinfomsgBuilder::default()
            .ifi_family(RtAddrFamily::Unspecified)
            .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
            .ifi_index(self.index as i32)
            .rtattrs(attrs)
            .build()?;

        self.router
            .send::<_, _, (), ()>(
                Rtm::Newlink,
                NlmF::REQUEST | NlmF::ACK,
                NlPayload::Payload(ifi),
            )
            .await?;
        Ok(())
    }

    /// Set state of `INTERFACE` via Netlink
    async fn set_state(&self, go_up: bool) -> Result<(), ManndError> {
        let ifi = match go_up {
            true => IfinfomsgBuilder::default()
                .ifi_family(RtAddrFamily::Unspecified)
                .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
                .ifi_index(self.index as i32)
                .ifi_flags(Iff::UP)
                .ifi_change(Iff::from_bits_truncate(1))
                .build()?,
            false => IfinfomsgBuilder::default()
                .ifi_family(RtAddrFamily::Unspecified)
                .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
                .ifi_index(self.index as i32)
                .ifi_flags(Iff::empty())
                .ifi_change(Iff::UP)
                .build()?,
        };

        self.router
            .send::<_, _, (), ()>(
                Rtm::Newlink,
                NlmF::REQUEST | NlmF::ACK,
                NlPayload::Payload(ifi),
            )
            .await?;
        Ok(())
    }

    /// Prevents routing loop
    ///
    /// Applies firewall mark for port 51820 to it's outgoing
    /// packets
    async fn add_wg_fwmark(&self) -> Result<(), ManndError> {
        Command::new("wg")
            .args(vec!["set", INTERFACE, "fwmark", "51820"])
            .output()?;
        Ok(())
    }

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

    async fn route_traffic(&self) -> Result<(), ManndError> {
        let mut attrs = RtBuffer::new();
        // ipv4
        attrs.push(
            RtattrBuilder::default()
                .rta_type(Rta::Dst)
                .rta_payload(Ipv4Addr::new(0, 0, 0, 0).octets())
                .build()?,
        );

        attrs.push(
            RtattrBuilder::default()
                .rta_type(Rta::Oif)
                .rta_payload(self.index)
                .build()?,
        );

        let rtmsg = RtmsgBuilder::default()
            .rtm_family(RtAddrFamily::Inet)
            .rtm_dst_len(0)
            .rtm_src_len(0)
            .rtm_tos(0)
            .rtm_table(neli::consts::rtnl::RtTable::Unspec)
            .rtm_protocol(neli::consts::rtnl::Rtprot::Boot)
            .rtm_scope(neli::consts::rtnl::RtScope::Link)
            .rtm_type(neli::consts::rtnl::Rtn::Unicast)
            .rtattrs(attrs)
            .build()?;

        self.router
            .send::<_, _, (), ()>(Rtm::Newroute, NlmF::REQUEST, NlPayload::Payload(rtmsg))
            .await?;

        let mut attrs = RtBuffer::new();

        attrs.push(
            RtattrBuilder::default()
                .rta_type(Rta::Dst)
                .rta_payload(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).octets())
                .build()?,
        );

        attrs.push(
            RtattrBuilder::default()
                .rta_type(Rta::Oif)
                .rta_payload(self.index)
                .build()?,
        );

        let rtmsg = RtmsgBuilder::default()
            .rtm_family(RtAddrFamily::Inet6)
            .rtm_dst_len(0)
            .rtm_src_len(0)
            .rtm_tos(0)
            .rtm_table(neli::consts::rtnl::RtTable::Unspec)
            .rtm_protocol(neli::consts::rtnl::Rtprot::Boot)
            .rtm_scope(neli::consts::rtnl::RtScope::Link)
            .rtm_type(neli::consts::rtnl::Rtn::Unicast)
            .rtattrs(attrs)
            .build()?;

        self.router
            .send::<_, _, (), ()>(Rtm::Newroute, NlmF::REQUEST, NlPayload::Payload(rtmsg))
            .await?;

        Ok(())
    }

    // [#] resolvconf -a il-tlv-wg-102 -m 0 -x
    // resolvconf: signature mismatch: /etc/resolv.conf
    // resolvconf: run `resolvconf -u` to update
}

impl Debug for Wireguard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}

// tests
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[tokio::test]
    async fn wg_intergration_test() -> Result<(), ManndError> {
        let tmp = NamedTempFile::new().unwrap();
        let db = Database::create(tmp.path()).unwrap();

        let wg = Wireguard::start_interface(Some(db)).await?;
        wg.set_addr(vec![
            IpAddr::V4(Ipv4Addr::new(12, 76, 70, 29)),
            IpAddr::V6(Ipv6Addr::new(
                0xfa00, 0xb1bb, 0x7bbb, 0xbb21, 1, 0, 0x9, 0x4694,
            )),
        ])
        .await?;

        wg.set_mtu(1420).await?;
        wg.add_wg_fwmark().await?;
        wg.add_ip_fwmark().await?;
        wg.prevent_default_route().await?;
        wg.route_traffic().await?;

        wg.delete_interface().await?;
        Ok(())
    }
}
