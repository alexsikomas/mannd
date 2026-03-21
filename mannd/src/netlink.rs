use neli::consts::nl::{NlTypeWrapper, NlmF};
use neli::consts::rtnl::{Arphrd, Ifa, Iff, Ifla, IflaInfo, RtAddrFamily, Rta, Rtm};
use neli::nl::{NlPayload, NlmsghdrBuilder};
use neli::router::asynchronous::NlRouter;
use neli::rtnl::{
    Ifaddrmsg, IfaddrmsgBuilder, Ifinfomsg, IfinfomsgBuilder, RtattrBuilder, RtmsgBuilder,
};
use neli::socket::asynchronous::NlSocketHandle;
use neli::types::RtBuffer;
use neli::utils::Groups;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::error::ManndError;

pub struct NlRouterWrapper {
    router: NlRouter,
}

pub trait NetlinkHandle {
    fn start_wireguard_interface(&self, name: &str)
    -> impl Future<Output = Result<(), ManndError>>;
    fn check_interface_state(&self, name: &str) -> impl Future<Output = Result<bool, ManndError>>;
    fn delete_interface(
        &self,
        name: &str,
        index: u32,
    ) -> impl Future<Output = Result<(), ManndError>>;
    fn set_interface_addr(
        &self,
        index: u32,
        ips: Vec<IpAddr>,
    ) -> impl Future<Output = Result<(), ManndError>>;
    fn set_interface_mtu(
        &self,
        index: u32,
        mtu: u32,
    ) -> impl Future<Output = Result<(), ManndError>>;
    fn set_interface_state(
        &self,
        index: u32,
        go_up: bool,
    ) -> impl Future<Output = Result<(), ManndError>>;
    fn interface_route_traffic(&self, index: u32) -> impl Future<Output = Result<(), ManndError>>;
}

impl NlRouterWrapper {
    pub fn new(router: NlRouter) -> Self {
        Self { router }
    }
}

impl NetlinkHandle for NlRouterWrapper {
    async fn start_wireguard_interface(&self, name: &str) -> Result<(), ManndError> {
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
                .rta_payload(name)
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

        self.router
            .send::<Rtm, Ifinfomsg, (), ()>(
                Rtm::Newlink,
                NlmF::REQUEST | NlmF::ACK | NlmF::EXCL | NlmF::CREATE,
                NlPayload::Payload(ifinfomsg),
            )
            .await?;
        Ok(())
    }

    async fn check_interface_state(&self, name: &str) -> Result<bool, ManndError> {
        // TODO: Can you do this without socket?
        let socket =
            NlSocketHandle::connect(neli::consts::socket::NlFamily::Route, None, Groups::empty())?;

        let mut buf = RtBuffer::new();
        buf.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Ifname)
                .rta_payload(name)
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

    async fn delete_interface(&self, name: &str, index: u32) -> Result<(), ManndError> {
        let mut attrs = RtBuffer::new();
        attrs.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Ifname)
                .rta_payload(name)
                .build()?,
        );

        let ifinfomsg = IfinfomsgBuilder::default()
            .ifi_family(neli::consts::rtnl::RtAddrFamily::Unspecified)
            .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
            .ifi_index(index as i32)
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

    async fn set_interface_state(&self, index: u32, go_up: bool) -> Result<(), ManndError> {
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

    async fn set_interface_addr(&self, index: u32, ips: Vec<IpAddr>) -> Result<(), ManndError> {
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

    async fn set_interface_mtu(&self, index: u32, mtu: u32) -> Result<(), ManndError> {
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

    async fn interface_route_traffic(&self, index: u32) -> Result<(), ManndError> {
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
                .rta_payload(index)
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
                .rta_payload(index)
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
}
