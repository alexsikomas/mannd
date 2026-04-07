// Dbus api: https://git.kernel.org/pub/scm/network/wireless/iwd.git/tree/doc
use std::{collections::HashMap, fmt::Debug, sync::Arc};
use tokio::sync::{RwLock, mpsc::Sender};

use tracing::{info, instrument};
use zbus::{
    Connection, Proxy,
    fdo::ObjectManagerProxy,
    zvariant::{ObjectPath, OwnedObjectPath},
};

use crate::{
    error::ManndError,
    modify_global, read_global,
    state::signals::SignalUpdate,
    store::{NetworkInfo, NetworkInfoBuilder, NetworkSecurity},
    utils::ssid_to_hex,
    wireless::{
        agent::AgentState,
        common::{NetworkFlags, get_prop_from_proxy},
    },
};

#[derive(Debug)]
pub struct Iwd {
    path: String,
    service: String,
    /// System connection
    conn: Connection,
    agent_state: Arc<RwLock<AgentState>>,
}

impl Iwd {
    #[instrument(err, skip(conn, agent_state))]
    pub async fn new(
        conn: Connection,
        agent_state: Arc<RwLock<AgentState>>,
    ) -> Result<Self, ManndError> {
        let service = "net.connman.iwd".to_string();

        match Self::find_adapter_path(&conn, &service).await {
            Ok(Some(path)) => Ok(Self {
                path,
                service,
                conn,
                agent_state,
            }),
            Err(e) => Err(ManndError::AdapterNotFound(format!(
                "Could not find an adapter, is iwd installed?\n Error: {e}"
            ))),
            _ => Err(ManndError::AdapterNotFound(
                "Could not find an adapter, is iwd installed?".to_string(),
            )),
        }
    }

    pub async fn get_networks(&mut self) -> Result<Vec<NetworkInfo>, ManndError> {
        let station = self.get_interface_proxy("Station").await?;
        let nearby: Vec<(OwnedObjectPath, i16)> = station.call("GetOrderedNetworks", &()).await?;

        if nearby.is_empty() {
            return Ok(vec![]);
        }

        let stored = read_global(|state| state.app.saved_networks.clone()).unwrap_or_default();
        let mut by_ssid: HashMap<String, NetworkInfo> =
            stored.into_iter().map(|n| (n.ssid.clone(), n)).collect();

        for net in by_ssid.values_mut() {
            net.flags
                .remove(NetworkFlags::NEARBY | NetworkFlags::CONNECTED | NetworkFlags::KNOWN);
            net.signal_dbm = None;
        }

        for (path, signal_strength) in nearby {
            if let Ok(ap) = self.get_ap_info(path, Some(signal_strength)).await {
                by_ssid
                    .entry(ap.ssid.clone())
                    .and_modify(|n| {
                        n.flags.insert(ap.flags);
                        n.security = ap.security.clone();
                        n.signal_dbm = match (n.signal_dbm, ap.signal_dbm) {
                            (Some(cur), Some(new)) => Some(cur.max(new)),
                            (Some(cur), None) => Some(cur),
                            (None, Some(new)) => Some(new),
                            (None, None) => None,
                        };
                    })
                    .or_insert(ap);
            };
        }

        let obj_proxy = ObjectManagerProxy::new(&self.conn, self.service.clone(), "/").await?;
        for (_path, interfaces) in obj_proxy.get_managed_objects().await? {
            let Some(props) = interfaces.get("net.connman.iwd.KnownNetwork") else {
                continue;
            };

            let ssid = props
                .get("Name")
                .and_then(|v| v.downcast_ref::<&str>().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let sec_str = props
                .get("Type")
                .and_then(|v| v.downcast_ref::<&str>().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();

            if !ssid.is_empty() {
                by_ssid
                    .entry(ssid.clone())
                    .and_modify(|n| n.flags.insert(NetworkFlags::KNOWN))
                    .or_insert_with(|| {
                        NetworkInfoBuilder::default()
                            .ssid(ssid)
                            .security(Self::iwd_type_to_security(&sec_str))
                            .flags(NetworkFlags::KNOWN)
                            .build()
                            .expect("NetworkInfo builder shouldn't fail")
                    });
            }
        }

        let networks: Vec<NetworkInfo> = by_ssid.into_values().collect();
        Ok(networks)
    }

    #[instrument(err, skip(self))]
    pub async fn connect_network(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        let password = match &network.security {
            NetworkSecurity::Wpa2 { passphrase } => Some(passphrase.clone()),
            NetworkSecurity::Wpa3Sae { password, .. } => Some(password.clone()),
            NetworkSecurity::Wpa3Transition { password } => Some(password.clone()),
            _ => None,
        };

        if let Some(pass) = password {
            if let Ok(mut writer) = self.agent_state.try_write() {
                writer.password = Some(pass);
            }
        }
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.network_path(network),
            "net.connman.iwd.Network",
        )
        .await?;
        proxy.call_noreply("Connect", &()).await?;
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

    /// Disconnects from the current Wi-Fi network, doesn't remove the network
    #[instrument(err, skip(self))]
    pub async fn disconnect(&self) -> Result<(), ManndError> {
        let proxy = self.get_interface_proxy("Station").await?;
        let resp: Result<(), zbus::Error> = proxy.call("Disconnect", &()).await;
        info!("Calling the disconnect function");
        match resp {
            Ok(()) => {
                info!("Disconnected from network");
                Ok(())
            }
            Err(err) => {
                tracing::error!("Could not disconnect. {err}");
                Err(ManndError::OperationFailed("Disconnect".to_string()))
            }
        }
    }

    /// Removes a network from the configured networks
    #[instrument(err, skip(self))]
    pub async fn remove_network(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        todo!()
        // info!("/net/connman/iwd/{}_{}", ssid_to_hex(&ssid), security,);
        //
        // let proxy = Proxy::new(
        //     &self.conn,
        //     self.service.clone(),
        //     format!("/net/connman/iwd/{}_{}", ssid_to_hex(&ssid), security,),
        //     "net.connman.iwd.KnownNetwork",
        // )
        // .await?;
        //
        // let res: Result<(), zbus::Error> = proxy.call("Forget", &()).await;
        // match res {
        //     Ok(()) => {
        //         info!("Successfully forgot {ssid}");
        //         Ok(())
        //     }
        //     Err(e) => {
        //         tracing::error!("Error occured while trying to forget network: {e}");
        //         Err(ManndError::OperationFailed("Remove Network".to_string()))
        //     }
        // }
    }

    #[instrument(err, skip(self))]
    pub async fn unregister_agent(&self) -> Result<(), ManndError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            "/net/connman/iwd",
            "net.connman.iwd.AgentManager",
        )
        .await?;

        let agent_path = "/org/mannd/IwdAgent";
        let agent_objpath = zbus::zvariant::OwnedObjectPath::try_from(agent_path)?;

        let resp: Result<(), zbus::Error> = proxy.call("UnregisterAgent", &(agent_objpath)).await;
        match resp {
            Ok(()) => {
                info!("Sucessfully unregisted agent");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Error unregistering agent: {:?}", e);
                Err(ManndError::OperationFailed(e.to_string()))
            }
        }
    }

