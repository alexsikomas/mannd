//! Reference: https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network
//!
//! The WpaBss struct has a lot of optional types mainly because it
//! differs significantly to what is provided from a scan vs a connected
//! network. Check the differences between properties in a .Network vs
//! .BSS
//!
//! When getting networks after a scan there will be duplicate SSIDs
//! because wpa_supplicant shows the different possible freqs.
//!
//! Regarding call_interface methods:
//! Not a huge fan of needing these two methods
//! but if you try to use .call() expecting ()
//! you will get an error...
//!
//! The alternative was making this more general
//! with flags, but that requires another
//! dependency, so for now I've settled on this.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use futures::StreamExt;
use tokio::{sync::mpsc::Sender, time::timeout};
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
    wireless::common::{
        get_prop_from_proxy, AccessPoint, AccessPointBuilder, NetworkFlags, Security,
    },
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {
    service: String,
    path: String,
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct WpaBss {
    ssid: String,
    bssid: Option<Vec<u8>>,
    security: Option<Security>,
    /// mhz
    freq: Option<u16>,
    /// bits per second
    rates: Option<Vec<u32>>,
}

// To be used externally
impl WpaSupplicant {
    pub fn new(conn: Connection) -> Result<Self, ComError> {
        let service = String::from("fi.w1.wpa_supplicant1");
        let path = String::from("/fi/w1/wpa_supplicant1/Interfaces/0");

        Ok(Self {
            conn,
            service,
            path,
        })
    }

    pub async fn connect_network(&self, ssid: String, psk: String) -> Result<(), ComError> {
        let psk_len = psk.len();
        if (psk_len < 8 || psk_len > 63) && psk_len != 0 {
            return Err(ComError::PasswordLength);
        }
        // let networks = self.networks.get(&ssid).unwrap();

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;

        let mut body: HashMap<String, OwnedValue> = HashMap::new();
        body.insert("ssid".into(), Value::new(ssid).try_to_owned()?);
        if psk_len == 0 {
            body.insert("key_mgmt".into(), Value::new("NONE").try_to_owned()?);
        } else {
            body.insert("psk".into(), Value::new(psk).try_to_owned()?);
        }

        let network_path = self
            .call_interface_method::<_, OwnedObjectPath>("AddNetwork", body)
            .await?;

        self.call_interface_method_noreply("SelectNetwork", network_path.clone())
            .await?;

        let mut stream = proxy.receive_signal("PropertiesChanged").await?;
        match self.check_connection(stream).await {
            Err(e) => {
                self.call_interface_method_noreply("RemoveNetwork", network_path)
                    .await?;
                Err(e)
            }
            _ => Ok(()),
        }
    }

    pub async fn disconnect(&self) -> Result<(), ComError> {
        self.call_interface_method_noreply("Disconnect", &())
            .await?;
        Ok(())
    }

    pub async fn status(&self) -> Result<String, ComError> {
        todo!()
    }

    pub async fn list_configured_networks(&self) -> Result<Vec<AccessPoint>, ComError> {
        let networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        let aps: Vec<AccessPoint> = vec![];
        for network in networks {
            let ap = self.get_network_info(network).await?;
        }

        Ok(aps)
    }

    pub async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError> {
        todo!()
    }

    pub async fn scan<'a>(&self, signal_tx: Sender<SignalUpdate<'a>>) -> Result<(), ComError> {
        if self.get_interface_prop::<bool>("Scanning").await? {
            return Ok(());
        }

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

        for network in networks {
            // ssid may appear multiple times if router broadcasts
            // ap at different freqs
            let bss = self.get_bss_info(network.clone()).await?;

            if seen.insert(bss.ssid.clone()) {
                let ap = AccessPointBuilder::default()
                    .ssid(bss.ssid.clone())
                    .security(bss.security.clone().unwrap())
                    .flags(NetworkFlags::NEARBY)
                    .build()?;

                aps.push(ap);
            }
        }

        Ok(aps)
    }

    /// Used for networks which have already been connected to by wpa supplicant
    pub async fn get_network_info(&self, net_path: OwnedObjectPath) -> Result<WpaBss, ComError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            net_path,
            "fi.w1.wpa_supplicant1.Network",
        )
        .await?;
        // the bssid can only be found via .Interface path

        let mut properties =
            get_prop_from_proxy::<HashMap<String, OwnedValue>>(&proxy, "Properties").await?;

        let ssid = properties
            .remove("ssid")
            .unwrap_or(Value::new("Unknown").try_into_owned()?);
        let ssid = ssid.to_string();

        Ok(WpaBss {
            ssid,
            bssid: None,
            security: None,
            freq: None,
            rates: None,
        })
    }

    /// Used for networks which are nearby by may
    /// not have been connected to yet
    pub async fn get_bss_info(&self, bss_path: OwnedObjectPath) -> Result<WpaBss, ComError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            bss_path,
            "fi.w1.wpa_supplicant1.BSS",
        )
        .await?;

        let bssid = Some(get_prop_from_proxy::<Vec<u8>>(&proxy, "BSSID").await?);
        let ssid_vec = get_prop_from_proxy::<Vec<u8>>(&proxy, "SSID").await?;
        let ssid = String::from_utf8_lossy(&ssid_vec).to_string();

        let freq = Some(get_prop_from_proxy::<u16>(&proxy, "Frequency").await?);
        let rates = Some(get_prop_from_proxy::<Vec<u32>>(&proxy, "Rates").await?);

        let rsn = get_prop_from_proxy::<HashMap<String, Value>>(&proxy, "RSN").await?;
        let security = Some(Self::get_security(rsn));

        Ok(WpaBss {
            ssid,
            bssid,
            security,
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

        // let status = get_prop_from_proxy::<String>(&proxy, "State").await?;

        // match status.as_str() {
        //     "completed" | "scanning" | "authenticating" | "associating" | "associated"
        //     | "4way_handshake" | "group_handshake" => Ok(true),
        //     _ => Ok(false),
        // }
        Ok(true)
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

    fn get_security<'a>(rsn: HashMap<String, Value<'a>>) -> Security {
        let mut security = Security::Open;

        // eap and psk can't be mixed (afaik)
        // so we only need to check one if it contains
        // 'psk' or 'eap'
        'sec: {
            // WEP/WPA1 network
            if rsn.is_empty() {
                break 'sec;
            }

            if let Some(arr) = rsn.get("KeyMgmt") {
                if let Ok(sec_types) = arr.clone().downcast::<Vec<String>>() {
                    // if this occurs it will assume the network
                    // is open, unless I can find that this is
                    // possible I'll keep it as is
                    if sec_types.is_empty() {
                        break 'sec;
                    }
                    security = if sec_types.first().unwrap().contains("psk") {
                        Security::Psk
                    } else {
                        Security::Ieee8021x
                    };
                } else {
                    tracing::error!(
                        "Could not cast 'KeyMgmt' to an array of strings, which it should be."
                    );
                }
            } else {
                tracing::error!("RSN non-empty but KeyMgmt not found!");
            }
        }
        security
    }

    async fn check_connection<'a>(&self, mut stream: SignalStream<'a>) -> Result<(), ComError> {
        let start = Instant::now();
        let max_wait = Duration::from_secs(15);

        // If first connected to a network then expect a disconnected first
        // SUCCESS: authenticating -> associating -> 4-way-handshake -> completed
        // FAILURE: associating -> 4-way-handshake -> disconnected (incorrect password)
        // FAILURE: scanning -> scanning -> scanning -> ... (cannot find network)
        loop {
            match timeout(Duration::from_secs(1), stream.next()).await {
                Ok(Some(msg)) => {
                    let res: HashMap<String, OwnedValue> = msg.body().deserialize()?;
                    if let Some(state) = res.get("State") {
                        let state_str = state.downcast_ref::<&str>().unwrap_or("Unknown");
                        info!("WPA STATE: {}", state_str);
                        match state_str {
                            "completed" => {
                                info!("Connected successfully!");
                                return Ok(());
                            }
                            "disconnected" => {
                                // since success also uses disconnected we check
                                // how long we have been going first
                                if start.elapsed().as_secs() > 2 {
                                    return Err(ComError::ConnectionFailed(
                                        "WPA rejected network request, check password".into(),
                                    ));
                                }
                            }
                            "inactive" => {
                                return Err(ComError::ConnectionFailed(
                                    "Interface is inactive!".into(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                Ok(None) => {
                    return Err(ComError::OperationFailed(
                        "DBus stream ended unexpectedly.".into(),
                    ));
                }
                Err(_) => {
                    if start.elapsed() > max_wait {
                        return Err(ComError::Timeout);
                    }
                }
            }
        }
    }

    async fn get_interface_prop<'a, T>(&self, prop: &'static str) -> Result<T, ComError>
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

    async fn get_interface_signal<'a, M>(
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
