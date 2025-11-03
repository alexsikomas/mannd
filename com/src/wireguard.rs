use futures::stream::StreamExt;

use neli::{
    attr::Attribute,
    consts::{
        nl::NlmF,
        rtnl::{Ifa, Iff, Ifla, IflaInfo, RtAddrFamily, Rtm},
    },
    genl::{AttrTypeBuilder, Genlmsghdr, NlattrBuilder},
    nl::{NlPayload, NlmsghdrBuilder},
    router::asynchronous::{NlRouter, NlRouterReceiverHandle},
    rtnl::{Ifaddrmsg, IfaddrmsgBuilder, Ifinfomsg, IfinfomsgBuilder, Rtattr, RtattrBuilder},
    types::{GenlBuffer, NlBuffer, RtBuffer},
    utils::Groups,
    ToBytes,
};
use std::{any::Any, ffi::CStr, io::Cursor, net::IpAddr};
use tokio::process::Command;
use tracing::info;

use crate::{error::ComError, utils::get_index};

const INTERFACE: &str = "wg-mannd";

struct Wireguard {
    router: NlRouter,
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

        Ok(Self { router })
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
        let index = get_index(INTERFACE).await?;

        for ip in ips {
            match ip {
                IpAddr::V4(addr) => {
                    let mut attrs = RtBuffer::new();

                    println!("addr: {}", addr.to_string());
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
                        .ifa_index(index)
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
                        .ifa_index(index)
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
        let index = get_index(INTERFACE).await?;
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
            .ifi_index(index as i32)
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
        let index = get_index(INTERFACE).await?;

        let ifi = match go_up {
            true => IfinfomsgBuilder::default()
                .ifi_family(RtAddrFamily::Unspecified)
                .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
                .ifi_index(index as i32)
                .ifi_flags(Iff::UP)
                .ifi_change(Iff::from_bits_truncate(1))
                .build()?,
            false => IfinfomsgBuilder::default()
                .ifi_family(RtAddrFamily::Unspecified)
                .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
                .ifi_index(index as i32)
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

    /// Configures the system DNS to exclusively use name servers from `INTERFACE`
    /// This will ignore all other DNS configurations due to `-x` flag
    async fn update_dns(&self) -> Result<(), ComError> {
        let mut command = Command::new("resolvconf")
            .args(vec!["-a", INTERFACE, "-m", "0", "-x"])
            .output()
            .await?;
        Ok(())
    }

    /// Prevents routing loop
    ///
    /// Applies firewall mark for port 51820 to it's outgoing
    /// packets
    async fn add_fwmark(&self) -> Result<(), ComError> {
        let mut command = Command::new("wg")
            .args(vec!["set", INTERFACE, "fwmark", "51820"])
            .output()
            .await?;
        Ok(())
    }

    // [#] resolvconf -a il-tlv-wg-102 -m 0 -x
    // resolvconf: signature mismatch: /etc/resolv.conf
    // resolvconf: run `resolvconf -u` to update
}

// tests
mod tests {
    use super::*;
    use std::{
        net::{Ipv4Addr, Ipv6Addr},
        sync::Arc,
    };

    #[tokio::test]
    async fn wg_intergration_test() -> Result<(), ComError> {
        let wg = Wireguard::start_interface().await?;

        wg.set_addr(vec![
            IpAddr::V4(Ipv4Addr::new(12, 76, 70, 29)),
            IpAddr::V6(Ipv6Addr::new(
                0xfa00, 0xb1bb, 0x7bbb, 0xbb21, 1, 0, 0x9, 0x4694,
            )),
        ])
        .await?;

        wg.set_mtu(1420).await?;
        Ok(())
    }
}
