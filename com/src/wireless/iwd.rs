use std::{env, fmt::format, path::PathBuf};

use async_trait::async_trait;
use quick_xml::{Reader, events::Event};
use tokio::fs::{self, File, OpenOptions};
use zbus::{
    Connection,
    fdo::ObjectManagerProxy,
    zvariant::{ObjectPath, OwnedObjectPath, Value},
};

use crate::{error::NdError, wireless::WifiAdapter};

pub struct Iwd {
    path: String,
    service: String,
    conn: Connection,
    networks: Option<Vec<Network>>,
}

#[async_trait]
impl WifiAdapter for Iwd {
    /// Creates a new instance of the `Iwd` struct. Takes in a `zbus::Connection` to minimise the
    /// number of connections that need to be created, allowing one to be shared by the
    /// `Controller` between processes.
    async fn new(conn: Connection) -> Result<Self, NdError> {
        let service = "net.connman.iwd".to_string();

        match Self::find_adapter_path(&conn, &service).await {
            Ok(Some(path)) => Ok(Self {
                conn,
                service,
                path,
                networks: None,
            }),
            Err(e) => Err(NdError::AdapterNotFound(format!(
                "Could not find an adapter, is iwd installed?\n Error: {e}"
            ))),
            _ => Err(NdError::AdapterNotFound(
                "Could not find an adapter, is iwd installed?".to_string(),
            )),
        }
    }

    /// Connects to a network provided an SSID and passphrase.
    ///
    /// Since iwd does not allow connecting via BSSID the connection band is determined by signal
    /// strength internally by iwd, this can be tweaked in the `main.conf` iwd configuration file
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), NdError> {
        todo!()
    }

    /// Disconnects from the current WiFi network, does not remove the network
    async fn disconnect(&self) -> Result<(), NdError> {
        todo!()
    }

    /// Returns the current status of the connected WiFi network
    async fn status(&self) -> Result<String, NdError> {
        todo!()
    }

    /// Lists all networks which are available to be connected to including networks that are out
    /// of range
    async fn list_configured_networks(&self) -> Result<Vec<String>, NdError> {
        todo!()
    }

    /// Adds a network but does not connect to it; used by `connect_network` before it connects to
    /// a network
    async fn add_network(&self, ssid: &str, psk: &str) -> Result<(), NdError> {
        todo!()
    }

    /// Removes a network from the configured networks
    async fn remove_network(&self, ssid: &str) -> Result<(), NdError> {
        todo!()
    }
}

// Networking related
impl Iwd {
    /// Returns the object path of the iwd station, currently only returns the first station
    async fn find_adapter_path(
        conn: &Connection,
        service: &String,
    ) -> Result<Option<String>, NdError> {
        let proxy = ObjectManagerProxy::new(conn, service.clone(), "/").await?;
        for (path, interface) in proxy.get_managed_objects().await? {
            // BUG: if multiple adapters will just return first one
            if interface.contains_key("net.connman.iwd.Station") {
                return Ok(Some(path.to_string()));
            }
        }
        Ok(None)
    }

    /// Returns the value of a property found under the `self.path` interfaces
    /// Trait bounds follow from `zbus` downcast
    async fn get_prop<'b, T>(&self, subpath: &str, prop: &str) -> Result<T, NdError>
    where
        T: TryFrom<Value<'b>>,
        <T as TryFrom<Value<'b>>>::Error: Into<zbus::zvariant::Error>,
    {
        let mut interface_path = self.service.clone();
        interface_path.push_str(format!(".{}", subpath).as_str());
        let proxy = zbus::Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            interface_path.clone(),
        )
        .await?;

