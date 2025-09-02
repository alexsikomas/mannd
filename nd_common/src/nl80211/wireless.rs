use std::borrow::Cow;

use crate::{
    error::NetworkdLibError,
    nl80211::defs::{
        attr::{Attrs, Nl80211Attr},
        cmd::Nl80211Cmd,
        interface::Interface,
        station::Station,
        NL_80211_GENL_NAME, NL_80211_GENL_VERSION,
    },
};
use neli::{
    consts::{
        nl::{NlmF, Nlmsg},
        socket::NlFamily,
    },
    err::{DeError, MsgError},
    genl::{Genlmsghdr, GenlmsghdrBuilder, NlattrBuilder, NoUserHeader},
    nl::NlPayload,
    router::asynchronous::{NlRouter, NlRouterReceiverHandle},
    types::GenlBuffer,
    utils::Groups,
};

pub struct Wireless {
    router: NlRouter,
    handle: NlRouterReceiverHandle<u16, Genlmsghdr<u8, u16, NoUserHeader>>,
    family_id: u16,
}

impl Wireless {
    pub async fn connect() -> Result<Self, NetworkdLibError> {
        let (mut router, mut handle) =
            NlRouter::connect(NlFamily::Generic, None, Groups::empty()).await?;
        let family_id = router.resolve_genl_family(NL_80211_GENL_NAME).await?;

        Ok(Self {
            router,
            handle,
            family_id,
        })
    }

    pub async fn get_info_vec<T>(
        &mut self,
        interface_index: Option<i32>,
        cmd: Nl80211Cmd,
    ) -> Result<Vec<T>, NetworkdLibError>
    where
        T: for<'a> TryFrom<Attrs<'a, Nl80211Attr>, Error = DeError>,
    {
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
                                    .build()
                                    .unwrap(),
                            )
                            .nla_payload(interface_index)
                            .build()
                            .unwrap(),
                    );
                }
                attrs
            })
            .version(NL_80211_GENL_VERSION)
            .build()
            .unwrap();

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
                    return Err(NetworkdLibError::NeliMsgError(MsgError::new(
                        "Parsing response.nl_type in get_info_vec",
                    )))
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

    /// Returns vector of interfaces.
    pub async fn get_interfaces(&mut self) -> Result<Vec<Interface>, NetworkdLibError> {
        Ok(self.get_info_vec(None, Nl80211Cmd::CmdGetInterface).await?)
    }

    /// Returns all the available wireless networks
    pub async fn get_station(
        &mut self,
        interface_index: Option<i32>,
    ) -> Result<Vec<Station>, NetworkdLibError> {
        Ok(self
            .get_info_vec(interface_index, Nl80211Cmd::CmdGetStation)
            .await?)
    }

    /// Connects to a wireless network
    async fn net_connect() {
        todo!()
    }

    /// Changes the power management mode
    async fn power_management() {
        todo!()
    }

    // Interface commands
    async fn get_interface() {}

    async fn set_interface() {}

    async fn new_interface() {}

    async fn del_interface() {}

    /// Used to make neli-wifi interfaces readable, only meant for debugging
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

            if let Some(v) = station.ht_mcs {
                println!()
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
