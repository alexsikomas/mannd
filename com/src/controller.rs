use crate::{
    error::ComError,
    netlink::WirelessNetlink,
    systemd::systemctl,
    wireless::{WifiAdapter, common::AccessPoint, iwd::Iwd, wpa_supplicant::WpaSupplicant},
};
use tracing::{error, info, instrument};
use zbus::Connection;

// Netlink not used here as I'm not implementing WPA authentication
#[derive(Debug)]
pub enum WirelessAdapter {
    Iwd(Iwd),
    Wpa(WpaSupplicant),
}

#[derive(Debug)]
pub struct Controller {
    pub wifi: Option<WirelessAdapter>,
    connection: Connection,
}

impl Controller {
    #[instrument]
    pub async fn new() -> Result<Self, ComError> {
        info!("Creating controller");
        let conn = zbus::Connection::system().await?;
        info!("Zbus connection successful");
        // let mut wired = WiredNetlink::connect().await?;

        Ok(Self {
            wifi: None,
            connection: conn,
        })
    }

    /// Sets wifi to be either iwd, wpa or netlink
    pub async fn determine_adapter(&mut self) {
        let ctl = systemctl::Systemctl::new(self.connection.clone());
        match ctl.is_service_active("iwd".to_string()).await {
            Some(v) => {
                if v {
                    self.connect_iwd().await;
                    return;
                }
            }
            _ => {}
        }

        match ctl.is_service_active("wpa_supplicant".to_string()).await {
            Some(v) => {
                if v {
                    self.connect_wpa().await;
                    return;
                }
            }
            _ => {}
        }
    }

    #[instrument]
    async fn connect_iwd(&mut self) -> Result<(), ComError> {
        info!("Attempting to setup iwd connection");
        let conn = self.connection.clone();
        match Iwd::new(conn).await {
            Ok(iwd) => {
                self.wifi = Some(WirelessAdapter::Iwd(iwd));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[instrument]
    async fn connect_wpa(&mut self) -> Result<(), ComError> {
        info!("Attempting to setup wpa connection");
        let conn = self.connection.clone();
        match WpaSupplicant::new(conn).await {
            Ok(wpa) => {
                self.wifi = Some(WirelessAdapter::Wpa(wpa));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn scan(&mut self) -> Result<(), ComError> {
        match &mut self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                iwd.scan().await?;
                Ok(())
            }
            _ => Err(ComError::NetworkNotFound),
        }
    }
    pub async fn get_networks(&mut self) -> Result<Vec<AccessPoint>, ComError> {
        match &mut self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                let imv = iwd.get_networks().await;
                match imv {
                    Ok(v) => Ok(v),
                    Err(e) => {
                        error!("ERROR! {:?}", e);
                        Err(ComError::NetworkNotFound)
                    }
                }
            }
            _ => Err(ComError::NetworkNotFound),
        }
    }

    async fn ssid_connect(&self, ssid: &str, psk: &str) -> Result<(), ComError> {
        match &self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                iwd.connect_network(ssid, psk).await?;
            }
            Some(WirelessAdapter::Wpa(wpa)) => {
                wpa.connect_network(ssid, psk).await?;
            }
            None => {
                info!("Tried to connect to network without an initalised adapter?");
            }
        };
        Ok(())
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
