use std::sync::Arc;
use tokio::sync::{mpsc::Sender, RwLock};

use crate::{
    error::ComError,
    state::{
        network::{ApConnectInfo, Credentials},
        signals::SignalUpdate,
    },
    systemd::systemctl,
    wireless::{
        agent::{AgentState, IwdAgent},
        common::{AccessPoint, Security},
        iwd::Iwd,
        wpa_supplicant::WpaSupplicant,
    },
};
use tracing::info;
use zbus::Connection;

// used in outside functions
#[derive(Debug, Clone)]
pub enum DaemonType {
    Iwd,
    Wpa,
}

// used for matching in controller
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

// Initialisations
impl Controller {
    pub async fn new() -> Result<Self, ComError> {
        let connection = Connection::system().await?;

        Ok(Self {
            wifi: None,
            connection,
        })
    }

    /// Sets wifi to be either iwd, wpa or netlink
    pub async fn determine_adapter(&mut self) {
        info!("Determining adapter");
        let ctl = systemctl::Systemctl::new(self.connection.clone());
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
        match WpaSupplicant::is_active(&self.connection).await {
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

    async fn connect_iwd(&mut self) -> Result<(), ComError> {
        let agent_state = Arc::new(RwLock::new(AgentState::new()));
        let conn = zbus::connection::Builder::system()?
            .serve_at("/org/mannd/IwdAgent", IwdAgent::new(agent_state.clone()))?
            .build()
            .await?;

        match Iwd::new(conn.clone(), agent_state.clone()).await {
            Ok(iwd) => {
                self.connection = conn;
                iwd.register_agent().await?;
                self.wifi = Some(WirelessAdapter::Iwd(iwd));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn connect_wpa(&mut self) -> Result<(), ComError> {
        match WpaSupplicant::new(self.connection.clone()) {
            Ok(wpa) => {
                self.wifi = Some(WirelessAdapter::Wpa(wpa));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

// run actions
impl Controller {
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

    pub async fn network_connect(&self, info: ApConnectInfo) -> Result<(), ComError> {
        match info.credentials {
            Credentials::Password(psk) => {
                match &self.wifi {
                    Some(WirelessAdapter::Iwd(iwd)) => {
                        iwd.connect_network_psk(info.ssid, psk).await?;
                    }
                    Some(WirelessAdapter::Wpa(wpa)) => {
                        wpa.connect_network_psk(info.ssid, psk).await?;
                    }
                    None => {
                        tracing::error!(
                            "Tried to connect to network without an initalised adapter?"
                        );
                    }
                };
            }
            Credentials::Eap(eap) => {
                match &self.wifi {
                    Some(WirelessAdapter::Iwd(iwd)) => {
                        iwd.connect_network_eap(info.ssid, eap).await?;
                    }
                    Some(WirelessAdapter::Wpa(wpa)) => {
                        wpa.connect_network_eap(info.ssid, eap).await?;
                    }
                    None => {
                        tracing::error!(
                            "Tried to connect to network without an initalised adapter?"
                        );
                    }
                };
            }
        }

        Ok(())
    }

    /// security is required for iwd due to the way it stores network names
    pub async fn connect_known(&self, ssid: String, security: Security) -> Result<(), ComError> {
        match &self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => {
                iwd.connect_known(ssid, security).await?;
            }
            Some(WirelessAdapter::Wpa(_wpa)) => {}
            None => {}
        }
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

    pub async fn remove_network(&self, ssid: String, security: Security) -> Result<(), ComError> {
        info!("Removing network");
        match &self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => match iwd.remove_network(ssid, security).await {
                Ok(()) => {
                    return Ok(());
                }
                Err(e) => {
                    return Err(e);
                }
            },
            Some(WirelessAdapter::Wpa(_wpa)) => {}
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
            Some(WirelessAdapter::Wpa(_wpa)) => {}
            None => {}
        };
        Ok(())
    }
}

// get information
impl Controller {
    pub async fn get_all_networks(&mut self) -> Result<Vec<AccessPoint>, ComError> {
        match &mut self.wifi {
            Some(WirelessAdapter::Iwd(iwd)) => match iwd.nearby_networks().await {
                Ok(v) => Ok(v),
                Err(_) => Err(ComError::OperationFailed(
                    "Error while getting scanned networks!".to_string(),
                )),
            },
            Some(WirelessAdapter::Wpa(wpa)) => match wpa.nearby_networks().await {
                Ok(v) => Ok(v),
                Err(_) => Err(ComError::OperationFailed(
                    "Error while getting scanned networks!".to_string(),
                )),
            },
            _ => Err(ComError::NetworkNotFound),
        }
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
            Some(WirelessAdapter::Wpa(_wpa)) => {}
            None => {}
        }

        // temp while wpa not implemented
        Ok(vec![])
    }

    pub fn daemon_type(&self) -> Option<DaemonType> {
        match self.wifi {
            Some(WirelessAdapter::Iwd(_)) => Some(DaemonType::Iwd),
            Some(WirelessAdapter::Wpa(_)) => Some(DaemonType::Wpa),
            _ => None,
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
