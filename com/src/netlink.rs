use std::{borrow::Cow, fmt::Debug, net::Ipv4Addr};

use crate::{
    error::ComError,
    utils::get_name,
    wireless::defs::{
        attr::{Attrs, Nl80211Attr},
        bss::Bss,
        cmd::Nl80211Cmd,
        interface::Interface,
        station::Station,
        NL_80211_GENL_NAME, NL_80211_GENL_VERSION,
    },
};

use neli::{
    consts::{
        nl::{NlTypeWrapper, NlmF, Nlmsg},
        rtnl::{Rta, Rtm, RtmF},
        socket::NlFamily,
    },
    err::{DeError, MsgError},
    genl::{Genlmsghdr, GenlmsghdrBuilder, NlattrBuilder, NoUserHeader},
    nl::{NlPayload, NlmsghdrBuilder},
    router::asynchronous::{NlRouter, NlRouterReceiverHandle},
    rtnl::{RtattrBuilder, Rtmsg, RtmsgBuilder},
    socket::asynchronous::NlSocketHandle,
    types::{GenlBuffer, RtBuffer},
    utils::Groups,
};
use tracing::{info, instrument};

pub struct Netlink {
    router: NlRouter,
    handle: NlRouterReceiverHandle<u16, Genlmsghdr<u8, u16, NoUserHeader>>,
    family_id: u16,
    mcast: Multicast,
}

impl Debug for Netlink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WirelessNetlink")
            .field("family_id", &self.family_id)
            .finish()
    }
}

impl Netlink {
    #[instrument]
    pub async fn connect_wireless() -> Result<Self, ComError> {
        let (router, handle) = NlRouter::connect(NlFamily::Generic, None, Groups::empty()).await?;
        let family_id = router.resolve_genl_family(NL_80211_GENL_NAME).await?;
        let scan_group = router
            .resolve_nl_mcast_group(NL_80211_GENL_NAME, "scan")
            .await?;

        let (mcast_sock, mcast_recv) =
            NlRouter::connect(NlFamily::Generic, None, Groups::new_groups(&[scan_group])).await?;

        info!("Successfully created wireless netlink connection");
        Ok(Self {
            router,
            handle,
            family_id,
            mcast: Multicast {
                sock: mcast_sock,
                recv: mcast_recv,
            },
        })
    }

    #[instrument]
    async fn nl_info<T>(
        &mut self,
        interface_index: Option<i32>,
        cmd: Nl80211Cmd,
    ) -> Result<Vec<T>, ComError>
    where
        T: for<'a> TryFrom<Attrs<'a, Nl80211Attr>, Error = DeError>,
    {
        let mut attrs = GenlBuffer::new();

        let msghdr = GenlmsghdrBuilder::<Nl80211Cmd, Nl80211Attr>::default()
            .cmd(cmd)
            .attrs({
                if let Some(interface_index) = interface_index {
                    attrs.push(
                        NlattrBuilder::default()
                            .nla_type(
                                neli::genl::AttrTypeBuilder::default()
                                    .nla_type(Nl80211Attr::AttrIfindex)
                                    .build()?,
                            )
                            .nla_payload(interface_index)
                            .build()?,
                    );
                }
                attrs
            })
            .version(NL_80211_GENL_VERSION)
            .build()?;

        let mut recv: NlRouterReceiverHandle<Nlmsg, Genlmsghdr<Nl80211Cmd, Nl80211Attr>> = self
            .router
            .send(
                self.family_id,
                NlmF::REQUEST | NlmF::DUMP,
                NlPayload::Payload(msghdr),
            )
            .await?;

        let mut retval = Vec::new();

        while let Some(response) = recv
            .next::<Nlmsg, Genlmsghdr<Nl80211Cmd, Nl80211Attr>>()
            .await
        {
            let response = response?;
            match response.nl_type() {
                Nlmsg::Noop => (),
                Nlmsg::Error => {
                    let err = "Parsing response.nl_type in get_info_vec";
                    tracing::error!(err);
                    return Err(ComError::NeliMsg(MsgError::new(err)));
                }
                Nlmsg::Done => return Ok(retval),
                _ => retval.push(
                    response
                        .get_payload()
                        .unwrap()
                        .attrs()
                        .get_attr_handle()
                        .try_into()?,
                ),
            };
        }

        Ok(retval)
    }

    /// Used to send commands
    #[instrument]
    async fn nl_action(
        &mut self,
        interface_index: Option<i32>,
        cmd: Nl80211Cmd,
    ) -> Result<NlRouterReceiverHandle<Nlmsg, Genlmsghdr<Nl80211Cmd, Nl80211Attr>>, ComError> {
        info!("Prepearing to perform netlink action");
        let msghdr = GenlmsghdrBuilder::<Nl80211Cmd, Nl80211Attr>::default()
            .cmd(cmd)
            .attrs({
                let mut attrs = GenlBuffer::new();
                if let Some(interface_index) = interface_index {
                    attrs.push(
                        NlattrBuilder::default()
                            .nla_type(
                                neli::genl::AttrTypeBuilder::default()
                                    .nla_type(Nl80211Attr::AttrIfindex)
                                    .build()?,
                            )
                            .nla_payload(interface_index)
                            .build()?,
                    );
                }
                attrs
            })
            .version(NL_80211_GENL_VERSION)
            .build()?;

        info!("Created netlink message header");
        let recv: NlRouterReceiverHandle<Nlmsg, Genlmsghdr<Nl80211Cmd, Nl80211Attr>> = self
            .router
            .send(
                self.family_id,
                NlmF::REQUEST | NlmF::ACK,
                NlPayload::Payload(msghdr),
            )
            .await?;

        Ok(recv)
    }

