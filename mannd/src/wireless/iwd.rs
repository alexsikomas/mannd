use std::{
    fmt::{Debug, Write},
    sync::Arc,
};
use tokio::sync::{RwLock, mpsc::Sender};

use tracing::{info, instrument};
use zbus::{
    Connection, Proxy,
    fdo::ObjectManagerProxy,
    zvariant::{ObjectPath, OwnedObjectPath},
};

use crate::{
    error::ManndError,
    state::signals::SignalUpdate,
    utils::ssid_to_hex,
    wireless::{
        agent::AgentState,
        common::{AccessPoint, AccessPointBuilder, NetworkFlags, Security, get_prop_from_proxy},
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

    /// Connects to a network provided an SSID and passphrase.
    ///
    /// Since iwd doesn't allow connecting via BSSID the connection band is determined by signal
    /// strength internally by iwd, tweakable in iwd config
    #[instrument(err, skip(self))]
    pub async fn connect_network_psk(&self, ssid: String, psk: String) -> Result<(), ManndError> {
        match self.agent_state.try_write() {
            Ok(mut writer) => {
                writer.password = Some(psk.clone());
            }
            Err(e) => {
                tracing::error!("Error trying to get lock on writer. {e}");
            }
        }

        let security = if psk.is_empty() {
            Security::Open
        } else {
            Security::Psk
        };

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            format!("{}/{}_{}", self.path.clone(), ssid_to_hex(&ssid), security),
            "net.connman.iwd.Network",
        )
        .await?;

        info!("Attempting to connect.");
        let resp: Result<(), zbus::Error> = proxy.call("Connect", &()).await;
        match resp {
            Ok(()) => {
                info!("Connected to the network without recieving an error.");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Error occured: {e}");
                Err(ManndError::OperationFailed(format!("{e}")))
            }
        }
    }

    #[instrument(err, skip(self))]
    pub async fn connect_known(&self, ssid: String, security: Security) -> Result<(), ManndError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            format!("{}/{}_{}", self.path.clone(), ssid_to_hex(&ssid), security),
            "net.connman.iwd.Network",
        )
        .await?;

        let _: Result<(), zbus::Error> = proxy.call("Connect", &()).await;

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
    pub async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ManndError> {
        info!("/net/connman/iwd/{}_{}", ssid_to_hex(&ssid), security,);

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            format!("/net/connman/iwd/{}_{}", ssid_to_hex(&ssid), security,),
            "net.connman.iwd.KnownNetwork",
        )
        .await?;

        let res: Result<(), zbus::Error> = proxy.call("Forget", &()).await;
        match res {
            Ok(()) => {
                info!("Successfully forgot {ssid}");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Error occured while trying to forget network: {e}");
                Err(ManndError::OperationFailed("Remove Network".to_string()))
            }
        }
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

    // Gets nearby and known networks
    #[instrument(err, skip(self))]
    pub async fn all_networks(&mut self) -> Result<Vec<AccessPoint>, ManndError> {
        let proxy = self.get_interface_proxy("Station").await?;
        let nearby_aps = proxy.call_method("GetOrderedNetworks", &()).await?.body();
        let nearby_aps: Vec<(OwnedObjectPath, i16)> = nearby_aps.deserialize()?;

        let mut access_points: Vec<AccessPoint> = vec![];
        for ap in nearby_aps {
            if let Some(ap_name) = ap.0.as_str().split('/').next_back() {
                let ap_info = self.get_ap_info(ap_name).await?;
                access_points.push(ap_info);
            }
        }

        let known_aps = self.get_known_networks().await?;
        let aps_to_add: Vec<AccessPoint> = known_aps
            .into_iter()
            .filter(|known_ap| !access_points.iter().any(|ap| ap.ssid == known_ap.ssid))
            .collect();

        access_points.extend(aps_to_add);
        Ok(access_points)
    }

    #[instrument(err, skip(self))]
    pub async fn get_known_networks(&mut self) -> Result<Vec<AccessPoint>, ManndError> {
        let mut known_networks: Vec<AccessPoint> = vec![];
        let proxy = ObjectManagerProxy::new(&self.conn, self.service.clone(), "/").await?;
        for (_path, interface) in proxy.get_managed_objects().await? {
            if let Some(known_network_props) = interface.get("net.connman.iwd.KnownNetwork") {
                let ssid = known_network_props
                    .get("Name")
                    .map_or_else(String::new, |name| {
                        name.downcast_ref::<String>().unwrap_or(String::new())
                    });

                let mut ap_builder = AccessPointBuilder::default()
                    .ssid(ssid)
                    .flags(NetworkFlags::KNOWN);

                if let Some(net_security) = known_network_props.get("Type") {
                    let security_str = net_security.downcast_ref::<&str>().unwrap_or("psk");
                    let security: Security = security_str.into();
                    ap_builder = ap_builder.security(security);
                }

                known_networks.push(ap_builder.build()?);
            }
        }
        Ok(known_networks)
    }
}

// Helper methods
impl Iwd {
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
    async fn get_interface_proxy(&self, interface: &'static str) -> Result<Proxy<'_>, ManndError> {
        Ok(Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            format!("net.connman.iwd.{interface}"),
        )
        .await?)
    }

    #[instrument(err, skip(self))]
    async fn get_ap_info<T: Into<String> + Debug>(
        &self,
        network: T,
    ) -> Result<AccessPoint, ManndError> {
        let network: String = network.into();
        let proxy = zbus::Proxy::new(
            &self.conn,
            self.service.clone(),
            format!("{}/{}", self.path.clone(), network),
            "net.connman.iwd.Network",
        )
        .await?;

        info!("Getting ap info");
        let ssid = get_prop_from_proxy::<String>(&proxy, "Name").await?;
        info!("ssid: {}", ssid);

        let security_str = get_prop_from_proxy::<String>(&proxy, "Type").await?;
        info!("sec: {:?}", security_str);

        let security: Security = security_str.as_str().into();
        let mut flags = NetworkFlags::NEARBY;

        if (get_prop_from_proxy::<OwnedObjectPath>(&proxy, "KnownNetwork").await).is_ok() {
            flags |= NetworkFlags::KNOWN;
        }

        get_prop_from_proxy::<bool>(&proxy, "Connected")
            .await
            .inspect(|b| {
                if *b {
                    flags |= NetworkFlags::CONNECTED;
                }
            })?;

        let ap = AccessPointBuilder::default()
            .ssid(ssid)
            .security(security)
            .flags(flags)
            .build()?;

        Ok(ap)
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
}
