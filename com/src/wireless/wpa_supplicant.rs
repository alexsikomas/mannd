//! Reference: https://w1.fi/wpa_supplicant/devel/dbus.html#dbus_network

use std::ffi::CString;

use async_trait::async_trait;
use zbus::{Connection, Proxy};

use crate::{
    error::ComError,
    wireless::{
        common::{get_prop_from_proxy, Security},
        WifiAdapter,
    },
    wpa_ctrl_open,
};

#[derive(Debug, Clone)]
pub struct WpaSupplicant {}

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
    pub fn new() -> Result<Self, ComError> {
        println!("INSIDE OF NEW WPA");
        let wpa_socket_path =
            CString::new("/run/wpa_supplicant/").expect("Could not make wpa CString");

        unsafe {
            let mut wpa_ctrl = wpa_ctrl_open(wpa_socket_path.as_ptr());
            println!("{:?}", wpa_ctrl);
        }
        Ok(Self {})
    }
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wpa_scan() -> Result<(), ComError> {
        // let conn = Connection::system().await.unwrap();
        let wpa = WpaSupplicant::new()?;
        // wpa.scan().await?;
        Ok(())
    }
}
