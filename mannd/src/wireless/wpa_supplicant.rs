//! [Reference](https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network)
//!
//! The `WpaBss` struct has a lot of optional types mainly because it differs significantly to what's provided from a scan vs a connected network.
//!
//! When getting networks after a scan there can be duplicate SSIDs because `wpa_supplicant` shows the different possible freqs.

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
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
    zvariant::{self, OwnedObjectPath, OwnedValue, Value},
};

use crate::{
    context,
    error::ManndError,
    read_global,
    state::signals::SignalUpdate,
    store::WpaState,
    utils::list_interfaces,
    wireless::{
        common::{AccessPoint, AccessPointBuilder, NetworkFlags, Security, get_prop_from_proxy},
        wpa_config::{ApplyScope, WpaAutoscan, WpaConfig},
    },
};

#[derive(Debug)]
pub struct WpaSupplicant {
    // persistant state
    config: WpaConfig,
    state: WpaState,

    service: String,
    path: String,
    conn: Connection,
    // this resets each time
    persist: bool,
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
    pub async fn new(
        config: WpaConfig,
        state: WpaState,
        conn: Connection,
    ) -> Result<Self, ManndError> {
        let service = String::from("fi.w1.wpa_supplicant1");
        let path = String::from("/fi/w1/wpa_supplicant1");

        let mut wpa = Self {
            config,
            state,
            conn,
            service,
            path,
            persist: false,
        };

        wpa.sync_interface_state().await?;
        wpa.apply_global_policy().await?;

        Ok(wpa)
    }

    #[instrument(err, skip(self))]
    pub async fn get_interfaces(&self) -> Result<Vec<WpaInterface>, ManndError> {
        let mut wpa_interfaces: Vec<WpaInterface> = vec![];
        // likely not needed
        let phys_interfaces = list_interfaces();

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            self.service.clone(),
        )
        .await?;

        let mut iface_paths: Vec<OwnedObjectPath> =
            get_prop_from_proxy(&proxy, "Interfaces").await?;
        for phy in phys_interfaces {
            if iface_paths.is_empty() {
                wpa_interfaces.push(WpaInterface::Unmanaged(phy));
            } else {
                if let Some(path) = &iface_paths.pop() {
                    let path_proxy = Proxy::new(
                        &self.conn,
                        self.service.clone(),
                        path,
                        "fi.w1.wpa_supplicant1.Interface",
                    )
                    .await?;
                    let ifname: String = get_prop_from_proxy(&path_proxy, "Ifname").await?;
                    wpa_interfaces.push(WpaInterface::Managed(ManagedInterface::new(
                        ifname,
                        path.clone(),
                    )));
                }
            }
        }

