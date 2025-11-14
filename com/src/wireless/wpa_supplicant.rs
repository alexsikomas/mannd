//! Reference: https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network
use std::collections::HashMap;

use async_trait::async_trait;
use futures::StreamExt;
use zbus::{
    names::MemberName,
    proxy::SignalStream,
    zvariant::{self, Dict, OwnedObjectPath, OwnedValue, Str, Value},
    Connection, Proxy,
};

use crate::{
    error::ComError,
    wireless::{
        common::{get_prop_from_proxy, Security},
        WifiAdapter,
    },
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {
    service: String,
    path: String,
    conn: Connection,
    networks: HashMap<String, OwnedObjectPath>,
}

#[derive(Debug, Clone)]
pub struct WpaBss {
    bssid: Vec<u8>,
    ssid: Vec<u8>,
    rsn: HashMap<String, OwnedValue>,
    // wpa: HashMap<String, OwnedValue>,
    // wps: HashMap<String, OwnedValue>,
    /// mhz
    freq: u16,
    /// bits per second
    rates: Vec<u32>,
    // signal: i16,
}

#[async_trait]
impl WifiAdapter for WpaSupplicant {
    async fn connect_network(
        &self,
        ssid: String,
        psk: String,
        security: Security,
    ) -> Result<(), ComError> {
        let mut body: HashMap<String, OwnedValue> = HashMap::new();
        let network_path = self.call_interface_method::<_, OwnedObjectPath>("AddNetwork", body);
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), ComError> {
        self.call_interface_method::<_, ()>("Disconnect", &())
            .await?;
        Ok(())
    }
    async fn status(&self) -> Result<String, ComError> {
        todo!()
    }

    async fn list_configured_networks(&self) -> Result<Vec<String>, ComError> {
        let networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        let network_strings: Vec<String> = networks.iter().map(|n| n.to_string()).collect();
        Ok(network_strings)
    }

    async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError> {
        todo!()
    }
}

impl WpaSupplicant {
    pub fn new(conn: Connection) -> Result<Self, ComError> {
        let service = String::from("fi.w1.wpa_supplicant1");
        let path = String::from("/fi/w1/wpa_supplicant1/Interfaces/0");
        let networks: HashMap<String, OwnedObjectPath> = HashMap::new();

        Ok(Self {
            conn,
            service,
            path,
            networks,
        })
    }

    pub async fn call_interface_method<T, U>(
        &self,
        method_name: &'static str,
        body: T,
    ) -> Result<U, ComError>
    where
        T: serde::ser::Serialize + zvariant::DynamicType,
        U: for<'a> zvariant::DynamicDeserialize<'a>,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;
        let res: U = proxy.call(method_name, &body).await?;
        Ok(res)
    }

    pub async fn get_interface_prop<'a, T>(&self, prop: &'static str) -> Result<T, ComError>
    where
        T: TryFrom<Value<'a>>,
        <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;
        return Ok(get_prop_from_proxy::<T>(&proxy, prop).await?);
    }

    pub async fn get_interface_signal<'a, M>(
        &self,
        signal_name: M,
    ) -> Result<SignalStream<'a>, ComError>
    where
        M: TryInto<MemberName<'a>>,
        M::Error: Into<zbus::Error>,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;

        let stream = proxy.receive_signal(signal_name).await?;
        Ok(stream)
    }

    pub async fn scan(&self, wait: bool) -> Result<(), ComError> {
        let mut dict: HashMap<String, OwnedValue> = HashMap::new();

        dict.insert("Type".to_string(), Value::new("active").try_to_owned()?);
        self.call_interface_method::<_, ()>("Scan", dict).await?;

        if wait {
            let mut scan_signal = self.get_interface_signal("ScanDone").await?;
            let message = scan_signal.next().await;
        }

        Ok(())
    }

    pub async fn nearby_networks(&mut self) -> Result<(), ComError> {
        let networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("BSSs")
            .await?;

        for network in networks {
            println!("{:?}", network.clone());
            let proxy = Proxy::new(
                &self.conn,
                self.service.clone(),
                network.clone(),
                "fi.w1.wpa_supplicant1.BSS",
            )
            .await?;
            let encoded = get_prop_from_proxy::<Vec<u8>>(&proxy, "SSID").await?;
            let ssid = String::from_utf8_lossy(&encoded);
            self.networks.insert(ssid.into_owned(), network);
        }
        println!("{:?}", self.networks);
        Ok(())
    }

    pub async fn get_network_info(&self, bss_path: OwnedObjectPath) -> Result<WpaBss, ComError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            bss_path,
            "fi.w1.wpa_supplicant1.BSS",
        )
        .await?;

        let bssid = get_prop_from_proxy::<Vec<u8>>(&proxy, "BSSID").await?;
        let ssid = get_prop_from_proxy::<Vec<u8>>(&proxy, "SSID").await?;
        let rsn = get_prop_from_proxy::<HashMap<String, OwnedValue>>(&proxy, "RSN").await?;
        let freq = get_prop_from_proxy::<u16>(&proxy, "Frequency").await?;
        let rates = get_prop_from_proxy::<Vec<u32>>(&proxy, "Rates").await?;

        Ok(WpaBss {
            bssid,
            ssid,
            rsn,
            freq,
            rates,
        })
    }
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wpa_scan() -> Result<(), ComError> {
        let conn = Connection::system().await.unwrap();
        let mut wpa = WpaSupplicant::new(conn)?;
        let mac = wpa.get_interface_prop::<Vec<u8>>("MACAddress").await?;
        wpa.list_configured_networks().await?;

        wpa.scan(false).await?;
        let network = wpa.nearby_networks().await?;

        Ok(())
    }
}
