//! [Reference](https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network)
//!
//! The `WpaBss` struct has a lot of optional types mainly because it differs significantly to what's provided from a scan vs a connected network.
//!
//! When getting networks after a scan there can be duplicate SSIDs because `wpa_supplicant` shows the different possible freqs.

use std::{
    collections::{HashMap, HashSet, btree_map::Entry},
    fmt::Debug,
    fs::{File, OpenOptions},
    path::PathBuf,
    time::{Duration, Instant},
};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc::Sender, time::timeout};
use tracing::{info, instrument};
use zbus::{
    Connection, Proxy,
    names::MemberName,
    proxy::SignalStream,
    zvariant::{self, NoneValue, OwnedObjectPath, OwnedValue, Value},
};

use crate::{
    SETTINGS,
    error::ManndError,
    ini_parse::IniConfig,
    state::signals::SignalUpdate,
    systemd::systemctl::get_service_path,
    utils::list_interfaces,
    wireless::common::{
        AccessPoint, AccessPointBuilder, NetworkFlags, Security, get_prop_from_proxy,
    },
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {
    service: String,
    path: String,
    conn: Connection,
    // only one interface dealing with Wi-Fi
    // at a time
    active_interface: OwnedObjectPath,
    // if false won't edit .service files
    // BUG: persist is defined in two
    // places (here and app_ctx), should be
    // in sync but find better way to do this
    persist: bool,
    // has prepare_wpa_persist been called?
    persist_files_prepared: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WpaInterface {
    Managed(String),
    Unmanaged(String),
}

impl<'a> From<&'a WpaInterface> for String {
    fn from(value: &'a WpaInterface) -> Self {
        match value {
            WpaInterface::Managed(v) | WpaInterface::Unmanaged(v) => v.clone(),
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

// Public functions
impl WpaSupplicant {
    #[instrument(err, skip(conn))]
    pub async fn new(conn: Connection) -> Result<Self, ManndError> {
        let service = String::from("fi.w1.wpa_supplicant1");
        let path = String::from("/fi/w1/wpa_supplicant1");
        let proxy = Proxy::new(&conn, service.clone(), path.clone(), service.clone()).await?;

        let mut active_interfaces =
            get_prop_from_proxy::<Vec<OwnedObjectPath>>(&proxy, "Interfaces").await?;
        let active_interface: OwnedObjectPath = if active_interfaces.is_empty() {
            OwnedObjectPath::null_value()
        } else {
            active_interfaces.swap_remove(0)
        };

        Ok(Self {
            conn,
            service,
            path,
            active_interface,
            persist: false,
            persist_files_prepared: false,
        })
    }

    #[instrument(err, skip(self))]
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

        for v in &interfaces {
            res.push(WpaInterface::Unmanaged(v.clone()));
        }

        Ok(res)
    }

    #[instrument(err, skip(self))]
    pub async fn connect_network_psk(&self, ssid: &str, psk: &String) -> Result<(), ManndError> {
        let psk_len = psk.len();
        if !(8..=63).contains(&psk_len) && psk_len != 0 {
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

    #[instrument(err, skip(self))]
    pub async fn connect_known(&self, ssid: &str) -> Result<(), ManndError> {
        let networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;
        for network in networks {
            if self.get_known_info(&network).await? == ssid {
                self.call_interface_method_noreply("SelectNetwork", network)
                    .await?;
            }
        }

        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn disconnect(&self) -> Result<(), ManndError> {
        self.call_interface_method_noreply("Disconnect", &())
            .await?;
        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn status(&self) -> Result<String, ManndError> {
        todo!()
    }

    #[instrument(err, skip(self))]
    pub async fn remove_network(&self, ssid: &str, security: &Security) -> Result<(), ManndError> {
        let known = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        for network in known {
            let cur_ssid = self.get_known_info(&network).await?;
            if cur_ssid == ssid {
                self.call_interface_method_noreply("RemoveNetwork", network)
                    .await?;
            }
        }
        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn scan(&self, signal_tx: Sender<SignalUpdate<'_>>) -> Result<(), ManndError> {
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

    #[instrument(err, skip(self))]
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
            let hidden = ssid.is_empty() || ssid.clone().into_bytes().iter().all(|&v| v == 0);

            if seen.insert(ssid.clone()) && !hidden {
                let ap = AccessPointBuilder::default()
                    .ssid(ssid)
                    .security(bss.security.clone().unwrap())
                    .flags(NetworkFlags::NEARBY)
                    .build()?;

                aps.push(ap);
            }
        }

        // due to freq issue mentioned above we go for network not bss
        let conn_net = self
            .get_interface_prop::<OwnedObjectPath>("CurrentNetwork")
            .await?;

        let known_networks = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        for network in known_networks {
            let known_ssid = self.get_known_info(&network).await?;
            if seen.insert(known_ssid.clone()) {
                let ap = AccessPointBuilder::default()
                    .ssid(known_ssid)
                    .security(Security::Unknown)
                    .flags(NetworkFlags::KNOWN)
                    .build()?;
                aps.push(ap);
            } else if let Some(ap) = aps.iter_mut().find(|ap| ap.ssid == known_ssid) {
                ap.flags.insert(NetworkFlags::KNOWN);
                if network == conn_net {
                    ap.flags.insert(NetworkFlags::CONNECTED);
                }
            }
        }

        Ok(aps)
    }

    pub const fn toggle_persist(&mut self) {
        self.persist = !self.persist;
    }
}

// functions which need to read/write data
impl WpaSupplicant {
    /// Modifies the wpa_supplicant systemd service as needed
    /// to allow for changes to persist through an env file
    #[instrument(err, skip(self))]
    async fn prepare_wpa_persist(&mut self) -> Result<(), ManndError> {
        let service_path = PathBuf::from(get_service_path(&self.conn, "wpa_supplicant").await);
        let Ok(mut conf) = IniConfig::new(service_path) else {
            return Err(ManndError::OperationFailed(
                "Could not find wpa_supplicant file or parse as INI".into(),
            ));
        };

        let Some(service_sect) = conf.sections.get_mut("Service") else {
            return Err(ManndError::SectionNotFound(
                "In wpa_supplicant service file, [Service] could not be located".into(),
            ));
        };

        let env_file = Self::get_env_file();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&env_file)?;

        let env_as_str = env_file.into_os_string().into_string().unwrap();

        match service_sect.entry("EnvironmentFile".into()) {
            Entry::Occupied(mut entry) => {
                if !entry.get().to_lowercase().contains(&env_as_str) {
                    entry.insert(env_as_str);
                    conf.overwrite()?;
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(env_as_str);
                conf.overwrite()?;
            }
        }

        self.persist_files_prepared = true;
        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn create_interface(&mut self, ifname: &str) -> Result<(), ManndError> {
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

        if self.persist {
            if !self.persist_files_prepared {
                self.prepare_wpa_persist().await?;
            }
        } else {
            let interface_path: OwnedObjectPath = proxy.call("CreateInterface", &body).await?;
            self.active_interface = interface_path;
        }

        Ok(())
    }
}

// Helper functions
impl WpaSupplicant {
    #[instrument(err, skip(self))]
    async fn call_interface_method<T, U>(
        &self,
        method_name: &'static str,
        body: T,
    ) -> Result<U, ManndError>
    where
        T: serde::ser::Serialize + zvariant::DynamicType + Debug + Send + Sync,
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

    #[instrument(err, skip(self))]
    async fn call_interface_method_noreply<T>(
        &self,
        method_name: &'static str,
        body: T,
    ) -> Result<(), ManndError>
    where
        T: serde::ser::Serialize + zvariant::DynamicType + Debug + Send + Sync,
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

    fn get_security(rsn: &HashMap<String, Value<'_>>) -> Security {
        let mut security = Security::Open;

        // eap and psk are exclusive
        // so only need to check one if it contains
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

    #[instrument(err, skip(self))]
    async fn check_connection(&self, mut stream: SignalStream<'_>) -> Result<(), ManndError> {
        let start = Instant::now();
        let max_wait = Duration::from_secs(15);

        // If first connected to a network then expect a disconnected first
        // SUCCESS: authenticating -> associating -> 4-way-handshake -> completed
        // FAILURE: associating -> 4-way-handshake -> disconnected (incorrect password)
        // FAILURE: scanning -> scanning -> scanning -> [ad inf.] (can't find network)
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
                                // since success also uses disconnected check
                                // how long we've been going first
                                // TODO: look into lowering this
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

    #[instrument(err)]
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
        get_prop_from_proxy::<T>(&proxy, prop).await
    }

    #[instrument(err, skip(self))]
    async fn get_interface_signal<'a, M>(
        &self,
        signal_name: M,
    ) -> Result<SignalStream<'a>, ManndError>
    where
        M: TryInto<MemberName<'a>> + Debug,
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
        PathBuf::from(SETTINGS.get("storage", "state").unwrap()).join("ManndWpaEnv")
    }

    // check if the wpa_supplicant service file
    // has EnvironmentFile='...' set to the mannd
    // environment file
    async fn check_service_for_env(&self) -> bool {
        let service_path = PathBuf::from(get_service_path(&self.conn, "wpa_supplicant").await);
        let Ok(mut conf) = IniConfig::new(service_path) else {
            return false;
        };
        let Some(service_sect) = conf.sections.get_mut("Service") else {
            return false;
        };
        let env_file = Self::get_env_file();
        if let Some(val) = service_sect.get("EnvironmentFile") {
            // file already has env file
            if val.to_lowercase().contains("mannd") && File::open(&env_file).is_ok() {
                return true;
            }
        }

        false
    }

    async fn should_edit_service(&self) -> bool {
        let service_path = PathBuf::from(get_service_path(&self.conn, "wpa_supplicant").await);
        let Ok(mut conf) = IniConfig::new(service_path) else {
            return false;
        };

        let Some(service_sect) = conf.sections.get_mut("Service") else {
            return false;
        };

        let env_file = Self::get_env_file();
        if let Some(val) = service_sect.get("EnvironmentFile")
            && val == &env_file
        {
            return true;
        }

        // write env file
        service_sect.insert(
            "EnvironmentFile".to_string(),
            env_file.into_os_string().into_string().unwrap(),
        );
        let _ = conf.write_file(None);
        true
    }

    /// Used for networks which are nearby by may
    /// not have been connected to yet
    #[instrument(err, skip(self))]
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
        let security = Some(Self::get_security(&rsn));

        Ok(WpaBss {
            ssid,
            bssid,
            security,
            freq,
            rates,
        })
    }

    // Known network, only returns SSID
    #[instrument(err, skip(self))]
    async fn get_known_info(&self, net_path: &OwnedObjectPath) -> Result<String, ManndError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            net_path,
            "fi.w1.wpa_supplicant1.Network",
        )
        .await?;
        let properties =
            get_prop_from_proxy::<HashMap<String, Value>>(&proxy, "Properties").await?;

        if let Some(ssid) = properties.get("ssid") {
            match ssid {
                Value::Str(s) => {
                    let clean = s.as_str().trim_matches('"').to_string();
                    return Ok(clean);
                }
                _ => return Err(ManndError::NetworkNotFound),
            }
        }
        Err(ManndError::NetworkNotFound)
    }

    /// Used for known networks
    #[instrument(err, skip(self))]
    async fn get_network_info(&self, net_path: OwnedObjectPath) -> Result<WpaBss, ManndError> {
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
}

#[cfg(wpa_installed)]
mod tests {
    use super::*;

    #[tokio::test]
    #[instrument(err)]
    async fn test_wpa_scan() -> Result<(), ManndError> {
        let conn = Connection::system().await.unwrap();
        let mut wpa = WpaSupplicant::new(conn).await?;
        let _ = wpa.get_interface_prop::<Vec<u8>>("MACAddress").await?;
        wpa.get_all_networks().await?;

        Ok(())
    }
}
