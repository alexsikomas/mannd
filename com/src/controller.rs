use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::{mpsc::Sender, RwLock};

use crate::{
    error::ComError,
    netlink::Netlink,
    state::signals::{self, SignalUpdate},
    systemd::systemctl,
    wireless::{
        agent::{AgentState, IwdAgent},
        common::{AccessPoint, Security},
        iwd::Iwd,
        wpa_supplicant::WpaSupplicant,
    },
};
use tracing::{error, info};
use zbus::{proxy::SignalStream, Connection};

// Netlink not used here as I'm not implementing WPA authentication
#[derive(Debug)]
pub enum WirelessAdapter {
    Iwd(Iwd),
    Wpa(WpaSupplicant),
}

#[derive(Debug)]
pub struct Controller {
    pub wifi: Option<WirelessAdapter>,
    pub netlink: Netlink,
    connection: Option<Connection>,
}

impl Controller {
    pub async fn new() -> Result<Self, ComError> {
        // This is mostly a temporary connection
        let connection = Some(Connection::system().await?);

        Ok(Self {
            wifi: None,
            netlink: Netlink::connect_wireless().await?,
            connection,
        })
    }

    /// Sets wifi to be either iwd, wpa or netlink
    pub async fn determine_adapter(&mut self) {
        info!("Determining adapter");
        match &self.connection {
            Some(conn) => {
                let ctl = systemctl::Systemctl::new(conn.clone());
                match ctl.is_service_active("iwd".to_string()).await {
                    Some(v) => {
                        if v {
                            let _ = self.connect_iwd().await;
                            info!("iwd connected");
                            return;
                        }
                    }
                    _ => {}
                }

                // if each interface is managed induvidually this can be complex to
                // find in systemd through dbus, so we use a different method
                match WpaSupplicant::is_active(&conn).await {
                    Ok(v) => {
                        if v {
                            let _ = self.connect_wpa().await;
                            return;
                        }
                    }
                    _ => {}
                }
                tracing::error!("Neither iwd or wpa found!");
            }
            None => {
                tracing::error!("Connection has yet to be set in the controller!");
            }
        }
    }

    async fn connect_iwd(&mut self) -> Result<(), ComError> {
        let agent_state = Arc::new(RwLock::new(AgentState::new()));
        let conn = zbus::connection::Builder::system()?
            .serve_at("/org/mannd/IwdAgent", IwdAgent::new(agent_state.clone()))?
            .build()
            .await?;

        match Iwd::new(conn.clone(), agent_state.clone()).await {
            Ok(iwd) => {
                self.connection = Some(conn);
                self.wifi = Some(WirelessAdapter::Iwd(iwd));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn connect_wpa(&mut self) -> Result<(), ComError> {
        match &self.connection {
            Some(conn) => match WpaSupplicant::new(conn.clone()) {
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

    pub async fn scan<'a>(&mut self, signal_tx: Sender<SignalUpdate<'a>>) -> Result<(), ComError> {
        match &mut self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                match iwd.scan(signal_tx).await {
                    Ok(_) => {
                        info!("Scan succeeded.");
                    }
                    Err(com) => {
                        tracing::error!("There was an error while scanning!\n{}", com);
                    }
                }
                Ok(())
            }
            Some(WirelessAdapter::Wpa(wpa)) => {
                wpa.scan(signal_tx).await?;
                Ok(())
            }
            _ => Err(ComError::NetworkNotFound),
        }
    }
    pub async fn get_networks(&mut self) -> Result<Vec<AccessPoint>, ComError> {
        match &mut self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => match iwd.get_networks().await {
                Ok(v) => Ok(v),
                Err(e) => Err(ComError::OperationFailed(
                    "Error while getting scanned networks!".to_string(),
                )),
            },
            Some(WirelessAdapter::Wpa(wpa)) => match wpa.nearby_networks().await {
                Ok(v) => Ok(v),
                Err(e) => Err(ComError::OperationFailed(
                    "Error while getting scanned networks!".to_string(),
                )),
            },
            _ => Err(ComError::NetworkNotFound),
        }
    }

    pub async fn ssid_connect(
        &self,
        ssid: String,
        psk: String,
        security: Security,
    ) -> Result<(), ComError> {
        match &self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                iwd.connect_network(ssid, psk, security).await?;
            }
            Some(WirelessAdapter::Wpa(wpa)) => {
                wpa.connect_network(ssid, psk).await?;
            }
            None => {
                tracing::error!("Tried to connect to network without an initalised adapter?");
            }
        };
        Ok(())
    }

    pub async fn disconenct(&self) -> Result<(), ComError> {
        match &self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                iwd.disconnect().await?;
            }
            Some(WirelessAdapter::Wpa(wpa)) => {
                wpa.disconnect().await?;
            }
            None => {
                tracing::error!("Tried to disconnect but no wifi adapter was initalised.");
                return Err(ComError::OperationFailed(
                    "No adapter to be able to disconnect from networks".to_string(),
                ));
            }
        };
        Ok(())
    }

    pub async fn get_known_networks(&mut self) -> Result<Vec<AccessPoint>, ComError> {
        match &mut self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => match iwd.get_known_networks().await {
                Ok(aps) => {
                    return Ok(aps);
                }
                Err(e) => {
                    return Err(e);
                }
            },
            Some(WirelessAdapter::Wpa(wpa)) => {}
            None => {}
        }

        // temp while wpa not implemented
        Ok(vec![])
    }

    pub async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError> {
        match &self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => match iwd.remove_network(ssid, security).await {
                Ok(()) => {
                    return Ok(());
                }
                Err(e) => {
                    return Err(e);
                }
            },
            Some(WirelessAdapter::Wpa(wpa)) => {}
            None => {}
        }
        Ok(())
    }

    /// Performs cleanup before the application exits
    pub async fn exit(&self) -> Result<(), ComError> {
        match &self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                iwd.unregister_agent().await?;
            }
            Some(WirelessAdapter::Wpa(wpa)) => {}
            None => {}
        };
        Ok(())
    }

    pub async fn info(&self, ssid: String) -> Result<(), ComError> {
        Ok(())
    }
}

impl WirelessAdapter {
    pub fn daemon_type(&self) -> u32 {
        match self {
            WirelessAdapter::Iwd(_) => 1,
            WirelessAdapter::Wpa(_) => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new() -> Result<(), ComError> {
        let controller = Controller::new().await;
        match controller {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    #[cfg(iwd_installed)]
    #[tokio::test]
    async fn test_connect_iwd() -> Result<(), ComError> {
        let controller = Controller::new().await;
        match controller {
            Ok(mut cont) => match cont.connect_iwd().await {
                Ok(_) => Ok(()),
                Err(_) => {
                    println!("iwd is not found");
                    Ok(())
                }
            },
            Err(_) => Err(ComError::OperationFailed(
                "Controller could not be initalised".to_string(),
            )),
        }
    }
}
