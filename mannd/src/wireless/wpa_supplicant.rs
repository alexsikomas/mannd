//! [Reference](https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network)
//!
//! The `WpaBss` struct has a lot of optional types mainly because it differs significantly to what's provided from a scan vs a connected network.
//!
//! When getting networks after a scan there can be duplicate SSIDs because `wpa_supplicant` shows the different possible freqs.

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::instrument;
use zbus::{
    Connection, DynamicType, ObjectPath, OwnedObjectPath, OwnedVariant, Proxy, Type, Variant, names::MemberName, proxy::SignalStream
};

use crate::{
    error::ManndError,
    modify_state,
    store::{
        NetworkInfo, NetworkInfoBuilder, NetworkSecurity, WpaNetworkPolicyOverrideBuilder, WpaState,
    },
    utils::{list_interfaces, validate_network, wpa_bssid_to_string},
    wireless::{
        WifiBackend,
        common::{NetworkFlags, get_prop_from_proxy},
        wifi_config::WpaAutoscan,
    },
    with_state,
};

#[derive(Debug)]
pub struct WpaSupplicant {
    /// The currently active managed interface. `None` until one is created.
    active_interface: Option<ManagedInterface>,

    service: String,
    path: String,
    conn: Connection,
}

impl WifiBackend for WpaSupplicant {
    async fn scan_networks(&self) -> Result<Option<SignalStream<'static>>, ManndError> {
        if self.get_interface_prop::<bool>("Scanning").await? {
            return Ok(None);
        }

        let mut dict: HashMap<String, OwnedVariant> = HashMap::new();
        dict.insert("Type".to_string(), Variant::new("active").try_to_owned()?);
        self.call_interface_method_noreply("Scan", dict).await?;