        Ok(wpa_interfaces)
    }

    #[instrument(err, skip(self))]
    pub async fn connect_network_psk(&self, ssid: &str, psk: &String) -> Result<(), ManndError> {
        let psk_len = psk.len();
        if !(8..=63).contains(&psk_len) && psk_len != 0 {
            return Err(ManndError::PasswordLength);
        }

        let Some(interface) = &self.state.active_interface else {
            return Err(ManndError::WpaNoInterfaces);
        };

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            interface.path.clone(),
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

        body.insert("freq_list".into(), Value::new("5180 5200 5220 5240 5260 5280 5300 5320 5500 5520 5540 5560 5580 5600 5620 5640 5660 5680 5700 5720 5745 5765 5785 5805 5825").try_to_owned()?);
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

// Helper functions
impl WpaSupplicant {
    /// Initialises what it can from the last recorded state
    async fn sync_interface_state(&mut self) -> Result<(), ManndError> {
        let cur_ifaces = self.get_interfaces().await?;
        let mut runtime_managed: Vec<ManagedInterface> = Vec::new();
        let mut managed_now: HashMap<String, ManagedInterface> = HashMap::new();
        let mut unmanaged_now: HashSet<String> = HashSet::new();

        for iface in cur_ifaces {
            match iface {
                WpaInterface::Managed(mi) => {
                    managed_now.insert(mi.name.clone(), mi);
                }
                WpaInterface::Unmanaged(name) => {
                    unmanaged_now.insert(name);
                }
            }
        }

        for ifname in self.state.desidred_interfaces.clone() {
            if let Some(mi) = managed_now.get(&ifname) {
                runtime_managed.push(mi.clone());
                continue;
            }

            if unmanaged_now.contains(&ifname) {
                let created = self.create_interface_runtime(&ifname).await?;
                runtime_managed.push(created);
            }
        }

        for (_name, mi) in managed_now {
            if !runtime_managed.iter().any(|x| x.name == mi.name) {
                runtime_managed.push(mi);
            }
        }

        self.state.managed_interfaces = runtime_managed;

        let pref_name = self.config.interfaces.preferred_interface.clone();
        let active_name = self.state.active_interface.as_ref().map(|m| m.name.clone());

        self.state.active_interface =
            self.choose_active_interface(pref_name.as_deref(), active_name.as_deref());

        self.write_state()?;
        Ok(())
    }

    /// Chooses pref if available, if not then prev if available if not
    /// then first managed interface nearby.
    fn choose_active_interface(
        &self,
        pref: Option<&str>,
        prev: Option<&str>,
    ) -> Option<ManagedInterface> {
        if let Some(name) = pref {
            if let Some(mi) = self
                .state
                .managed_interfaces
                .iter()
                .find(|m| m.name == name)
            {
                return Some(mi.clone());
            }
        }

        if let Some(name) = prev {
            if let Some(mi) = self
                .state
                .managed_interfaces
                .iter()
                .find(|m| m.name == name)
            {
                return Some(mi.clone());
            }
        }

        self.state.managed_interfaces.first().cloned()
    }

    #[instrument(err, skip(self))]
    async fn apply_global_policy(&self) -> Result<(), ManndError> {
        let policy = &self.config.policy;

        let managed: Vec<ManagedInterface> = self
            .get_interfaces()
            .await?
            .into_iter()
            .filter_map(|iface| match iface {
                WpaInterface::Managed(m) => Some(m),
                WpaInterface::Unmanaged(_) => None,
            })
            .collect();

        let targets: Vec<ManagedInterface> = match &policy.apply_scope {
            ApplyScope::AllInterfaces => managed,
            ApplyScope::Interfaces(names) => managed
                .into_iter()
                .filter(|m| names.iter().any(|n| n == &m.name))
                .collect(),
        };

        for iface in targets {
            self.apply_global_policy_interface(&iface).await?;
        }
        Ok(())
    }

    async fn apply_global_policy_interface(
        &self,
        iface: &ManagedInterface,
    ) -> Result<(), ManndError> {
        let policy = &self.config.policy;
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            iface.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;

        if let Some(country) = &policy.country {
            proxy.set_property("Country", country.clone()).await?
        }

        proxy.set_property("FastReauth", policy.fast_reauth).await?;

        if let Some(scan_interval) = policy.scan_interval_sec {
            let interval_i32 = i32::try_from(scan_interval)?;
            proxy.set_property("ScanInterval", interval_i32).await?;
        }

        let autoscan_type = match &policy.autoscan {
            WpaAutoscan::Disabled => String::new(),
            WpaAutoscan::Exponential { base, limit } => {
                format!("exponential:{base}:{limit}")
            }
            WpaAutoscan::Periodic { interval } => format!("periodic:{interval}"),
        };
        proxy.call_noreply("AutoScan", &autoscan_type).await?;

        Ok(())
    }

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
        let Some(interface) = &self.state.active_interface else {
            return Err(ManndError::WpaNoInterfaces);
        };

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            interface.path.clone(),
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
        let Some(interface) = &self.state.active_interface else {
            return Err(ManndError::WpaNoInterfaces);
        };

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            interface.path.clone(),
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
        let Some(interface) = &self.state.active_interface else {
            return Err(ManndError::WpaNoInterfaces);
        };

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            interface.path.clone(),
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
        let Some(interface) = &self.state.active_interface else {
            return Err(ManndError::WpaNoInterfaces);
        };

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            interface.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;

        let stream = proxy.receive_signal(signal_name).await?;
        Ok(stream)
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

    /// Returns a Path to where the interface config file should be made/found
    fn interface_config_file(ifname: &str) -> Result<PathBuf, ManndError> {
        let settings = &context().settings;
        let mut home = PathBuf::from(&settings.storage.state);
        home.push(format!("mannd/wpa/{ifname}.conf"));
        Ok(home)
    }

    async fn create_interface_runtime(&self, ifname: &str) -> Result<ManagedInterface, ManndError> {
        let mut body: HashMap<String, OwnedValue> = HashMap::new();
        body.insert("Ifname".into(), Value::from(ifname).try_to_owned()?);

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            self.service.clone(),
        )
        .await?;

        let interface_path: OwnedObjectPath = proxy.call("CreateInterface", &body).await?;
        Ok(ManagedInterface::new(ifname.into(), interface_path))
    }

    #[instrument(err, skip(self))]
    pub async fn create_interface(&mut self, ifname: &str) -> Result<(), ManndError> {
        let interface = self.create_interface_runtime(ifname).await?;
        // find matching existing interface and reassign
        if let Some(existing) = self
            .state
            .managed_interfaces
            .iter_mut()
            .find(|m| m.name == interface.name)
        {
            *existing = interface.clone();
        } else {
            self.state.managed_interfaces.push(interface.clone());
        }

        if self.persist && !self.state.desidred_interfaces.iter().any(|n| n == ifname) {
            self.state.desidred_interfaces.push(ifname.to_string());
        }

        self.state.active_interface = Some(interface);
        self.write_state()?;

        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn remove_interface(&mut self, ifname: &str) -> Result<(), ManndError> {
        if self.state.managed_interfaces.is_empty() {
            return Err(ManndError::WpaRemoveEmpty);
        }

        let Some(interface) = self
            .state
            .managed_interfaces
            .iter()
            .find(|iface| iface.name == ifname)
            .cloned()
        else {
            return Err(ManndError::WpaRemoveNotFound);
        };

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            self.service.clone(),
        )
        .await?;

        proxy
            .call_noreply("RemoveInterface", &(interface.path.clone()))
            .await?;

        self.state
            .managed_interfaces
            .retain(|iface| iface.name != ifname);

        let removed_active = self
            .state
            .active_interface
            .as_ref()
            .is_some_and(|iface| iface.name == ifname);

        if removed_active {
            let pref_name = self.config.interfaces.preferred_interface.clone();
            self.state.active_interface = self.choose_active_interface(pref_name.as_deref(), None);
        }

        self.write_state()?;
        Ok(())
    }

    #[instrument(err, skip(self))]
    fn write_state(&self) -> Result<(), ManndError> {
        read_global(|state| state.db.write_wpa_state(&self.state)).transpose()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManagedInterface {
    name: String,
    path: OwnedObjectPath,
}

impl ManagedInterface {
    fn new(name: String, path: OwnedObjectPath) -> Self {
        Self { name, path }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WpaInterface {
    Managed(ManagedInterface),
    Unmanaged(String),
}

impl WpaInterface {
    pub fn name(&self) -> &str {
        match self {
            Self::Managed(interface) => &interface.name,
            Self::Unmanaged(name) => name,
        }
    }

    pub const fn is_managed(&self) -> bool {
        matches!(self, Self::Managed(_))
    }
}
