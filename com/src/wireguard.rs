use neli::{
    consts::{
        nl::NlmF,
        rtnl::{Ifa, Iff, Ifla, IflaInfo, RtAddrFamily, Rta, Rtm},
    },
    nl::NlPayload,
    router::asynchronous::NlRouter,
    rtnl::{Ifaddrmsg, IfaddrmsgBuilder, Ifinfomsg, IfinfomsgBuilder, RtattrBuilder, RtmsgBuilder},
    types::RtBuffer,
    utils::Groups,
};
use redb::{TypeName, Value};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
};
use tokio::process::Command;

use crate::{error::ComError, utils::get_index};

const INTERFACE: &str = "wg-mannd";

struct Wireguard {
    router: NlRouter,
    index: u32,
}

impl Wireguard {
    /// Connects socket and sets up `INTERFACE`
    async fn start_interface() -> Result<Self, ComError> {
        let (router, handle) =
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

        Ok(Self { router, index })
    }

    async fn delete_interface(&self) -> Result<(), ComError> {
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

    async fn set_conf(path: &'static str) -> Result<(), ComError> {
        let _ = Command::new("wg")
            .args(vec!["setconf", INTERFACE, path])
            .output()
            .await;

        Ok(())
    }
    /// Adds the IPv4/6 address to the `INTERFACE`
    async fn set_addr(&self, ips: Vec<IpAddr>) -> Result<(), ComError> {
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
    /// MTU should typically be set to 1420 since
    /// standard ethernet = 1500, worst case overhead = 80
    async fn set_mtu(&self, mtu: u32) -> Result<(), ComError> {
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
    async fn set_state(&self, go_up: bool) -> Result<(), ComError> {
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
    async fn add_wg_fwmark(&self) -> Result<(), ComError> {
        let command = Command::new("wg")
            .args(vec!["set", INTERFACE, "fwmark", "51820"])
            .output()
            .await?;
        Ok(())
    }

    async fn add_ip_fwmark(&self) -> Result<(), ComError> {
        let _ = Command::new("sudo")
            .args(vec![
                "ip", "-6", "rule", "del", "not", "fwmark", "51820", "table", "51820",
            ])
            .output()
            .await;

        let _ = Command::new("sudo")
            .args(vec![
                "ip", "-6", "rule", "add", "not", "fwmark", "51820", "table", "51820",
            ])
            .output()
            .await?;

        let _ = Command::new("sudo")
            .args(vec![
                "ip", "-4", "rule", "del", "not", "fwmark", "51820", "table", "51820",
            ])
            .output()
            .await;

        let _ = Command::new("sudo")
            .args(vec![
                "ip", "-4", "rule", "add", "not", "fwmark", "51820", "table", "51820",
            ])
            .output()
            .await?;

        Ok(())
        // This is not used because neli doesn't implement
        // FIB_RULE_INVERT (0x02)
        //
        // let FRA_FWMARK = 10;
        // let FIB_RULE_INVERT = 0x02;
        // // ipv6
        // let mut attrs = RtBuffer::new();
        // attrs.push(
        //     RtattrBuilder::default()
        //         .rta_type(Rta::Mark)
        //         .rta_payload(0xca6cu32)
        //         .build()?,
        // );
        //
        // // hex is just 51820
        // attrs.push(
        //     RtattrBuilder::default()
        //         .rta_type(Rta::Table)
        //         .rta_payload(0xca6c)
        //         .build()?,
        // );
        //
        // let rm: RtmF = 0x02.into();
        // println!("{:?}", RtmF::EQUALIZE.bits());
        // let rtmsg = RtmsgBuilder::default()
        //     .rtm_family(RtAddrFamily::Inet)
        //     .rtm_dst_len(0)
        //     .rtm_src_len(0)
        //     .rtm_tos(0)
        //     .rtm_table(neli::consts::rtnl::RtTable::Unspec)
        //     .rtm_protocol(neli::consts::rtnl::Rtprot::Unspec)
        //     .rtm_scope(neli::consts::rtnl::RtScope::Universe)
        //     .rtm_type(neli::consts::rtnl::Rtn::Unspec)
        //     .rtattrs(attrs)
        //     .build()?;
        //
        // self.router
        //     .send::<_, _, (), ()>(
        //         Rtm::Newrule,
        //         NlmF::REQUEST | NlmF::ACK | NlmF::EXCL | NlmF::CREATE,
        //         NlPayload::Payload(rtmsg),
        //     )
        //     .await?;
    }

    async fn prevent_default_route(&self) -> Result<(), ComError> {
        // neli also doesn't implement FRA_SUPPRESS_PREFIXLEN
        let _ = Command::new("sudo")
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
            .output()
            .await;

        let _ = Command::new("sudo")
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
            .output()
            .await?;

        let _ = Command::new("sudo")
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
            .output()
            .await;

        let _ = Command::new("sudo")
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
            .output()
            .await?;

        Ok(())
    }

    async fn route_traffic(&self) -> Result<(), ComError> {
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

#[derive(Debug, Clone, PartialEq)]
pub struct WgFileTable {
    path: String,
    // unix timestamp
    last_accessed: i64,
    // ISO 3166-1 alpha-2
    country: [u8; 2],
}

// Binary format as follows:
// [u32 = length of path][u8 = path][i64 = last_accessed][u32; 2 = char]
// the brackets are only for illustration
impl Value for WgFileTable {
    type AsBytes<'a> = Vec<u8>;
    type SelfType<'a> = Self;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let (len, cont) = data
            .split_first_chunk::<{ size_of::<u32>() }>()
            .expect("Too short; cannot read length");
        let len = u32::from_le_bytes(*len) as usize;

        if len > cont.len() {
            panic!("Cannot parse path, too long")
        }

        let (path, cont) = cont.split_at(len);
        let path = String::from_bytes(path);

        let (last_accessed, cont) = cont
            .split_first_chunk::<{ size_of::<i64>() }>()
            .expect("Too short; cannot read access time");
        let last_accessed = i64::from_le_bytes(*last_accessed);

        let (c1_bytes, cont) = cont
            .split_first_chunk::<{ size_of::<u8>() }>()
            .expect("Data too short for country char 1");
        let (c2_bytes, cont) = cont
            .split_first_chunk::<{ size_of::<u8>() }>()
            .expect("Data too short for country char 2");

        if !cont.is_empty() {
            panic!("Unexpected trailing data");
        }

        let c1 = u8::from_le_bytes(*c1_bytes);
        let c2 = u8::from_le_bytes(*c2_bytes);

        Self {
            path,
            last_accessed,
            country: [c1, c2],
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let path_bytes = value.path.as_bytes();
        let path_len = path_bytes.len();
        let cap = size_of::<u32>() + path_len + size_of::<i64>() + 2 * size_of::<u32>();

        let mut bytes = Vec::with_capacity(cap);
        bytes.extend_from_slice(&(path_len as u32).to_le_bytes());
        bytes.extend_from_slice(path_bytes);
        bytes.extend_from_slice(&value.last_accessed.to_le_bytes());

        bytes.extend_from_slice(&(value.country[0] as u8).to_le_bytes());
        bytes.extend_from_slice(&(value.country[1] as u8).to_le_bytes());
        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("WgFile")
    }
}

// tests
mod tests {
    use super::*;

    #[tokio::test]
    async fn wg_intergration_test() -> Result<(), ComError> {
        match caps::has_cap(
            None,
            caps::CapSet::Permitted,
            caps::Capability::CAP_NET_ADMIN,
        ) {
            Ok(val) => {
                if !val {
                    println!("Wireguard integration test must be run with net_admin permission");
                    return Err(ComError::OperationFailed("Failed".to_string()));
                }
            }
            Err(e) => {
                println!("Error occured while checking capabilities! {e}");
                return Err(ComError::OperationFailed("Failed".to_string()));
            }
        }

        let wg = Wireguard::start_interface().await?;
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