    /// Returns vector of interfaces.
    pub async fn get_interfaces(&mut self) -> Result<Vec<Interface>, ComError> {
        Ok(self.nl_info(None, Nl80211Cmd::CmdGetInterface).await?)
    }

    /// Returns vector of stations
    pub async fn get_station(
        &mut self,
        interface_index: Option<i32>,
    ) -> Result<Vec<Station>, ComError> {
        Ok(self
            .nl_info(interface_index, Nl80211Cmd::CmdGetStation)
            .await?)
    }

    /// Returns all the available wireless networks
    pub async fn get_bss(&mut self, interface_index: Option<i32>) -> Result<Vec<Bss>, ComError> {
        self.nl_action(interface_index, Nl80211Cmd::CmdTriggerScan)
            .await?;

        // Wait until CmdNewScanResults is recieved i.e. scan completed
        loop {
            match self
                .mcast
                .recv
                .next::<Nlmsg, Genlmsghdr<Nl80211Cmd, Nl80211Attr>>()
                .await
            {
                Some(Ok(v)) => match v.get_payload() {
                    Some(p) => {
                        if p.cmd().cmp(&Nl80211Cmd::CmdNewScanResults).is_eq() {
                            break;
                        }
                    }
                    None => {}
                },
                Some(Err(e)) => {
                    return Err(ComError::NeliRouter(Box::new(e)));
                }
                _ => {}
            }
        }

        Ok(self
            .nl_info(interface_index, Nl80211Cmd::CmdGetScan)
            .await?)
    }

    /// Used to identify the main physical interface
    ///
    /// uses route command to see which interface processes it
    pub async fn get_main_interface() -> Result<(String, u32), ComError> {
        // can't use self router as we need route here
        let socket = NlSocketHandle::connect(NlFamily::Route, None, Groups::empty())?;
        // Does the following command
        // ip rotue get 8.8.8.8
        let mut buffer = RtBuffer::new();
        buffer.push(
            RtattrBuilder::default()
                .rta_type(Rta::Dst)
                .rta_payload(Ipv4Addr::new(8, 8, 8, 8).octets())
                .build()?,
        );

        let rtmsg = RtmsgBuilder::default()
            .rtm_family(neli::consts::rtnl::RtAddrFamily::Inet)
            .rtm_dst_len(32)
            .rtm_src_len(0)
            .rtm_tos(0)
            .rtm_table(neli::consts::rtnl::RtTable::Unspec)
            .rtm_protocol(neli::consts::rtnl::Rtprot::Unspec)
            .rtm_scope(neli::consts::rtnl::RtScope::Universe)
            .rtm_type(neli::consts::rtnl::Rtn::Unspec)
            .rtm_flags(RtmF::LOOKUPTABLE)
            .rtattrs(buffer)
            .build()?;

        let nlmsg = NlmsghdrBuilder::default()
            .nl_type(Rtm::Getroute)
            .nl_flags(NlmF::REQUEST)
            .nl_payload(NlPayload::Payload(rtmsg))
            .build()?;

        socket.send(&nlmsg).await?;
        let messages = socket.recv_all::<NlTypeWrapper, Rtmsg>().await?;

        let mut index = 0;
        for msg in messages.0 {
            if let Some(payload) = msg.get_payload() {
                if let Some(attr) = payload.rtattrs().get_attr_handle().get_attribute(Rta::Oif) {
                    let bytes = attr.rta_payload().as_ref();
                    if bytes.len() == 4 {
                        let arr: [u8; 4] = bytes.try_into().unwrap();
                        index = u32::from_le_bytes(arr);
                    }
                }
            }
        }
        let name = get_name(index).await?;
        Ok((name, index))
    }

    /// Changes the power management mode
    async fn power_management() {
        todo!()
    }

    // Interface commands
    // async fn get_interface(&self) {
    // self.nl_info(interface_index, Nl80211Cmd::CmdGetInterface);
    // }

    async fn set_interface() {}

    async fn new_interface() {}

    async fn del_interface() {}

