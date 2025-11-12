//! Reference: https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network
use std::collections::HashMap;

use async_trait::async_trait;
use zbus::{
    zvariant::{Dict, OwnedValue, Str, Value},
    Connection, Proxy,
};

use crate::{
    error::ComError,
    wireless::{
        common::{get_prop_from_proxy, Security},
        WifiAdapter,
    },
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {
    service: String,
    path: String,
    conn: Connection,
}

#[async_trait]
impl WifiAdapter for WpaSupplicant {
    async fn connect_network(
        &self,
        ssid: String,
        psk: String,
        security: Security,
    ) -> Result<(), ComError> {
        todo!()
    }
    async fn disconnect(&self) -> Result<(), ComError> {
        todo!()
    }
    async fn status(&self) -> Result<String, ComError> {
        todo!()
    }
    async fn list_configured_networks(&self) -> Result<Vec<String>, ComError> {
        todo!()
    }
    async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError> {
        todo!()
    }
}

impl WpaSupplicant {
    pub fn new(conn: Connection) -> Result<Self, ComError> {
        let service = String::from("fi.w1.wpa_supplicant1");
        let path = String::from("/fi/w1/wpa_supplicant1/Interfaces/0");
        Ok(Self {
            conn,
            service,
            path,
        })
    }
    pub async fn scan(&self) -> Result<(), ComError> {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;
        let mut dict: HashMap<String, OwnedValue> = HashMap::new();

        dict.insert("Type".to_string(), Value::new("active").try_to_owned()?);
        proxy.call_noreply("Scan", &dict).await?;
        Ok(())
    }

    pub async fn get_interface_prop<'a, T>(&self, prop: &'static str) -> Result<T, ComError>
    where
        T: TryFrom<Value<'a>>,
        <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
    {
        let proxy = Proxy::new(
            &self.conn,
            self.service.clone(),
            self.path.clone(),
            "fi.w1.wpa_supplicant1.Interface",
        )
        .await?;
        return Ok(get_prop_from_proxy::<T>(&proxy, prop).await?);
    }
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wpa_scan() -> Result<(), ComError> {
        let conn = Connection::system().await.unwrap();
        let wpa = WpaSupplicant::new(conn)?;
        let mac = wpa.get_interface_prop::<Vec<u8>>("MACAddress").await?;
        println!("{:?}", mac);

        wpa.scan().await?;
        Ok(())
    }
}