        match proxy.get_property(prop).await? {
            Some(val) => Ok(<zbus::zvariant::Value<'_> as Clone>::clone(&val)
                .downcast::<T>()
                .unwrap()),
            None => Err(NdError::PropertyNotFound(format!(
                "Could not find given property {} at {}",
                prop, interface_path
            ))),
        }
    }

    /// Returns the value of a property found under the `self.path` interfaces
    /// Proxy must be passed in, use this to reduce overhead
    /// Trait bounds follow from `zbus` downcast
    async fn get_prop_from_proxy<'b, T>(
        &self,
        proxy: &zbus::Proxy<'b>,
        prop: &str,
    ) -> Result<T, NdError>
    where
        T: TryFrom<Value<'b>>,
        <T as TryFrom<Value<'b>>>::Error: Into<zbus::zvariant::Error>,
    {
        match proxy.get_property(prop).await? {
            Some(val) => Ok(<zbus::zvariant::Value<'_> as Clone>::clone(&val)
                .downcast::<T>()
                .unwrap()),
            None => Err(NdError::PropertyNotFound(format!(
                "Could not find given property {} at {}",
                prop,
                proxy.path()
            ))),
        }
    }

    /// Performs a scan with iwd which internally updates the dbus to include new networks
    pub async fn get_all_networks(&mut self) -> Result<(), NdError> {
        let proxy = zbus::Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "org.freedesktop.DBus.Introspectable",
        )
        .await?;
        let introspect = proxy.introspect().await?;
        let mut xml = Reader::from_str(&introspect);
        xml.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut network_paths = Vec::<String>::new();
        loop {
            match xml.read_event_into(&mut buf) {
                // ap names are under 'node' in iwd dbus
                // self close tag triggers Empty event
                Ok(Event::Empty(ref e)) => {
                    if e.name().as_ref() == b"node" {
                        for attribute in e.attributes() {
                            let attr = attribute.unwrap();
                            if attr.key.as_ref() == b"name" {
                                network_paths.push(
                                    attr.decode_and_unescape_value(xml.decoder())
                                        .unwrap()
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => panic!("Error at position {}: {:?}", xml.buffer_position(), e),
                _ => (),
            }
        }

        let mut networks = vec![];
        for path in network_paths {
            let network = self.get_network_info(path).await?;
            networks.push(network);
        }
        self.networks = Some(networks);
        Ok(())
    }

    pub async fn get_network_info(&self, network: String) -> Result<Network, NdError> {
        let proxy = zbus::Proxy::new(
            &self.conn,
            self.service.clone(),
            format!("{}/{}", self.path.clone(), network),
            "net.connman.iwd.Network",
        )
        .await?;

        let ess = self
            .get_prop_from_proxy::<Vec<OwnedObjectPath>>(&proxy, "ExtendedServiceSet")
            .await?;

        let connected = self
            .get_prop_from_proxy::<bool>(&proxy, "Connected")
            .await?;

        let device = self
            .get_prop_from_proxy::<OwnedObjectPath>(&proxy, "Device")
            .await?;

        let known_network: Option<OwnedObjectPath>;
        match self
            .get_prop_from_proxy::<OwnedObjectPath>(&proxy, "KnownNetwork")
            .await
        {
            Ok(known) => {
                known_network = Some(known);
            }
            Err(_) => known_network = None,
        }

        let name = self.get_prop_from_proxy::<String>(&proxy, "Name").await?;

        let security: Security;
        match self
            .get_prop_from_proxy::<String>(&proxy, "Type")
            .await?
            .as_str()
        {
            "psk" => security = Security::Psk,
            "open" => security = Security::Open,
            "8021x" => security = Security::Ieee8021x,
            _ => {
                return Err(NdError::InvalidSecurityType);
            }
        }

        Ok(Network {
            ess,
            connected,
            device,
            known_network,
            name,
            security,
        })
    }
}

// Configuration related
impl Iwd {
    /// Returns either the location of main.conf if it has been created or a folder where it should
    /// be created.
    ///
    /// First, checks if `$CONFIGURATION_DIRECTORY` exists if so creates/finds main.conf
    /// If the env variable does not exist then checks if /etc/iwd exists if so creates/finds
    /// main.conf
    /// If /etc/iwd does not exist then creates the directory and main.conf
    async fn get_conf() -> Result<PathBuf, NdError> {
        let iwd_path = "/etc/iwd";
        let env_var = "CONFIGURATION_DIRECTORY";
        let dir = env::var(env_var);
        match dir {
            // found env
            Ok(v) => {
                let conf_path = format!("{}/main.conf", v);
                // not found conf
                if fs::metadata(v).await.is_err() {
                    let file = OpenOptions::new()
                        .write(true)
                        .read(true)
                        .create(true)
                        .open(&conf_path)
                        .await?;
                }
                return Ok(PathBuf::from(conf_path));
            }
            // no env
            Err(e) => {
                let conf_path = format!("{}/main.conf", iwd_path);
                if fs::metadata(&conf_path).await.is_err() {
                    // /etc/iwd could possibly not exist
                    fs::create_dir_all(iwd_path).await?;
                    OpenOptions::new()
                        .write(true)
                        .read(true)
                        .create(true)
                        .open(&conf_path)
                        .await?;
                }
                return Ok(PathBuf::from(conf_path));
            }
        }
    }
}

#[cfg(test)]
#[cfg(iwd_installed)]
mod tests {
    use zbus::zvariant::ObjectPath;

    use super::*;

    // Networking tests
    async fn setup() -> Result<Iwd, NdError> {
        let conn = zbus::Connection::system().await?;
        Ok(Iwd::new(conn).await?)
    }

    #[tokio::test]
    async fn test_get_connected_network() -> Result<(), NdError> {
        let iwd = setup().await?;
        iwd.get_prop::<ObjectPath>("Station", "ConnectedNetwork")
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_networks() -> Result<(), NdError> {
        let mut iwd = setup().await?;
        iwd.get_all_networks().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_network_info() -> Result<(), NdError> {
        let mut iwd = setup().await?;
        let _ = iwd.get_all_networks().await?;
        Ok(())
    }

    // configuration tests
    #[tokio::test]
    async fn test_get_conf_path() -> Result<(), NdError> {
        let path = Iwd::get_conf().await?;
        Ok(())
    }
}

pub struct Network {
    ess: Vec<OwnedObjectPath>,
    connected: bool,
    device: OwnedObjectPath,
    known_network: Option<OwnedObjectPath>,
    name: String,
    security: Security,
}

enum Security {
    Open,
    Psk,
    Ieee8021x,
}

enum IwdConfigGroup {
    General,
    Network,
    Blacklist,
    Rank,
    Scan,
}

enum GeneralSettings {
    EnableNetworkConfiguration(bool),
    AddressRandomization(AddrRandOpts),
    AddressRandomizationRange(AddrRandRangeOpts),
    // -100 to 1; default: -70
    RoamThreshold(i8),
    // default: -76
    RoamThreshold5G(i8),
    // default -80
    CriticalRoamThreshold(i8),
    // default: -82
    CriticalRoamThreshold5G(i8),
    RoamRetryInterval(u16),
    ManagementFrameProtection(ManagementFrameProtectionOpts),
}

enum AddrRandOpts {
    Disabled,
    Once,
    Network,
}

enum AddrRandRangeOpts {
    Full,
    Nic,
}

enum ManagementFrameProtectionOpts {
    Optional,
    Required,
    Disabled,
}

enum NetworkSettings {
    EnableIpv6(bool),
    NameResolvingService(NameResolver),
    // default: 300
    RoutePriorityOffset(u32),
}

enum NameResolver {
    Resolveconf,
    Systemd,
    None,
}

enum BlacklistSettings {
    // default: 60
    InitialTimeout(u32),
    // default: 30
    InitialAccessPointBusyTimeout(u32),
    // default: 30
    Multiplier(u32),
    // default: 86400
    MaximumTimeout(u32),
}

enum RankSettings {
    // band modif. default: 1.0
    BandModifier2_4Ghz(f32),
    BandModifier5Ghz(f32),
    BandModifier6Ghz(f32),
}

enum ScanSettings {
    DisablePeriodicScan(bool),
    // default: 10
    InitialPeriodicScanInterval(u32),
    // default: 300
    MaximumPeriodicScanInterval(u32),
    DisableRoamingScan(bool),
}
