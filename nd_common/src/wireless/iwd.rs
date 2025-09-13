use zbus::{fdo::ObjectManagerProxy, zvariant::Value, Connection};

use crate::{error::NdError, wireless::WifiAdapter};

pub struct Iwd {
    path: String,
    service: String,
    conn: Connection,
}

impl WifiAdapter for Iwd {
    async fn connect_network(&self, ssid: &str, psk: &str) -> Result<(), NdError> {
        todo!()
    }
    async fn disconnect(&self) {
        todo!()
    }
    async fn status(&self) -> String {
        todo!()
    }
    async fn list_configured_networks(&self) -> Vec<String> {
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
            if interface.contains_key("net.connman.iwd.Station") {
                return Ok(Some(path.to_string()));
            }
        }
        Ok(None)
    }

    /// Returns the value of a property found under the `self.path` interfaces
    /// Trait bounds follow from `zbus` downcast
    async fn get_station_property<'a, T>(&self, subpath: &str, prop: &str) -> Result<T, NdError>
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
}
