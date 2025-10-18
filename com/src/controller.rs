use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    error::ComError,
    netlink::WirelessNetlink,
    systemd::systemctl,
    wireless::{
        WifiAdapter,
        agent::{AgentState, IwdAgent},
        common::AccessPoint,
        iwd::Iwd,
        wpa_supplicant::WpaSupplicant,
    },
};
use tracing::{error, info, instrument};
use zbus::{Connection, conn::Builder};

// Netlink not used here as I'm not implementing WPA authentication
#[derive(Debug)]
pub enum WirelessAdapter {
    Iwd(Iwd),
    Wpa(WpaSupplicant),
}

#[derive(Debug)]
pub struct Controller {
    pub wifi: Option<WirelessAdapter>,
    connection: Option<Connection>,
}

impl Controller {
    #[instrument]
    pub async fn new() -> Result<Self, ComError> {
        // This is mostly a temporary connection
        let connection = Some(Connection::system().await?);

        Ok(Self {
            wifi: None,
            connection,
        })
    }

    /// Sets wifi to be either iwd, wpa or netlink
    pub async fn determine_adapter(&mut self) {
        match &self.connection {
            Some(conn) => {
                let ctl = systemctl::Systemctl::new(conn.clone());
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
            None => {
                tracing::error!("Connection has yet to be set in the controller!");
            }
        }
    }

    #[instrument]
    async fn connect_iwd(&mut self) -> Result<(), ComError> {
        let agent_state = Arc::new(RwLock::new(AgentState::new()));
        let conn = zbus::connection::Builder::system()?
            .serve_at("/org/mannd/IwdAgent", IwdAgent::new(agent_state.clone()))?
            .build()
            .await?;

        match Iwd::new(conn.clone(), agent_state.clone()).await {
            Ok(iwd) => {
                self.connection = Some(conn);
                iwd.register_agent().await?;
                self.wifi = Some(WirelessAdapter::Iwd(iwd));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[instrument]
    async fn connect_wpa(&mut self) -> Result<(), ComError> {
        match &self.connection {
            Some(conn) => match WpaSupplicant::new(conn.clone()).await {
                Ok(wpa) => {
                    self.wifi = Some(WirelessAdapter::Wpa(wpa));
                    Ok(())
                }
                Err(e) => Err(e),
            },
            None => {
                tracing::error!("wpa_supplicant attempted to connect without a ZBus connection!");
                Err(ComError::OperationFailed(
                    "No ZBus Connection, check the logs.".to_string(),
                ))
            }
        }
    }

    pub async fn scan(&mut self) -> Result<(), ComError> {
        match &mut self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                match iwd.scan().await {
                    Ok(_) => {
                        info!("Scan succeeded.");
                    }
                    Err(com) => {
                        tracing::error!("There was an error while scanning!\n{}", com);
                    }
                }
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

    pub async fn ssid_connect(&self, ssid: String, psk: String) -> Result<(), ComError> {
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
