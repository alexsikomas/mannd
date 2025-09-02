use std::{borrow::Cow, error::Error};

use crate::{
    error::{NeliError, NetworkdLibError},
    nl80211::{defs::Nl80211Cmd, interface::Interface, socket::Wifi},
};
use neli::{
    consts::socket::NlFamily, err::SocketError, socket::asynchronous::NlSocketHandle,
    utils::Groups, ToBytes,
};

pub struct Wireless {
    wifi: Wifi,
}

impl Wireless {
    pub async fn new() -> Result<Self, NetworkdLibError> {
        Ok(Self {
            wifi: Wifi::connect().await?,
        })
    }

    /// Returns vector of interfaces.
    pub async fn get_interfaces(&mut self) -> Result<Vec<Interface>, NetworkdLibError> {
        Ok(self
            .wifi
            .get_info_vec(None, Nl80211Cmd::CmdGetInterface)
            .await?)
    }

    /// Returns all the available wireless networks
    async fn get_networks(&mut self) {}

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
    pub fn format_interfaces() {
        todo!()
        // if interfaces.is_empty() {
        //     println!("No Wi-Fi interfaces found!");
        //     return;
        // }
        //
        // println!("Found {} Wi-Fi interfaces:", interfaces.len());
        // println!("--------------------------------------------------");
        //
        // for (i, interface) in interfaces.iter().enumerate() {
        //     println!("Interface [{}]:\n", i + 1);
        //
        //     println!(" Interface Index: {}", interface.index.unwrap_or(0));
        //
        //     let ssid = interface
        //         .ssid
        //         .as_ref()
        //         .map(|v| String::from_utf8_lossy(v))
        //         .unwrap_or(Cow::Borrowed("N/A"));
        //
        //     println!(" SSID:            {}", ssid);
        //
        //     let mac = interface
        //         .mac
        //         .as_ref()
        //         .map_or_else(|| "N/A".to_string(), |v| Self::format_mac_address(v));
        //     println!(" MAC Address:     {}", mac);
        //
        //     let name = interface
        //         .name
        //         .as_ref()
        //         .map(|v| String::from_utf8_lossy(v))
        //         .unwrap_or(Cow::Borrowed("N/A"));
        //
        //     println!(" Interface Name:  {}", name);
        //
        //     println!(" Frequency:       {} MHz", interface.frequency.unwrap_or(0));
        //     println!(" Channel:         {}", interface.channel.unwrap_or(0));
        //     println!(" Power:           {} dBm", interface.power.unwrap_or(0));
        //     println!(" Wiphy index:     {}", interface.phy.unwrap_or(0));
        //     println!(" Device:          {}", interface.device.unwrap_or(0));
        //
        //     if i < interfaces.len() - 1 {
        //         println!("--------------------------------------------------");
        //     }
        // }
        // println!("--------------------------------------------------");
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
