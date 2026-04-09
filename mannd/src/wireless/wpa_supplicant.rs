//! [Reference](https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network)
//!
//! The `WpaBss` struct has a lot of optional types mainly because it differs significantly to what's provided from a scan vs a connected network.
//!
//! When getting networks after a scan there can be duplicate SSIDs because `wpa_supplicant` shows the different possible freqs.

use std::{
    cmp::Ordering,
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
    zvariant::{self, ObjectPath, OwnedObjectPath, OwnedValue, Value},
};

use crate::{
    context,
    error::ManndError,
    modify_global, read_global,
    state::signals::SignalUpdate,
    store::{NetworkInfo, NetworkInfoBuilder, NetworkSecurity, WpaState},
    utils::{list_interfaces, validate_network},
    wireless::{
        common::{NetworkFlags, get_prop_from_proxy},
        wpa_config::{ApplyScope, WpaAutoscan, WpaConfig, WpaUiSort},
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
    security: Option<NetworkSecurity>,
    signal_dbm: Option<i16>,
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
        let phys_interfaces = list_interfaces();

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            self.service.clone(),
        )
        .await?;

        let iface_paths: Vec<OwnedObjectPath> = get_prop_from_proxy(&proxy, "Interfaces").await?;
        let mut managed_by_name: HashMap<String, ManagedInterface> = HashMap::new();

        for path in iface_paths {
            let iface_proxy = self.get_interface_proxy(path.clone()).await?;
            let ifname: String = get_prop_from_proxy(&iface_proxy, "Ifname").await?;
            managed_by_name.insert(ifname.clone(), ManagedInterface::new(ifname, path));
        }

        for phy in phys_interfaces {
            if let Some(managed) = managed_by_name.remove(&phy) {
                wpa_interfaces.push(WpaInterface::Managed(managed));
            } else {
                wpa_interfaces.push(WpaInterface::Unmanaged(phy));
            }
        }

        for (_name, managed) in managed_by_name {
            wpa_interfaces.push(WpaInterface::Managed(managed));
        }

        Ok(wpa_interfaces)
    }

    /// Checks for nearby networks, if a network has been connected to previously
    /// and is nearby adds that network. Updates the global state. This should only
    /// run after a scan operation has completed
    #[instrument(err, skip(self))]
    pub async fn get_networks(&self) -> Result<Vec<NetworkInfo>, ManndError> {
        let nearby = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("BSSs")
            .await?;

        if nearby.is_empty() {
            return Ok(vec![]);
        }

        let saved_networks =
            read_global(|state| state.app.saved_networks.clone()).unwrap_or_default();

        let mut by_ssid: HashMap<String, NetworkInfo> = saved_networks
            .into_iter()
            .map(|n| (n.ssid.clone(), n))
            .collect();

        // reset flags
        for net in by_ssid.values_mut() {
            net.flags
                .remove(NetworkFlags::NEARBY | NetworkFlags::CONNECTED | NetworkFlags::KNOWN);
            net.signal_dbm = None;
        }

        let cur_network = self
            .get_interface_prop::<OwnedObjectPath>("CurrentNetwork")
            .await
            .ok();

        let known_paths = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await
            .unwrap_or_default();

        for path in known_paths {
            let Ok(ssid) = self.get_known_ssid(&path).await else {
                continue;
            };
            by_ssid
                .entry(ssid.clone())
                .and_modify(|n| {
                    n.flags.insert(NetworkFlags::KNOWN);
                    if cur_network.as_ref().is_some_and(|cur| cur == &path) {
                        n.flags.insert(NetworkFlags::CONNECTED);
                    }
                })
                .or_insert_with(|| {
                    let mut flags = NetworkFlags::KNOWN;
                    if cur_network.as_ref().is_some_and(|cur| cur == &path) {
                        flags.insert(NetworkFlags::CONNECTED);
                    }
                    NetworkInfoBuilder::default()
                        .ssid(ssid)
                        .security(NetworkSecurity::Open)
                        .flags(flags)
                        .build()
                        .expect("BSS builder shouldn't fail")
                });
        }

        let nearby = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("BSSs")
            .await
            .unwrap_or_default();
        self.merge_nearby_bss(nearby, &mut by_ssid).await;

        let mut networks: Vec<NetworkInfo> = by_ssid.into_values().collect();
        self.sort_networks(&mut networks);
        Ok(networks)
    }

    #[instrument(err, skip(self))]
    pub async fn connect_network(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        let network_path = self.add_network(network).await?;
        self.call_interface_method_noreply("SelectNetwork", network_path.clone())
            .await?;

        // connect is a decently unlikely event so this is fine
        modify_global(|state| {
            if let Some(existing) = state
                .app
                .saved_networks
                .iter_mut()
                .find(|n| n.ssid == network.ssid)
            {
                *existing = network.clone();
            } else {
                state.app.saved_networks.push(network.clone());
            }
        });

        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn connect_known(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        let network_paths = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        for path in network_paths {
            if self.get_known_ssid(&path).await? == network.ssid {
                self.call_interface_method_noreply("SelectNetwork", path)
                    .await?;
                return Ok(());
            }
        }

        Err(ManndError::NetworkNotFound)
    }

    /// Adds the networks that have historically been connected to, to the wpa_supplicant1 dbus
    /// so they can be connected to.
    #[instrument(err, skip(self))]
    pub async fn add_from_list(&self, networks: &mut Vec<NetworkInfo>) -> Result<(), ManndError> {
        // if connected or no networks, skip
        if self.is_connected().await? || networks.is_empty() {
            return Ok(());
        }

        let Some(known) = read_global(|state| state.app.saved_networks.clone()) else {
            return Ok(());
        };

        for known_net in known {
            if let Some(near_net) = networks.iter_mut().find(|n| n.ssid == known_net.ssid) {
                self.add_network(&known_net).await?;
                near_net.flags |= NetworkFlags::KNOWN;
            }
        }

        Ok(())
    }

    /// Does not update the NetworkFlags of the disconnected network
    /// instead it is expected that get_networks is called
    #[instrument(err, skip(self))]
    pub async fn disconnect(&self) -> Result<(), ManndError> {
        self.call_interface_method_noreply("Disconnect", &())
            .await?;

        Ok(())
    }

    #[instrument(err, skip(self))]
    pub async fn remove_network(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        let known = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
            .await?;

        for net_path in known {
            let cur_ssid = self.get_known_ssid(&net_path).await?;
            if cur_ssid == network.ssid {
                self.call_interface_method_noreply("RemoveNetwork", net_path)
                    .await?;
            }
        }

        modify_global(|state| {
            state.app.saved_networks.retain(|n| n.ssid != network.ssid);
        });

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

    pub const fn toggle_persist(&mut self) {
        self.persist = !self.persist;
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
}

// Helper functions
impl WpaSupplicant {
    async fn active_interface_proxy(&self) -> Result<Proxy<'_>, ManndError> {
        let Some(interface) = &self.state.active_interface else {
            return Err(ManndError::WpaNoInterfaces);
        };

        Ok(Proxy::new(
            &self.conn,
            self.service.clone(),
            interface.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?)
    }

    async fn is_connected(&self) -> Result<bool, ManndError> {
        let cur_network = self
            .get_interface_prop::<OwnedObjectPath>("CurrentNetwork")
            .await?;

        Ok(!cur_network.eq(&OwnedObjectPath::from(ObjectPath::from_str_unchecked("/"))))
    }

    #[instrument(err, skip(self))]
    pub async fn status(&self) -> Result<String, ManndError> {
        self.get_interface_prop::<String>("State").await
    }

    /// Initialises interfaces from last state if they are present
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

        for ifname in self.state.desired_interfaces.clone() {
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
        [pref, prev]
            .into_iter()
            .flatten()
            .find_map(|name| {
                self.state
                    .managed_interfaces
                    .iter()
                    .find(|m| m.name == name)
                    .cloned()
            })
            .or_else(|| self.state.managed_interfaces.first().cloned())
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
        let proxy = self.get_interface_proxy(iface.path.clone()).await?;

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

        if self.persist && !self.state.desired_interfaces.iter().any(|n| n == ifname) {
            self.state.desired_interfaces.push(ifname.to_string());
        }

        self.state.active_interface = Some(interface);
        self.write_state()?;

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
        let proxy = self.active_interface_proxy().await?;
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
        let proxy = self.active_interface_proxy().await?;
        proxy.call_noreply(method_name, &body).await?;
        Ok(())
    }

    #[instrument(err, skip(self))]
    fn write_state(&self) -> Result<(), ManndError> {
        read_global(|state| state.db.write_wpa_state(&self.state)).transpose()?;
        Ok(())
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

    // Known network, only returns SSID
    #[instrument(err, skip(self))]
    async fn get_known_ssid(&self, net_path: &OwnedObjectPath) -> Result<String, ManndError> {
        let proxy = self.get_network_proxy(net_path.clone()).await?;
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

        let signal_dbm = get_prop_from_proxy::<i16>(&proxy, "Signal").await.ok();
        let freq = Some(get_prop_from_proxy::<u16>(&proxy, "Frequency").await?);
        let rates = Some(get_prop_from_proxy::<Vec<u32>>(&proxy, "Rates").await?);

        let rsn = get_prop_from_proxy::<HashMap<String, Value>>(&proxy, "RSN").await?;
        let security = Some(Self::get_security(&rsn));

        Ok(WpaBss {
            ssid,
            bssid,
            security,
            signal_dbm,
            freq,
            rates,
        })
    }

    async fn merge_nearby_bss(
        &self,
        nearby: Vec<OwnedObjectPath>,
        by_ssid: &mut HashMap<String, NetworkInfo>,
    ) {
        for bss_path in nearby {
            let Ok(bss) = self.get_bss_info(bss_path).await else {
                continue;
            };

            if bss.ssid.is_empty() || bss.ssid.as_bytes().iter().all(|&b| b == 0) {
                continue;
            }

            let ssid = bss.ssid.clone();
            let bss_security = bss.security.clone();
            let bss_signal = bss.signal_dbm;

            by_ssid
                .entry(ssid.clone())
                .and_modify(|n| {
                    n.flags.insert(NetworkFlags::NEARBY);
                    if let Some(sec) = &bss_security {
                        n.security = sec.clone();
                    }
                    n.signal_dbm = match (n.signal_dbm, bss_signal) {
                        (Some(cur), Some(new)) => Some(cur.max(new)),
                        (Some(cur), None) => Some(cur),
                        (None, Some(new)) => Some(new),
                        (None, None) => None,
                    };
                })
                .or_insert_with(|| {
                    NetworkInfoBuilder::default()
                        .ssid(ssid)
                        .security(bss_security.unwrap_or(NetworkSecurity::Open))
                        .signal_dbm(bss_signal)
                        .flags(NetworkFlags::NEARBY)
                        .build()
                        .expect("BSS builder shouldn't fail")
                });
        }
    }

    // #[instrument(err, skip(self))]
    // async fn check_connection(&self, mut stream: SignalStream<'_>) -> Result<(), ManndError> {
    //     let start = Instant::now();
    //     let max_wait = Duration::from_secs(15);
    //
    //     // If first connected to a network then expect a disconnected first
    //     // SUCCESS: authenticating -> associating -> 4-way-handshake -> completed
    //     // FAILURE: associating -> 4-way-handshake -> disconnected (incorrect password)
    //     // FAILURE: scanning -> scanning -> scanning -> [ad inf.] (can't find network)
    //     loop {
    //         match timeout(Duration::from_secs(1), stream.next()).await {
    //             Ok(Some(msg)) => {
    //                 let res: HashMap<String, OwnedValue> = msg.body().deserialize()?;
    //                 if let Some(state) = res.get("State") {
    //                     let state_str = state.downcast_ref::<&str>().unwrap_or("Unknown");
    //                     info!("WPA STATE: {}", state_str);
    //                     match state_str {
    //                         "completed" => {
    //                             info!("Connected successfully!");
    //                             return Ok(());
    //                         }
    //                         "disconnected" => {
    //                             // since success also uses disconnected check
    //                             // how long we've been going first
    //                             // TODO: look into lowering this
    //                             if start.elapsed().as_secs() > 2 {
    //                                 return Err(ManndError::ConnectionFailed(
    //                                     "WPA rejected network request, check password".into(),
    //                                 ));
    //                             }
    //                         }
    //                         "inactive" => {
    //                             return Err(ManndError::ConnectionFailed(
    //                                 "Interface is inactive!".into(),
    //                             ));
    //                         }
    //                         _ => {}
    //                     }
    //                 }
    //             }
    //             Ok(None) => {
    //                 return Err(ManndError::OperationFailed(
    //                     "DBus stream ended unexpectedly.".into(),
    //                 ));
    //             }
    //             Err(_) => {
    //                 if start.elapsed() > max_wait {
    //                     return Err(ManndError::Timeout);
    //                 }
    //             }
    //         }
    //     }
    // }

    fn get_security(rsn: &HashMap<String, Value<'_>>) -> NetworkSecurity {
        if rsn.is_empty() {
            return NetworkSecurity::Open;
        }

        let key_mgmt: Vec<String> = rsn
            .get("KeyMgmt")
            .and_then(|v| v.clone().downcast::<Vec<String>>().ok())
            .unwrap_or_default();

        if key_mgmt.is_empty() {
            return NetworkSecurity::Open;
        }

        let has = |needle: &str| {
            key_mgmt
                .iter()
                .any(|k| k.to_ascii_lowercase().contains(needle))
        };

        let sae = has("sae");
        let psk = has("psk");
        let owe = has("owe");

        if owe {
            return NetworkSecurity::Owe;
        }

        if sae && psk {
            return NetworkSecurity::Wpa3Transition {
                password: String::new(),
            };
        }

        if sae {
            return NetworkSecurity::Wpa3Sae {
                password: String::new(),
                pwe: None,
            };
        }

        if psk {
            return NetworkSecurity::Wpa2 {
                passphrase: String::new(),
            };
        }

        NetworkSecurity::Open
    }
}

// Utility functions
impl WpaSupplicant {
    #[instrument(err)]
    async fn get_interface_prop<T>(&self, prop: &'static str) -> Result<T, ManndError>
    where
        for<'a> T: TryFrom<Value<'a>>,
        for<'a> <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
    {
        // lifetime issue with interface proxy so made directly
        let proxy = self.active_interface_proxy().await?;
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
        let proxy = self.active_interface_proxy().await?;
        let stream = proxy.receive_signal(signal_name).await?;
        Ok(stream)
    }

    fn sort_networks(&self, networks: &mut [NetworkInfo]) {
        match self.config.ui.sort_networks_by {
            WpaUiSort::SignalStrength => {
                networks.sort_by(|a, b| {
                    let signal_ord = match (a.signal_dbm, b.signal_dbm) {
                        (Some(a_sig), Some(b_sig)) => b_sig.cmp(&a_sig),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => Ordering::Equal,
                    };
                    signal_ord.then_with(|| {
                        a.ssid
                            .to_ascii_lowercase()
                            .cmp(&b.ssid.to_ascii_lowercase())
                    })
                });
            }
            WpaUiSort::NameAsc => {
                networks.sort_by(|a, b| {
                    a.ssid
                        .to_ascii_lowercase()
                        .cmp(&b.ssid.to_ascii_lowercase())
                });
            }
            WpaUiSort::NameDesc => {
                networks.sort_by(|a, b| {
                    b.ssid
                        .to_ascii_lowercase()
                        .cmp(&a.ssid.to_ascii_lowercase())
                });
            }
        }
    }

    async fn get_interface_proxy(
        &self,
        iface_path: OwnedObjectPath,
    ) -> Result<Proxy<'_>, ManndError> {
        Ok(Proxy::new(
            &self.conn,
            self.service.clone(),
            iface_path,
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?)
    }

    async fn get_network_proxy(&self, net_path: OwnedObjectPath) -> Result<Proxy<'_>, ManndError> {
        Ok(Proxy::new(
            &self.conn,
            self.service.clone(),
            net_path,
            "fi.w1.wpa_supplicant1.Network",
        )
        .await?)
    }

    fn build_add_network_body(
        &self,
        network: &NetworkInfo,
    ) -> Result<HashMap<String, OwnedValue>, ManndError> {
        let mut body = HashMap::new();

        body.insert(
            "ssid".into(),
            Value::new(network.ssid.as_str()).try_to_owned()?,
        );

        if network.hidden {
            body.insert("scan_ssid".into(), Value::new(1_i32).try_to_owned()?);
        }

        match &network.security {
            NetworkSecurity::Open => {
                body.insert("key_mgmt".into(), Value::new("NONE").try_to_owned()?);
            }
            NetworkSecurity::Wpa2 { passphrase } => {
                body.insert(
                    "psk".into(),
                    Value::new(passphrase.as_str()).try_to_owned()?,
                );
            }
            NetworkSecurity::Wpa2Hex { psk_hex } => {
                body.insert("psk".into(), Value::new(psk_hex.as_str()).try_to_owned()?);
            }
            NetworkSecurity::Wpa3Sae { password, .. } => {
                body.insert("key_mgmt".into(), Value::new("SAE").try_to_owned()?);
                body.insert(
                    "sae_password".into(),
                    Value::new(password.as_str()).try_to_owned()?,
                );
            }
            NetworkSecurity::Wpa3Transition { password } => {
                body.insert("psk".into(), Value::new(password.as_str()).try_to_owned()?);
            }
            NetworkSecurity::Owe => {
                body.insert("key_mgmt".into(), Value::new("OWE").try_to_owned()?);
            }
        }

        Ok(body)
    }

    #[instrument(err, skip(self))]
    async fn add_network(&self, network: &NetworkInfo) -> Result<OwnedObjectPath, ManndError> {
        validate_network(network)?;

        let Some(_) = &self.state.active_interface else {
            return Err(ManndError::WpaNoInterfaces);
        };

        let body = self.build_add_network_body(network)?;

        let network_path: OwnedObjectPath = self
            .call_interface_method::<_, OwnedObjectPath>("AddNetwork", body)
            .await?;

        Ok(network_path)
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
