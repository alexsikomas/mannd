use std::{io::Read, net};

use async_trait::async_trait;
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
    name::QName,
};
use zbus::{
    Connection,
    fdo::ObjectManagerProxy,
    zvariant::{ObjectPath, Value},
};

use crate::{error::NdError, wireless::WifiAdapter};

pub struct Iwd {
    path: String,
    service: String,
    conn: Connection,
}

#[async_trait]
impl WifiAdapter for Iwd {
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), NdError> {
        todo!()
    }
    async fn disconnect(&self) -> Result<(), NdError> {
        todo!()
    }
    async fn status(&self) -> Result<String, NdError> {
        todo!()
    }
    async fn list_configured_networks(&self) -> Result<Vec<String>, NdError> {
        todo!()
    }
    async fn add_network(&self, ssid: &str, psk: &str) -> Result<(), NdError> {
        todo!()
    }
    async fn remove_network(&self, ssid: &str) -> Result<(), NdError> {
        todo!()
    }
}

impl Iwd {
    pub async fn new(conn: zbus::Connection) -> Result<Self, NdError> {
        let service = "net.connman.iwd".to_string();

        match Self::find_adapter_path(&conn, &service).await {
            Ok(Some(path)) => Ok(Self {
                conn,
                service,
                path,
            }),
            Err(e) => Err(NdError::AdapterNotFound(format!(
                "Log: {e}, Could not find an adapter, is iwd installed?"
            ))),
            _ => Err(NdError::AdapterNotFound(
                "Could not find an adapter, is iwd installed?".to_string(),
            )),
        }
    }
    /// Gets the object path of the iwd station
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
    async fn get_prop<'a, T>(&self, subpath: &str, prop: &str) -> Result<T, NdError>
    where
        T: TryFrom<Value<'a>>,
        <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
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

    async fn get_prop_from_proxy<'a, T>(
        &self,
        proxy: &zbus::Proxy<'a>,
        prop: &str,
    ) -> Result<T, NdError>
    where
        T: TryFrom<Value<'a>>,
        <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
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

    pub async fn get_all_networks(&self) -> Result<Vec<Network>, NdError> {
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
        Ok(networks)
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
            .get_prop_from_proxy::<Vec<ObjectPath>>(&proxy, "ExtendedServiceSet")
            .await?;

        let connected = self
            .get_prop_from_proxy::<bool>(&proxy, "Connected")
            .await?;

        let device = self
            .get_prop_from_proxy::<ObjectPath>(&proxy, "Device")
            .await?;

        let known_network: Option<ObjectPath>;
        match self
            .get_prop_from_proxy::<ObjectPath>(&proxy, "KnownNetwork")
            .await
        {
            Ok(known) => {
                known_network = Some(known);
            }
            Err(_) => known_network = None,
        }

        let name = self.get_prop_from_proxy::<String>(&proxy, "Name").await?;

        let mut security: Security;
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

#[cfg(test)]
#[cfg(iwd_installed)]
mod tests {
    use zbus::zvariant::ObjectPath;

    use super::*;

    async fn setup() -> Result<Iwd, NdError> {
        let conn = zbus::Connection::system().await?;
        Ok(Iwd::new(conn).await?)
    }

    #[tokio::test]
    async fn test_get_station_networks() -> Result<(), NdError> {
        let iwd = setup().await?;
        iwd.get_prop::<ObjectPath>("Station", "ConnectedNetwork")
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_networks() -> Result<(), NdError> {
        let iwd = setup().await?;
        iwd.get_all_networks().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_network_info() -> Result<(), NdError> {
        let iwd = setup().await?;
        let networks = iwd.get_all_networks().await?;
        Ok(())
    }
}

pub struct Network<'a> {
    ess: Vec<ObjectPath<'a>>,
    connected: bool,
    device: ObjectPath<'a>,
    known_network: Option<ObjectPath<'a>>,
    name: String,
    security: Security,
}

enum Security {
    Open,
    Psk,
    Ieee8021x,
}
