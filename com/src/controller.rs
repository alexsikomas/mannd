use crate::{
    error::ComError,
    netlink::{WiredNetlink, WirelessNetlink},
    wireless::{WifiAdapter, iwd::Iwd, wpa_supplicant::WpaSupplicant},
};
use tracing::{info, instrument};
use zbus::Connection;

#[derive(Debug)]
pub struct Controller {
    // Wireless Daemons
    wifi: Option<Box<dyn WifiAdapter + Send + Sync>>,
    /// Used for ethernet and wireless information iwd/wpa don't provide
    nl_wifi: Option<WirelessNetlink>,
    nl_wired: Option<WiredNetlink>,
    connection: Connection,
}

impl Controller {
    #[instrument]
    async fn new() -> Result<Self, ComError> {
        info!("Creating controller");
        let conn = zbus::Connection::system().await?;
        info!("Zbus connection successful");
        let nl_wifi = WirelessNetlink::connect().await?;
        info!("Netlink connection successful");
        // let mut nl_wired = WiredNetlink::connect().await?;

        // Init wifi later
        Ok(Self {
            wifi: None,
            nl_wifi: Some(nl_wifi),
            nl_wired: None,
            connection: conn,
        })
    }

    #[instrument]
    async fn connect_iwd(&mut self) -> Result<(), ComError> {
        info!("Attempting to setup iwd connection in controller");
        let conn = self.connection.clone();
        match Iwd::new(conn).await {
            Ok(iwd) => {
                self.wifi = Some(Box::new(iwd));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn ssid_connect(&self, ssid: &str, psk: &str) -> Result<(), ComError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new() -> Result<(), ComError> {
        let controller = Controller::new().await;
        match controller {
            Ok(val) => Ok(()),
            Err(e) => Err(ComError::OperationFailed("Test".to_string())),
        }
    }

    #[cfg(iwd_installed)]
    #[tokio::test]
    async fn test_connect_iwd() -> Result<(), ComError> {
        let mut controller = Controller::new().await;
        match controller {
            Ok(mut cont) => match cont.connect_iwd().await {
                Ok(iwd) => Ok(()),
                Err(e) => Err(ComError::OperationFailed("iwd not found".to_string())),
            },
            Err(e) => Err(ComError::OperationFailed(
                "Controller could not be initalised".to_string(),
            )),
        }
    }
}
