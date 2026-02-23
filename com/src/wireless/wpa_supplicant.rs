//! Reference: <https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network>
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
    path::PathBuf,
    time::{Duration, Instant},
};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc::Sender, time::timeout};
use tracing::info;
use zbus::{
    names::MemberName,
    proxy::SignalStream,
    zvariant::{self, NoneValue, OwnedObjectPath, OwnedValue, Value},
    Connection, Proxy,
};

use crate::{
    error::ManndError,
    ini_parse::IniConfig,
    state::signals::SignalUpdate,
    systemd::systemctl::get_service_path,
    utils::list_interfaces,
    wireless::common::{
        get_prop_from_proxy, AccessPoint, AccessPointBuilder, NetworkFlags, Security,
    },
    STATE_HOME,
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {
    service: String,
    path: String,
    conn: Connection,
    // only one interface dealing with Wi-Fi
    // at a time
    active_interface: OwnedObjectPath,
    // if false will not edit .service files
    overwrite_service: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WpaInterface {
    Managed(String),
    Unmanaged(String),
}

impl<'a> Into<String> for &'a WpaInterface {
    fn into(self) -> String {
        match self {
            WpaInterface::Managed(v) => v.clone(),
            WpaInterface::Unmanaged(v) => v.clone(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WpaBss {
    ssid: String,
    bssid: Option<Vec<u8>>,
    security: Option<Security>,
    /// mhz
    freq: Option<u16>,
    /// bits per second
    rates: Option<Vec<u32>>,
}

#[derive(Debug, Clone)]
pub struct WpaKnownNetwork {
    ssid: String,
    connected: bool,
}

// To be used externally
impl WpaSupplicant {
    pub async fn new(conn: Connection) -> Result<Self, ManndError> {
        let service = String::from("fi.w1.wpa_supplicant1");
        let path = String::from("/fi/w1/wpa_supplicant1");
        let proxy = Proxy::new(&conn, service.clone(), path.clone(), service.clone()).await?;

        let mut active_interfaces =
            get_prop_from_proxy::<Vec<OwnedObjectPath>>(&proxy, "Interfaces").await?;
        let active_interface: OwnedObjectPath;

        if active_interfaces.is_empty() {
            active_interface = OwnedObjectPath::null_value();
        } else {
            active_interface = active_interfaces.swap_remove(0);
        }
        // let mut inplace_changes = ;

        Ok(Self {
            conn,
            service,
            path,
            active_interface,
            overwrite_service: false,
        })
    }

    pub async fn get_interfaces(&self) -> Result<Vec<WpaInterface>, ManndError> {
        let mut res: Vec<WpaInterface> = vec![];
        let mut wpa_interfaces: Vec<String> = vec![];
        let mut interfaces = list_interfaces();
        // read list from wpa ifaces and see if ifnames match

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            self.service.clone(),
        )
        .await?;

        let iface_paths: Vec<OwnedObjectPath> = get_prop_from_proxy(&proxy, "Interfaces").await?;
        for path in iface_paths {
            let path_proxy = Proxy::new(
                &self.conn,
                self.service.clone(),
                path,
                "fi.w1.wpa_supplicant1.Interface",
            )
            .await?;
            let ifname: String = get_prop_from_proxy(&path_proxy, "Ifname").await?;
            res.push(WpaInterface::Managed(ifname.clone()));
            wpa_interfaces.push(ifname);
        }
        interfaces.retain(|v| !wpa_interfaces.contains(v));

        interfaces
            .iter()
            .for_each(|v| res.push(WpaInterface::Unmanaged(v.clone())));

        Ok(res)
    }

    pub async fn create_interface(&mut self, ifname: String) -> Result<(), ManndError> {
        let mut body: HashMap<String, OwnedValue> = HashMap::new();
        body.insert("Ifname".to_string(), Value::new(ifname).try_to_owned()?);

        // call create interface
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            self.service.clone(),
        )
        .await?;

        let interface_path: OwnedObjectPath = proxy.call("CreateInterface", &body).await?;
        info!("Interface path: {:?}", interface_path);

        // add args to wpa supplicant service

        // networkd match on name
        self.active_interface = interface_path.clone();

        Ok(())
    }

    pub async fn connect_network_psk(&self, ssid: String, psk: String) -> Result<(), ManndError> {
        let psk_len = psk.len();
        if (psk_len < 8 || psk_len > 63) && psk_len != 0 {
            return Err(ManndError::PasswordLength);
        }
        // let networks = self.networks.get(&ssid).unwrap();

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.active_interface.clone(),
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

        let stream = proxy.receive_signal("PropertiesChanged").await?;
        match self.check_connection(stream).await {
            Err(e) => {
                self.call_interface_method_noreply("RemoveNetwork", network_path)
                    .await?;
                Err(e)
            }
            _ => Ok(()),
        }
    }

    // pub async fn connect_network_eap(&self, ssid: String, eap: EapInfo) -> Result<(), ManndError> {
    //     Ok(())
    // }

    pub async fn disconnect(&self) -> Result<(), ManndError> {
        self.call_interface_method_noreply("Disconnect", &())
            .await?;
        Ok(())
    }

    pub async fn status(&self) -> Result<String, ManndError> {
        todo!()
    }

    pub async fn list_configured_networks(&self) -> Result<Vec<AccessPoint>, ManndError> {
        let networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        let aps: Vec<AccessPoint> = vec![];
        for network in networks {
            let ap = self.get_network_info(network).await?;
        }

        Ok(aps)
    }

    pub async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ManndError> {
        todo!()
    }

    pub async fn scan<'a>(&self, signal_tx: Sender<SignalUpdate<'a>>) -> Result<(), ManndError> {
        if self.get_interface_prop::<bool>("Scanning").await? {
            return Ok(());
        }

        let mut dict: HashMap<String, OwnedValue> = HashMap::new();

        dict.insert("Type".to_string(), Value::new("active").try_to_owned()?);
        self.call_interface_method_noreply("Scan", dict).await?;

        let scan_signal = self.get_interface_signal("ScanDone").await?;
        match signal_tx.send(SignalUpdate::Add(scan_signal)).await {
            Ok(()) => Ok(()),
            Err(_) => Err(ManndError::SignalSend("in wpa_supplicant scan".to_string())),
        }
    }

    pub async fn get_all_networks(&mut self) -> Result<Vec<AccessPoint>, ManndError> {
        // paths with 'Networks' are known but possibly not nearby known networks
        // will also appear in 'BSS' paths
        let nearby_networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("BSSs")
            .await?;

        let mut aps: Vec<AccessPoint> = vec![];
        let mut seen: HashSet<String> = HashSet::new();

        for network in nearby_networks {
            // ssid may appear multiple times if router broadcasts
            // ap at different freqs
            let bss = self.get_bss_info(network.clone()).await?;
            let ssid = bss.ssid.clone();
            let hidden = if ssid.is_empty() || ssid.clone().into_bytes().iter().all(|&v| v == 0) {
                true
            } else {
                false
            };

            if seen.insert(ssid.clone()) && !hidden {
                let ap = AccessPointBuilder::default()
                    .ssid(ssid)
                    .security(bss.security.clone().unwrap())
                    .flags(NetworkFlags::NEARBY)
                    .build()?;

                aps.push(ap);
            }
        }

        let known_networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        for network in known_networks {
            let known = self.get_known_info(network).await?;
            if seen.insert(known.ssid.clone()) {
                let ap = AccessPointBuilder::default()
                    .ssid(known.ssid)
                    .security(Security::Unknown)
                    .flags(NetworkFlags::KNOWN)
                    .build()?;
                aps.push(ap);
            } else {
                if let Some(ap) = aps.iter_mut().find(|ap| ap.ssid == known.ssid) {
                    ap.flags.insert(NetworkFlags::KNOWN);
                    if known.connected == true {
                        ap.flags.insert(NetworkFlags::CONNECTED);
                    }
                }
            }
        }

        Ok(aps)
    }

    pub async fn nearby_networks(&mut self) -> Result<Vec<AccessPoint>, ManndError> {
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
            let ssid = bss.ssid.clone();

            // BUG: not all hidden networks are removed
            if seen.insert(ssid.clone()) && !ssid.is_empty() {
                let ap = AccessPointBuilder::default()
                    .ssid(ssid)
                    .security(bss.security.clone().unwrap())
                    .flags(NetworkFlags::NEARBY)
                    .build()?;

                aps.push(ap);
            }
        }

        Ok(aps)
    }

    /// Used for networks which have already been connected to by wpa supplicant
    pub async fn get_network_info(&self, net_path: OwnedObjectPath) -> Result<WpaBss, ManndError> {
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
    async fn get_bss_info(&self, bss_path: OwnedObjectPath) -> Result<WpaBss, ManndError> {
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

    // Known network, only returns SSID
    async fn get_known_info(
        &self,
        net_path: OwnedObjectPath,
    ) -> Result<WpaKnownNetwork, ManndError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            net_path,
            "fi.w1.wpa_supplicant1.Network",
        )
        .await?;
        let properties =
            get_prop_from_proxy::<HashMap<String, Value>>(&proxy, "Properties").await?;

        let mut res = WpaKnownNetwork {
            ssid: String::new(),
            connected: false,
        };

        if let Some(ssid) = properties.get("ssid") {
            match ssid {
                Value::Str(s) => {
                    let clean = s.as_str().trim_matches('"').to_string();
                    res.ssid = clean;
                }
                _ => return Err(ManndError::NetworkNotFound),
            }
        } else {
            return Err(ManndError::NetworkNotFound);
        };

        let connected = get_prop_from_proxy::<bool>(&proxy, "Enabled").await?;
        res.connected = connected;
        Ok(res)
    }
}

// Helper functions
impl WpaSupplicant {
    async fn call_interface_method<T, U>(
        &self,
        method_name: &'static str,
        body: T,
    ) -> Result<U, ManndError>
    where
        T: serde::ser::Serialize + zvariant::DynamicType,
        U: for<'a> zvariant::DynamicDeserialize<'a>,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.active_interface.clone(),
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
    ) -> Result<(), ManndError>
    where
        T: serde::ser::Serialize + zvariant::DynamicType,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.active_interface.clone(),
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

    async fn check_connection<'a>(&self, mut stream: SignalStream<'a>) -> Result<(), ManndError> {
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
                                    return Err(ManndError::ConnectionFailed(
                                        "WPA rejected network request, check password".into(),
                                    ));
                                }
                            }
                            "inactive" => {
                                return Err(ManndError::ConnectionFailed(
                                    "Interface is inactive!".into(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                Ok(None) => {
                    return Err(ManndError::OperationFailed(
                        "DBus stream ended unexpectedly.".into(),
                    ));
                }
                Err(_) => {
                    if start.elapsed() > max_wait {
                        return Err(ManndError::Timeout);
                    }
                }
            }
        }
    }

    async fn get_interface_prop<'a, T>(&self, prop: &'static str) -> Result<T, ManndError>
    where
        T: TryFrom<Value<'a>>,
        <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.active_interface.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;
        return Ok(get_prop_from_proxy::<T>(&proxy, prop).await?);
    }

    async fn get_interface_signal<'a, M>(
        &self,
        signal_name: M,
    ) -> Result<SignalStream<'a>, ManndError>
    where
        M: TryInto<MemberName<'a>>,
        M::Error: Into<zbus::Error>,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.active_interface.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;

        let stream = proxy.receive_signal(signal_name).await?;
        Ok(stream)
    }

    fn get_env_file() -> PathBuf {
        STATE_HOME.0.join("ManndEnvFile")
    }

    async fn should_edit_service(&self) -> bool {
        let service_path = PathBuf::from(get_service_path(&self.conn, "wpa_supplicant").await);
        let Ok(mut conf) = IniConfig::new(service_path) else { return false };
        let Some(service_sect) = conf.sections.get_mut(&"Service".to_string()) else { return false };
        let env_file = Self::get_env_file();
        match service_sect.get("EnvironmentFile") {
            // file already has env file
            Some(val) => {
                if val == &env_file {
                    return true;
                }
            }
            None => {}
        };

        // write env file
        service_sect.insert(
            "EnvironmentFile".to_string(),
            env_file.into_os_string().into_string().unwrap(),
        );
        conf.write_file();
        true
    }
}

#[cfg(wpa_installed)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wpa_scan() -> Result<(), ManndError> {
        let conn = Connection::system().await.unwrap();
        let mut wpa = WpaSupplicant::new(conn)?;
        let _ = wpa.get_interface_prop::<Vec<u8>>("MACAddress").await?;
        wpa.list_configured_networks().await?;

        let _ = wpa.nearby_networks().await?;

        Ok(())
    }
}
