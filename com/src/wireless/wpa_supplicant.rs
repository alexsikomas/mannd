//! Reference: https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network
//!
//! Regarding call_interface methods:
//! Not a huge fan of needing these two methods
//! but alas if you try to use .call() expecting
//! () you will get an error...
//!
//! The alternative was making this more general
//! with flags, but that requires another
//! dependency, so for now I've settled on this.

use std::collections::{HashMap, HashSet};

use futures::StreamExt;
use tokio::sync::mpsc::Sender;
use tracing::info;
use zbus::{
    names::MemberName,
    proxy::SignalStream,
    zvariant::{self, Dict, OwnedObjectPath, OwnedValue, Str, Value},
    Connection, Proxy,
};

use crate::{
    error::ComError,
    state::signals::SignalUpdate,
    wireless::common::{get_prop_from_proxy, AccessPoint, AccessPointBuilder, Security},
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {
    service: String,
    path: String,
    conn: Connection,
    // We keep track of this because by converting
    // to `AccessPoint` we lose the ObjectPath
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

// To be used externally
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

    pub async fn connect_network(
        &self,
        ssid: String,
        psk: String,
        security: Security,
    ) -> Result<(), ComError> {
        let mut body: HashMap<String, OwnedValue> = HashMap::new();
        let network_path = self.call_interface_method::<_, OwnedObjectPath>("AddNetwork", body);
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), ComError> {
        self.call_interface_method::<_, ()>("Disconnect", &())
            .await?;
        Ok(())
    }

    pub async fn status(&self) -> Result<String, ComError> {
        todo!()
    }

    pub async fn list_configured_networks(&self) -> Result<Vec<String>, ComError> {
        let networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        let network_strings: Vec<String> = networks.iter().map(|n| n.to_string()).collect();
        Ok(network_strings)
    }

    pub async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError> {
        todo!()
    }

    pub async fn scan<'a>(&self, signal_tx: Sender<SignalUpdate<'a>>) -> Result<(), ComError> {
        info!("Scan function call");
        let mut dict: HashMap<String, OwnedValue> = HashMap::new();

        dict.insert("Type".to_string(), Value::new("active").try_to_owned()?);
        self.call_interface_method_noreply("Scan", dict).await?;

        let mut scan_signal = self.get_interface_signal("ScanDone").await?;
        match signal_tx.send(SignalUpdate::Add(scan_signal)).await {
            Ok(()) => Ok(()),
            Err(_) => Err(ComError::SignalSend("in wpa_supplicant scan".to_string())),
        }
    }

    pub async fn nearby_networks(&mut self) -> Result<Vec<AccessPoint>, ComError> {
        let networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("BSSs")
            .await?;

        // to be returned
        let mut aps: Vec<AccessPoint> = vec![];
        let mut seen: HashSet<String> = HashSet::new();

        self.networks.clear();

        for network in networks {
            let proxy = Proxy::new(
                &self.conn,
                self.service.clone(),
                network.clone(),
                "fi.w1.wpa_supplicant1.BSS",
            )
            .await?;
            // ssid may appear multiple times if router broadcasts
            // ap at different freqs
            let encoded = get_prop_from_proxy::<Vec<u8>>(&proxy, "SSID").await?;
            let ssid = String::from_utf8_lossy(&encoded);

            if seen.insert(ssid.to_string()) {
                let ap = AccessPointBuilder::default()
                    .ssid(ssid.clone().to_string())
                    .security(Security::Psk)
                    .connected(false)
                    .known(false)
                    .nearby(true)
                    .build()?;

                aps.push(ap);
            }

            self.networks.insert(ssid.into_owned(), network);
        }

        Ok(aps)
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

    pub async fn is_active(conn: &Connection) -> Result<bool, ComError> {
        let proxy = Proxy::new(
            conn,
            "fi.w1.wpa_supplicant1",
            "/fi/w1/wpa_supplicant1/Interfaces/0",
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;

        let status = get_prop_from_proxy::<String>(&proxy, "State").await?;

        match status.as_str() {
            "completed" | "scanning" | "authenticating" | "associating" | "associated"
            | "4way_handshake" | "group_handshake" => Ok(true),
            _ => Ok(false),
        }
    }
}

// Helper functions
impl WpaSupplicant {
    async fn call_interface_method<T, U>(
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

    async fn call_interface_method_noreply<T>(
        &self,
        method_name: &'static str,
        body: T,
    ) -> Result<(), ComError>
    where
        T: serde::ser::Serialize + zvariant::DynamicType,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;
        proxy.call_noreply(method_name, &body).await?;
        Ok(())
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
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wpa_scan() -> Result<(), ComError> {
        let conn = Connection::system().await.unwrap();
        let mut wpa = WpaSupplicant::new(conn)?;
        let mac = wpa.get_interface_prop::<Vec<u8>>("MACAddress").await?;
        wpa.list_configured_networks().await?;

        let network = wpa.nearby_networks().await?;

        Ok(())
    }
}
