use std::sync::Arc;
use tokio::sync::{RwLock, mpsc::Sender};

use tracing::info;
use zbus::{
    Connection, Proxy,
    fdo::ObjectManagerProxy,
    zvariant::{ObjectPath, OwnedObjectPath},
};

use crate::{
    error::ComError,
    state::{network::EapInfo, signals::SignalUpdate},
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

// To be used externally
impl Iwd {
    /// Creates a new instance of the `Iwd` struct. Takes in a `zbus::Connection` to minimise the
    /// number of connections that need to be created, allowing one to be shared by the
    /// `Controller` between processes.
    pub async fn new(
        conn: Connection,
        agent_state: Arc<RwLock<AgentState>>,
    ) -> Result<Self, ComError> {
        let service = "net.connman.iwd".to_string();

        match Self::find_adapter_path(&conn, &service).await {
            Ok(Some(path)) => Ok(Self {
                conn,
                service,
                path,
                agent_state,
            }),
            Err(e) => Err(ComError::AdapterNotFound(format!(
                "Could not find an adapter, is iwd installed?\n Error: {e}"
            ))),
            _ => Err(ComError::AdapterNotFound(
                "Could not find an adapter, is iwd installed?".to_string(),
            )),
        }
    }

    /// Connects to a network provided an SSID and passphrase.
    ///
    /// Since iwd does not allow connecting via BSSID the connection band is determined by signal
    /// strength internally by iwd, this can be tweaked in the iwd configuration file
    pub async fn connect_network_psk(&self, ssid: String, psk: String) -> Result<(), ComError> {
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
            format!(
                "{}/{}_{}",
                self.path.clone(),
                Self::ssid_to_hex(ssid),
                security
            ),
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
                Err(ComError::OperationFailed(format!("{}", e)))
            }
        }
    }

    pub async fn connect_network_eap(&self, ssid: String, eap: EapInfo) -> Result<(), ComError> {
        Ok(())
    }

    pub async fn connect_known(&self, ssid: String, security: Security) -> Result<(), ComError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            format!(
                "{}/{}_{}",
                self.path.clone(),
                Self::ssid_to_hex(ssid),
                security
            ),
            "net.connman.iwd.Network",
        )
        .await?;

        let _: Result<(), zbus::Error> = proxy.call("Connect", &()).await;

        Ok(())
    }

    /// Disconnects from the current WiFi network, does not remove the network
    pub async fn disconnect(&self) -> Result<(), ComError> {
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
                Err(ComError::OperationFailed("Disconnect".to_string()))
            }
        }
    }

    /// Returns the current status of the connected WiFi network
    pub async fn status(&self) -> Result<String, ComError> {
        todo!()
    }

    /// Lists all networks which are available to be connected to including networks that are out
    /// of range
    pub async fn list_configured_networks(&self) -> Result<Vec<String>, ComError> {
        todo!()
    }

    /// Removes a network from the configured networks
    pub async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError> {
        info!(
            "/net/connman/iwd/{}_{}",
            Self::ssid_to_hex(ssid.to_string()),
            security,
        );

        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            format!(
                "/net/connman/iwd/{}_{}",
                Self::ssid_to_hex(ssid.to_string()),
                security,
            ),
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
                Err(ComError::OperationFailed("Remove Network".to_string()))
            }
        }
    }

    // TODO: Research impl Drop for async/converting this func to
    // use blocking zbus instead of making this public
    pub async fn unregister_agent(&self) -> Result<(), ComError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            "/net/connman/iwd",
            "net.connman.iwd.AgentManager",
        )
        .await?;

        let agent_path = "/org/mannd/IwdAgent";
        let agent_objpath = zbus::zvariant::OwnedObjectPath::try_from(agent_path).unwrap();

        let resp: Result<(), zbus::Error> = proxy.call("UnregisterAgent", &(agent_objpath)).await;
        match resp {
            Ok(_) => {
                info!("Sucessfully unregisted agent");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Error unregistering agent: {:?}", e);
                Err(ComError::OperationFailed(e.to_string()))
            }
        }
    }

    pub async fn scan<'a>(&mut self, signal_tx: Sender<SignalUpdate<'a>>) -> Result<(), ComError> {
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

    pub async fn nearby_networks(&mut self) -> Result<Vec<AccessPoint>, ComError> {
        let proxy = self.get_interface_proxy("Station").await?;
        let aps = proxy.call_method("GetOrderedNetworks", &()).await?.body();
        let aps: Vec<(OwnedObjectPath, i16)> = aps.deserialize()?;

        let mut access_points: Vec<AccessPoint> = vec![];
        for ap in aps {
            // FIX: very janky
            let ap_info = self
                .get_ap_info(String::from(ap.0.as_str().split("/").last().unwrap()))
                .await?;
            access_points.push(ap_info);
        }

        Ok(access_points)
    }

    // FIX: This code is not something I would write anymore, check with iwd
    // device if this was done for a reason or just bad
    pub async fn get_known_networks(&mut self) -> Result<Vec<AccessPoint>, ComError> {
        let mut known_networks: Vec<AccessPoint> = vec![];
        let proxy = ObjectManagerProxy::new(&self.conn, self.service.clone(), "/").await?;
        for (path, interface) in proxy.get_managed_objects().await? {
            if let Some(known_network_props) = interface.get("net.connman.iwd.KnownNetwork") {
                let mut ssid: String = "".to_string();

                if let Some(name) = known_network_props.get("Name") {
                    ssid = name.downcast_ref::<String>().unwrap_or("".to_string());
                }

                let mut ap_builder = AccessPointBuilder::default()
                    .ssid(ssid)
                    .flags(NetworkFlags::KNOWN);

                if let Some(net_security) = known_network_props.get("Type") {
                    let security_str = net_security.downcast_ref::<&str>().unwrap_or("psk");
                    let security = Security::from_str(security_str);
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
    async fn find_adapter_path(
        conn: &Connection,
        service: &String,
    ) -> Result<Option<String>, ComError> {
        let proxy = ObjectManagerProxy::new(conn, service.clone(), "/").await?;
        for (path, interface) in proxy.get_managed_objects().await? {
            // BUG: if multiple adapters will just return first one
            if interface.contains_key("net.connman.iwd.Station") {
                return Ok(Some(path.to_string()));
            }
        }
        Ok(None)
    }

    fn ssid_to_hex(ssid: String) -> String {
        let bytes = ssid.as_bytes();
        bytes.into_iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub async fn register_agent(&self) -> Result<(), ComError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            "/net/connman/iwd",
            "net.connman.iwd.AgentManager",
        )
        .await?;

        let agent_path =
            ObjectPath::from_static_str("/org/mannd/IwdAgent").expect("Static path is valid");
        let resp: Result<(), zbus::Error> = proxy.call("RegisterAgent", &(agent_path)).await;
        match resp {
            Ok(_) => {
                info!("Agent registration call successful.");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Error registering agent: {:?}", e);
                Err(ComError::OperationFailed(e.to_string()))
            }
        }
    }

    async fn get_interface_proxy(&self, interface: &'static str) -> Result<Proxy<'_>, ComError> {
        Ok(Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            format!("net.connman.iwd.{}", interface),
        )
        .await?)
    }

    async fn get_ap_info(&self, network: String) -> Result<AccessPoint, ComError> {
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

        let security = Security::from_str(security_str.as_str());
        let mut flags = NetworkFlags::NEARBY;

        // let known_network: Option<OwnedObjectPath>;
        match get_prop_from_proxy::<OwnedObjectPath>(&proxy, "KnownNetwork").await {
            Ok(_) => {
                flags = flags | NetworkFlags::KNOWN;
            }
            // not actually an error the field is just optional
            Err(_) => {}
        };

        get_prop_from_proxy::<bool>(&proxy, "Connected")
            .await
            .inspect(|b| {
                if *b {
                    flags = flags | NetworkFlags::CONNECTED;
                }
            })?;

        let ap = AccessPointBuilder::default()
            .ssid(ssid)
            .security(security)
            .flags(flags)
            .build()?;

        Ok(ap)
    }

    pub async fn get_modes(&self) -> Result<Vec<String>, ComError> {
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

#[cfg(test)]
#[cfg(iwd_installed)]
mod tests {
    use zbus::zvariant::ObjectPath;

    use super::*;

    // Networking tests
    // async fn setup() -> Result<Iwd, ComError> {
    //     let conn = zbus::Connection::system().await?;
    //     Ok(Iwd::new(conn).await?)
    // }

    // #[tokio::test]
    // async fn test_get_connected_network() -> Result<(), ComError> {
    //     let iwd = setup().await?;
    //     iwd.get_prop::<OwnedObjectPath>("Station", "ConnectedNetwork")
    //         .await?;
    //     Ok(())
    // }

    // #[tokio::test]
    // async fn test_get_networks() -> Result<(), ComError> {
    //     let mut iwd = setup().await?;
    //     iwd.update_networks().await?;
    //     iwd.print_networks().await;
    //     Ok(())
    // }
    //
    // // configuration tests
    // #[tokio::test]
    // async fn test_get_conf_path() -> Result<(), ComError> {
    //     let path = Iwd::get_conf().await?;
    //     Ok(())
    // }
}
//
// #[derive(Debug)]
// pub struct IwdNetwork {
//     pub ap: AccessPoint,
//     ess: Vec<OwnedObjectPath>,
//     connected: bool,
//     device: OwnedObjectPath,
//     known_network: Option<OwnedObjectPath>,
// }
//

// impl std::fmt::Display for IwdNetwork {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "Name: {}\n", self.ap.ssid);
//         write!(f, "Connected: {}\n", self.connected);
//         write!(f, "Security: {}\n", self.ap.security);
//         write!(f, "Device: {:?}\n", self.device);
//         write!(f, "Known Network: {:?}\n", self.known_network);
//         write!(f, "Ess: {:?}", self.ess)
//     }
// }

// Config settings

// enum IwdConfigGroup {
//     General,
//     Network,
//     Blacklist,
//     Rank,
//     Scan,
// }
//
// enum GeneralSettings {
//     EnableNetworkConfiguration(bool),
//     AddressRandomization(AddrRandOpts),
//     AddressRandomizationRange(AddrRandRangeOpts),
//     // -100 to 1; default: -70
//     RoamThreshold(i8),
//     // default: -76
//     RoamThreshold5G(i8),
//     // default -80
//     CriticalRoamThreshold(i8),
//     // default: -82
//     CriticalRoamThreshold5G(i8),
//     RoamRetryInterval(u16),
//     ManagementFrameProtection(ManagementFrameProtectionOpts),
// }
//
// enum AddrRandOpts {
//     Disabled,
//     Once,
//     Network,
// }
//
// enum AddrRandRangeOpts {
//     Full,
//     Nic,
// }
//
// enum ManagementFrameProtectionOpts {
//     Optional,
//     Required,
//     Disabled,
// }
//
// enum NetworkSettings {
//     EnableIpv6(bool),
//     NameResolvingService(NameResolver),
//     // default: 300
//     RoutePriorityOffset(u32),
// }
//
// enum NameResolver {
//     Resolveconf,
//     Systemd,
//     None,
// }
//
// enum BlacklistSettings {
//     // default: 60
//     InitialTimeout(u32),
//     // default: 30
//     InitialAccessPointBusyTimeout(u32),
//     // default: 30
//     Multiplier(u32),
//     // default: 86400
//     MaximumTimeout(u32),
// }
//
// enum RankSettings {
//     // band modif. default: 1.0
//     BandModifier2_4Ghz(f32),
//     BandModifier5Ghz(f32),
//     BandModifier6Ghz(f32),
// }
//
// enum ScanSettings {
//     DisablePeriodicScan(bool),
//     // default: 10
//     InitialPeriodicScanInterval(u32),
//     // default: 300
//     MaximumPeriodicScanInterval(u32),
//     DisableRoamingScan(bool),
// }