    #[instrument(err)]
    pub async fn scan(&mut self, signal_tx: Sender<SignalUpdate<'_>>) -> Result<(), ManndError> {
        let proxy = self.get_interface_proxy("Station").await?;
        if !get_prop_from_proxy::<bool>(&proxy, "Scanning").await? {
            proxy.call_noreply("Scan", &()).await?;
            let proxy = Proxy::new(
                &self.conn,
                self.service.clone(),
                self.path.clone(),
                "org.freedesktop.DBus.Properties",
            )
            .await?;
            let signal = proxy.receive_signal("PropertiesChanged").await?;
            let _ = signal_tx.send(SignalUpdate::Add(signal)).await;
        }
        Ok(())
    }
}

// Helper methods
impl Iwd {
    #[instrument(err, skip(self))]
    async fn get_ap_info(
        &self,
        path: OwnedObjectPath,
        signal_dbm: Option<i16>,
    ) -> Result<NetworkInfo, ManndError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            path,
            "net.connman.iwd.Network",
        )
        .await?;

        let ssid = get_prop_from_proxy::<String>(&proxy, "Name").await?;
        let sec_str = get_prop_from_proxy::<String>(&proxy, "Type").await?;
        let mut flags = NetworkFlags::NEARBY;
        if get_prop_from_proxy::<OwnedObjectPath>(&proxy, "KnownNetwork")
            .await
            .is_ok()
        {
            flags |= NetworkFlags::KNOWN;
        }
        if get_prop_from_proxy::<bool>(&proxy, "Connected")
            .await
            .unwrap_or(false)
        {
            flags |= NetworkFlags::CONNECTED;
        }

        Ok(NetworkInfoBuilder::default()
            .ssid(ssid)
            .security(Self::iwd_type_to_security(&sec_str))
            .signal_dbm(signal_dbm)
            .flags(flags)
            .build()?)
    }

    /// Returns the object path of the iwd station, currently only returns the first station
    #[instrument(err, skip(conn))]
    async fn find_adapter_path(
        conn: &Connection,
        service: &str,
    ) -> Result<Option<String>, ManndError> {
        let proxy = ObjectManagerProxy::new(conn, service, "/").await?;
        for (path, interface) in proxy.get_managed_objects().await? {
            // BUG: if multiple adapters, return first one
            if interface.contains_key("net.connman.iwd.Station") {
                return Ok(Some(path.to_string()));
            }
        }
        Ok(None)
    }

    #[instrument(err, skip(self))]
    pub async fn register_agent(&self) -> Result<(), ManndError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            "/net/connman/iwd",
            "net.connman.iwd.AgentManager",
        )
        .await?;

        let agent_path = ObjectPath::from_static_str("/org/mannd/IwdAgent")?;
        let resp: Result<(), zbus::Error> = proxy.call("RegisterAgent", &(agent_path)).await;
        match resp {
            Ok(()) => {
                info!("Agent registration call successful.");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Error registering agent: {:?}", e);
                Err(ManndError::OperationFailed(e.to_string()))
            }
        }
    }

    #[instrument(err, skip(self))]
    pub async fn get_modes(&self) -> Result<Vec<String>, ManndError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "net.connman.iwd.Adapter",
        )
        .await?;

        let modes = get_prop_from_proxy::<Vec<String>>(&proxy, "SupportedModes").await?;
        Ok(modes)
    }

    #[instrument(err, skip(self))]
    async fn get_interface_proxy(&self, interface: &'static str) -> Result<Proxy<'_>, ManndError> {
        Ok(Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            format!("net.connman.iwd.{interface}"),
        )
        .await?)
    }

    fn network_path(&self, network: &NetworkInfo) -> String {
        format!(
            "{}/{}_{}",
            self.path,
            ssid_to_hex(&network.ssid),
            Self::iwd_security_type(&network.security)
        )
    }

    fn iwd_security_type(security: &NetworkSecurity) -> &'static str {
        match security {
            NetworkSecurity::Open | NetworkSecurity::Owe => "open",
            _ => "psk",
        }
    }

    fn iwd_type_to_security(type_str: &str) -> NetworkSecurity {
        match type_str {
            "psk" => NetworkSecurity::Wpa2 {
                passphrase: String::new(),
            },
            _ => NetworkSecurity::Open,
        }
    }
}