        Ok(Some(self.get_interface_signal("ScanDone").await?))
    }

    #[instrument(err, skip(self))]
    async fn get_networks(&self) -> Result<Vec<NetworkInfo>, ManndError> {
        let nearby = self
            .get_interface_prop::<Vec<OwnedObjectPath>>("BSSs")
            .await?;

        if nearby.is_empty() {
            return Ok(vec![]);
        }

        let saved_nets = with_state(|state| state.app.saved_networks.clone());
        let (is_conn, path) = self.is_connected().await?;

        let conn_bssid = if let Some(ref saved) = saved_nets
            && is_conn
        {
            let proxy = self.get_network_proxy(path).await?;
            let conn_info =
                get_prop_from_proxy::<HashMap<String, OwnedVariant>>(&proxy, "Properties").await?;

            let conn_ssid: &str = conn_info
                .get("ssid")
                .ok_or_else(|| {
                    ManndError::OperationFailed("Getting ssid from connected network".into())
                })?
                .try_into()?;
            let conn_ssid: String = conn_ssid.trim_matches('"').to_string();

            saved
                .iter()
                .find(|s| s.ssid == conn_ssid)
                .and_then(|s| s.bssid.clone())
        } else {
            None
        };

        // There can be multiple BSSIDs per SSID; prefer the one with the
        // highest signal / frequency, except when already connected
        let mut by_ssid: HashMap<String, (NetworkInfo, u16)> = HashMap::default();
        for network in nearby {
            let (mut info, freq) = self.get_bss_info(network).await?;

            if info.bssid == conn_bssid {
                info.flags |= NetworkFlags::KNOWN | NetworkFlags::CONNECTED;
                by_ssid.insert(info.ssid.clone(), (info, freq));
                continue;
            }

            if let Some(false) =
                with_state(|state| Arc::clone(&state.wifi_config).ui.show_hidden_networks)
                && (info.ssid.is_empty() || info.ssid.clone().into_bytes().iter().all(|&v| v == 0))
            {
                continue;
            }

            // TODO: temporary needs to account for 6 GHz later
            let is_5ghz = freq >= 5000;
            if is_5ghz {
                let mut policy = info
                    .wpa_policy_override
                    .take()
                    .unwrap_or_else(|| WpaNetworkPolicyOverrideBuilder::default().build().unwrap());
                policy.allow_freq_mhz.push(u32::from(freq));
                info.wpa_policy_override = Some(policy);
                info.bssid = None;
            }

            if let Some(prev) = by_ssid.get_mut(&info.ssid) {
                if info
                    .flags
                    .contains(NetworkFlags::KNOWN | NetworkFlags::CONNECTED)
                {
                    continue;
                }

                if freq > prev.1 {
                    prev.0 = info;
                    prev.1 = freq;
                }

                if is_5ghz
                    && let Some(p) = &mut prev.0.wpa_policy_override
                {
                    p.allow_freq_mhz.push(u32::from(freq));
                }
            } else {
                if let Some(ref saved) = saved_nets
                    && saved.iter().any(|n| n.bssid == info.bssid)
                {
                    info.flags |= NetworkFlags::KNOWN;
                }
                by_ssid.insert(info.ssid.clone(), (info, freq));
            }
        }

        let mut res: Vec<NetworkInfo> = by_ssid.into_values().map(|(n, _)| n).collect();
        with_state(|state| {
            state
                .wifi_config
                .ui
                .sort_networks_by
                .sort_networks(&mut res);
        });

        Ok(res)
    }

    #[instrument(err, skip_all)]
    async fn connect_network(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        let mut net_path: Option<OwnedObjectPath> = None;

        if network.flags.contains(NetworkFlags::KNOWN) {
            let network_paths = self
                .get_interface_prop::<Vec<OwnedObjectPath>>("Networks")
                .await?;

            for path in network_paths {
                if self.get_known_ssid(&path).await? == network.ssid {
                    net_path = Some(path);
                    break;
                }
            }
        }

        let path = match net_path {
            Some(p) => p,
            None if network.flags.contains(NetworkFlags::KNOWN) => {
                return Err(ManndError::NetworkNotFound);
            }
            None => self.add_network(network).await?,
        };

        if let Some(freq) = &network.pref_ghz {
            let freqs: &'static str = freq.clone().into();

            let mut props: HashMap<String, OwnedVariant> = HashMap::new();
            props.insert("freq_list".into(), Variant::from(freqs).try_to_owned()?);
            let proxy = self.get_network_proxy(path.clone()).await?;
            proxy.set_property("Properties", props).await?;
        }

        self.call_interface_method_noreply("SelectNetwork", path)
            .await?;

        modify_state(|state| {
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
            let _ = state.db.write_app_state(&state.app);
        });
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), ManndError> {
        self.call_interface_method_noreply("Disconnect", &())
            .await?;
        Ok(())
    }

    #[instrument(err, skip_all)]
    async fn forget_network(&self, network: &NetworkInfo) -> Result<(), ManndError> {
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

        modify_state(|state| {
            state.app.saved_networks.retain(|n| n.ssid != network.ssid);
            let _ = state.db.write_app_state(&state.app);
        });

        Ok(())
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

impl WpaSupplicant {
    /// Construct a new `WpaSupplicant` backend.
    ///
    /// Looks up all currently managed interfaces from D-Bus and picks the
    /// active one according to this preference order:
    ///   1. `preferred_interface` from `wifi.conf`
    ///   2. The interface name persisted in `WpaState` (last session's choice)
    ///   3. The first managed interface found
    #[instrument(err, skip(conn))]
    pub async fn new(state: WpaState, conn: Connection) -> Result<Self, ManndError> {
        let service = String::from("fi.w1.wpa_supplicant1");
        let path = String::from("/fi/w1/wpa_supplicant1");

        let mut wpa = Self {
            active_interface: None,
            service,
            path,
            conn,
        };

        wpa.pick_active_interface(&state.active_interface_name)
            .await?;
        wpa.apply_global_policy().await?;

        Ok(wpa)
    }

    /// Returns every known interface (both managed by wpa_supplicant and
    /// unmanaged physical interfaces).  The result is used by the TUI to
    /// let the user add/remove interfaces
    #[instrument(err, skip(self))]
    pub async fn get_interfaces(&self) -> Result<Vec<WpaInterface>, ManndError> {
        let phys_interfaces = list_interfaces();
        let iface_paths = self.get_managed_interface_paths().await?;

        let mut managed_by_name: HashMap<String, ManagedInterface> = HashMap::new();
        for path in iface_paths {
            let iface_proxy = self.get_interface_proxy(path.clone()).await?;
            let ifname: String = get_prop_from_proxy(&iface_proxy, "Ifname").await?;
            managed_by_name.insert(ifname.clone(), ManagedInterface::new(ifname, path));
        }

        let mut result: Vec<WpaInterface> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for phy in &phys_interfaces {
            seen.insert(phy.clone());
            if let Some(managed) = managed_by_name.remove(phy) {
                result.push(WpaInterface::Managed(managed));
            } else {
                result.push(WpaInterface::Unmanaged(phy.clone()));
            }
        }

        // any managed interfaces not in the physical list still appear
        for (_name, managed) in managed_by_name {
            result.push(WpaInterface::Managed(managed));
        }

        Ok(result)
    }

    /// Register a new interface with wpa_supplicant and make it the active one.
    #[instrument(err, skip(self))]
    pub async fn create_interface(&mut self, ifname: &str) -> Result<(), ManndError> {
        let managed = self.create_interface_on_dbus(ifname).await?;
        self.active_interface = Some(managed);
        self.write_state()?;
        Ok(())
    }

    /// Deregister an interface from wpa_supplicant.  If it was the active
    /// interface the backend will attempt to fall back to another managed one.
    #[instrument(err, skip(self))]
    pub async fn remove_interface(&mut self, ifname: &str) -> Result<(), ManndError> {
        let ifaces = self.get_interfaces().await?;

        let Some(target) = ifaces.iter().find_map(|i| match i {
            WpaInterface::Managed(m) if m.name == ifname => Some(m.clone()),
            _ => None,
        }) else {
            return Err(ManndError::WpaRemoveNotFound);
        };

        let root_proxy = self.root_proxy().await?;
        root_proxy
            .call_noreply("RemoveInterface", &(target.path.clone()))
            .await?;

        let was_active = self
            .active_interface
            .as_ref()
            .is_some_and(|a| a.name == ifname);

        if was_active {
            // Try to pick a replacement from what remains on D-Bus
            self.pick_active_interface(&None).await?;
        }

        self.write_state()?;
        Ok(())
    }
}

// helpers
impl WpaSupplicant {
    /// Queries D-Bus for the list of managed interface object paths.
    async fn get_managed_interface_paths(&self) -> Result<Vec<OwnedObjectPath>, ManndError> {
        let proxy = self.root_proxy().await?;
        get_prop_from_proxy::<Vec<OwnedObjectPath>>(&proxy, "Interfaces").await
    }

    /// Builds a proxy for the wpa_supplicant root service object.
    async fn root_proxy(&self) -> Result<Proxy<'_>, ManndError> {
        Ok(Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            self.service.clone(),
        )
        .await?)
    }

    async fn active_interface_proxy(&self) -> Result<Proxy<'_>, ManndError> {
        let Some(interface) = &self.active_interface else {
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

    async fn is_connected(&self) -> Result<(bool, OwnedObjectPath), ManndError> {
        let cur_network = self
            .get_interface_prop::<OwnedObjectPath>("CurrentNetwork")
            .await?;

        let is_conn = cur_network.ne(&OwnedObjectPath::from(ObjectPath::from_str_unchecked("/")));
        Ok((is_conn, cur_network))
    }

    /// Select the active interface according to:
    ///   1. `preferred_interface` from config
    ///   2. `preferred_name` argument (last session's persisted name)
    ///   3. First available managed interface
    async fn pick_active_interface(
        &mut self,
        preferred_name: &Option<String>,
    ) -> Result<(), ManndError> {
        let iface_paths = self.get_managed_interface_paths().await?;

        // Build a (name, path) map for the currently managed interfaces
        let mut by_name: HashMap<String, ManagedInterface> = HashMap::new();
        for path in iface_paths {
            let proxy = self.get_interface_proxy(path.clone()).await?;
            let ifname: String = get_prop_from_proxy(&proxy, "Ifname").await?;
            by_name.insert(ifname.clone(), ManagedInterface::new(ifname, path));
        }

        let config_pref: Option<String> =
            with_state(|state| state.wifi_config.general.preferred_interface.clone()).flatten();

        self.active_interface = [config_pref.as_deref(), preferred_name.as_deref()]
            .into_iter()
            .flatten()
            .find_map(|name| by_name.remove(name))
            .or_else(|| by_name.into_values().next());

        self.write_state()?;
        Ok(())
    }

    #[instrument(err, skip(self))]
    async fn apply_global_policy(&self) -> Result<(), ManndError> {
        let iface_paths = self.get_managed_interface_paths().await?;
        for path in iface_paths {
            let proxy = self.get_interface_proxy(path).await?;
            self.apply_global_policy_on_proxy(&proxy).await?;
        }
        Ok(())
    }

    async fn apply_global_policy_on_proxy(&self, proxy: &Proxy<'_>) -> Result<(), ManndError> {
        let Some(config) = with_state(|state| Arc::clone(&state.wifi_config)) else {
            return Ok(());
        };

        if let Some(country) = &config.general.country {
            proxy.set_property("Country", country.clone()).await?;
        }

        proxy
            .set_property("FastReauth", config.wpa.fast_reauth)
            .await?;

        if let Some(scan_interval) = config.wpa.scan_interval_sec {
            let interval_i32 = i32::try_from(scan_interval)?;
            proxy.set_property("ScanInterval", interval_i32).await?;
        }

        let autoscan_type = match &config.wpa.autoscan {
            WpaAutoscan::Disabled => String::new(),
            WpaAutoscan::Exponential { base, limit } => format!("exponential:{base}:{limit}"),
            WpaAutoscan::Periodic { interval } => format!("periodic:{interval}"),
        };
        proxy.call_noreply("AutoScan", &autoscan_type).await?;

        Ok(())
    }

    #[instrument(err, skip(self))]
    async fn call_interface_method_noreply<T>(
        &self,
        method_name: &'static str,
        body: T,
    ) -> Result<(), ManndError>
    where
        T: serde::ser::Serialize + DynamicType + Debug + Send + Sync,
    {
        let proxy = self.active_interface_proxy().await?;
        proxy.call_noreply(method_name, &body).await?;
        Ok(())
    }

    fn write_state(&self) -> Result<(), ManndError> {
        let wpa_state = WpaState {
            active_interface_name: self.active_interface.as_ref().map(|m| m.name.clone()),
        };
        with_state(|state| state.db.write_wpa_state(&wpa_state)).transpose()?;
        Ok(())
    }

    async fn create_interface_on_dbus(&self, ifname: &str) -> Result<ManagedInterface, ManndError> {
        let mut body: HashMap<String, Variant> = HashMap::new();
        body.insert("Ifname".into(), Variant::from(ifname));

        let proxy = self.root_proxy().await?;
        let interface_path: OwnedObjectPath = proxy.call("CreateInterface", &body).await?;
        Ok(ManagedInterface::new(ifname.into(), interface_path))
    }

    // Known network only returns SSID
    #[instrument(err, skip(self))]
    async fn get_known_ssid(&self, net_path: &OwnedObjectPath) -> Result<String, ManndError> {
        let proxy = self.get_network_proxy(net_path.clone()).await?;
        let properties =
            get_prop_from_proxy::<HashMap<String, OwnedVariant>>(&proxy, "Properties").await?;
        match properties.get("ssid").map(|v| &**v) {
            Some(Variant::Str(s)) => Ok(s.as_str().trim_matches('"').to_string()),
            _ => Err(ManndError::NetworkNotFound)
        }
    }

    /// Used for nearby networks that may not have been connected to yet
    #[instrument(err, skip(self))]
    async fn get_bss_info(
        &self,
        bss_path: OwnedObjectPath,
    ) -> Result<(NetworkInfo, u16), ManndError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            bss_path,
            "fi.w1.wpa_supplicant1.BSS",
        )
        .await?;

        let bssid = get_prop_from_proxy::<Vec<u8>>(&proxy, "BSSID").await?;
        let bssid = wpa_bssid_to_string(bssid);
        let ssid_vec = get_prop_from_proxy::<Vec<u8>>(&proxy, "SSID").await?;
        let ssid = String::from_utf8_lossy(&ssid_vec).to_string();

        let signal_dbm = get_prop_from_proxy::<i16>(&proxy, "Signal").await.ok();
        let freq = get_prop_from_proxy::<u16>(&proxy, "Frequency").await?;

        let rsn = get_prop_from_proxy::<HashMap<String, OwnedVariant>>(&proxy, "RSN").await?;
        let security = Self::get_security(&rsn);

        let info = NetworkInfoBuilder::default()
            .ssid(ssid)
            .bssid(Some(bssid))
            .security(security)
            .signal_dbm(signal_dbm)
            .wpa_policy_override(Some(
                WpaNetworkPolicyOverrideBuilder::default()
                    .allow_freq_mhz(vec![freq.into()])
                    .build()?,
            ))
            .flags(NetworkFlags::NEARBY)
            .build()?;

        Ok((info, freq))
    }

    fn get_security(rsn: &HashMap<String, OwnedVariant>) -> NetworkSecurity {
        if rsn.is_empty() {
            return NetworkSecurity::Open;
        }

        let key_mgmt: Vec<String> = rsn
            .get("KeyMgmt")
            .and_then(|v| Vec::<String>::try_from(v.clone()).ok())
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

// Proxy & property utilities

impl WpaSupplicant {
    #[instrument(err)]
    async fn get_interface_prop<T>(&self, prop: &'static str) -> Result<T, ManndError>
    where
        T: DeserializeOwned + Type,
    {
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
        network: &NetworkInfo,
    ) -> Result<HashMap<String, OwnedVariant>, ManndError> {
        let mut body = HashMap::new();

        body.insert(
            "ssid".into(),
            Variant::new(network.ssid.as_str()).try_to_owned()?,
        );

        if network.hidden {
            body.insert("scan_ssid".into(), Variant::new(1_i32).try_to_owned()?);
        }

        match &network.security {
            NetworkSecurity::Open => {
                body.insert("key_mgmt".into(), Variant::new("NONE").try_to_owned()?);
            }
            NetworkSecurity::Wpa2 { passphrase } => {
                body.insert(
                    "psk".into(),
                    Variant::new(passphrase.as_str()).try_to_owned()?,
                );
            }
            NetworkSecurity::Wpa2Hex { psk_hex } => {
                body.insert("psk".into(), Variant::new(psk_hex.as_str()).try_to_owned()?);
            }
            NetworkSecurity::Wpa3Sae { password, .. } => {
                body.insert("key_mgmt".into(), Variant::new("SAE").try_to_owned()?);
                body.insert(
                    "sae_password".into(),
                    Variant::new(password.as_str()).try_to_owned()?,
                );
            }
            NetworkSecurity::Wpa3Transition { password } => {
                body.insert("psk".into(), Variant::new(password.as_str()).try_to_owned()?);
            }
            NetworkSecurity::Owe => {
                body.insert("key_mgmt".into(), Variant::new("OWE").try_to_owned()?);
            }
        }

        if let Some(freq) = &network.pref_ghz {
            let freqs: &'static str = freq.clone().into();
            body.insert("freq_list".into(), Variant::new(freqs).try_to_owned()?);
        }

        Ok(body)
    }

    #[instrument(err, skip_all)]
    async fn add_network(&self, network: &NetworkInfo) -> Result<OwnedObjectPath, ManndError> {
        validate_network(network)?;

        if self.active_interface.is_none() {
            return Err(ManndError::WpaNoInterfaces);
        }

        let body = WpaSupplicant::build_add_network_body(network)?;

        let proxy = self.active_interface_proxy().await?;
        let network_path: OwnedObjectPath = proxy.call("AddNetwork", &body).await?;

        Ok(network_path)
    }
}

// public types (used by TUI / messages)

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManagedInterface {
    pub name: String,
    pub path: OwnedObjectPath,
}

impl ManagedInterface {
    const fn new(name: String, path: OwnedObjectPath) -> Self {
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