    /// For debugging purposes
    pub fn format_interfaces(interfaces: &Vec<Interface>) {
        if interfaces.is_empty() {
            println!("No Wi-Fi interfaces found!");
            return;
        }

        println!("Found {} Wi-Fi interfaces:", interfaces.len());
        println!("--------------------------------------------------");

        for (i, interface) in interfaces.iter().enumerate() {
            println!("Interface [{}]:\n", i + 1);

            println!(" Interface Index: {}", interface.index.unwrap_or(0));

            let ssid = interface
                .ssid
                .as_ref()
                .map(|v| String::from_utf8_lossy(v))
                .unwrap_or(Cow::Borrowed("N/A"));

            println!(" SSID:            {}", ssid);

            let mac = interface
                .mac
                .as_ref()
                .map_or_else(|| "N/A".to_string(), |v| Self::format_mac_address(v));
            println!(" MAC Address:     {}", mac);

            let name = interface
                .name
                .as_ref()
                .map(|v| String::from_utf8_lossy(v))
                .unwrap_or(Cow::Borrowed("N/A"));

            println!(" Interface Name:  {}", name);

            println!(" Frequency:       {} MHz", interface.frequency.unwrap_or(0));
            println!(" Channel:         {}", interface.channel.unwrap_or(0));
            println!(" Power:           {} dBm", interface.power.unwrap_or(0));
            println!(" Wiphy index:     {}", interface.phy.unwrap_or(0));
            println!(" Device:          {}", interface.device.unwrap_or(0));

            if i < interfaces.len() - 1 {
                println!("--------------------------------------------------");
            }
        }
        println!("--------------------------------------------------");
    }

    /// For debugging purposes
    pub fn format_station(stations: &Vec<Station>) {
        if stations.is_empty() {
            println!("No station found!");
            return;
        }

        println!("Found {} station:", stations.len());
        println!("--------------------------------------------------");

        for (i, station) in stations.iter().enumerate() {
            println!("Interface [{}]:\n", i + 1);

            if let Some(bssid) = &station.bssid {
                println!(" Station BSSID: {}", Self::format_mac_address(bssid))
            }

            if let Some(v) = station.average_signal {
                println!(" Average Signal: {} dBm", v);
            }

            if let Some(v) = station.beacon_loss {
                println!(" Beacon Loss: {} dBm", v);
            }

            if let Some(v) = station.connected_time {
                println!(" Connected Time: {}s", v);
            }

            if let Some(v) = station.rx_bitrate {
                println!(" Reception Bitrate: {}", v);
            }

            if let Some(v) = station.rx_packets {
                println!(" Total Received Packets: {}", v);
            }

            if let Some(v) = station.signal {
                println!(" Current Signal: {} dBm", v);
            }

            if let Some(v) = station.tx_bitrate {
                println!(" Transmission Bitrate: {}", v);
            }

            if let Some(v) = station.tx_failed {
                println!(" Failed Packets: {}", v);
            }

            if let Some(v) = station.tx_retries {
                println!(" No. Packet Retries: {}", v);
            }

            // wifi 4-7 in order
            let connection_types = vec![
                station.ht_mcs,
                station.vht_mcs,
                station.he_mcs,
                station.eht_mcs,
            ];

            for (i, v) in connection_types.iter().enumerate() {
                match v {
                    Some(_) => {
                        println!(" WiFi Version: {}", i + 4);
                    }
                    _ => {}
                }
            }

            if i < stations.len() - 1 {
                println!("--------------------------------------------------");
            }
        }
        println!("--------------------------------------------------");
    }

    pub fn format_bss(bss_vec: Vec<Bss>) {
        if bss_vec.is_empty() {
            println!("Scan empty!");
            return;
        }

        println!("Found {} BSS:", bss_vec.len());
        println!("--------------------------------------------------");

        for (i, bss) in bss_vec.iter().enumerate() {
            println!("BSS [{}]:\n", i + 1);

            if let Some(info) = &bss.information_elements {
                // info in form [0,l,...] where l is number of elements until end of ssid
                let len = info[1] as usize;
                let mut buf = String::with_capacity(len);
                for i in 2..(len + 2) {
                    buf.push(info[i] as char);
                }
                println!(" SSID: {buf}");
            }

            if let Some(id) = &bss.bssid {
                println!(" BSSID: {}", Self::format_mac_address(id))
            }

            if let Some(freq) = bss.frequency {
                println!(" Frequency: {}", freq)
            }

            if let Some(interval) = bss.beacon_interval {
                println!(" Beacon Interval: {}", interval)
            }

            if let Some(seen) = bss.seen_ms_ago {
                println!(" Last seen: {}ms", seen)
            }

            if let Some(status) = bss.status {
                println!(" Status: {}", status)
            }

            if let Some(signal) = bss.signal {
                println!(" Signal: {}mBm", signal)
            }

            if i < bss_vec.len() - 1 {
                println!("--------------------------------------------------");
            }
        }
        println!("--------------------------------------------------");
    }

    fn format_mac_address(mac: &[u8]) -> String {
        if mac.is_empty() {
            return "N/A".to_string();
        }
        mac.iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<String>>()
            .join(":")
    }
}

struct Multicast {
    sock: NlRouter,
    recv: NlRouterReceiverHandle<u16, Genlmsghdr<u8, u16>>,
}

mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    #[tokio::test]
    async fn json_route_test() -> Result<(), ComError> {
        Netlink::get_main_interface().await?;
        Ok(())
    }
}
