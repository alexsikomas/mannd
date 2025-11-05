//! Reference: https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network

use async_trait::async_trait;
use zbus::{Connection, Proxy};

use crate::{
    error::ComError,
    wireless::{
        common::{get_prop_from_proxy, Security},
        WifiAdapter,
    },
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {
    conn: Connection,
    service: String,
    path: String,
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
        let path = String::from("/fi/w1/wpa_supplicant1");
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
            format!("{}.Interface", self.service.clone()),
        )
        .await?;

        proxy.call::<_, _, ()>("Scan", &()).await?;
        Ok(())
    }
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wpa_scan() -> Result<(), ComError> {
        let conn = Connection::system().await.unwrap();
        let wpa = WpaSupplicant::new(conn)?;
        wpa.scan().await?;
        Ok(())
    }
}
