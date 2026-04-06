//! # Controller
//!
//! Syncronises the various networking backends.
//!
//! ## Backends
//! - General Wi-Fi
//!     - [iwd](crate::wireless::iwd)
//!     - [wpa_supplicant](crate::wireless::wpa_supplicant)
//! - Other
//!     - [WireGuard](crate::wireguard)

use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};
use tokio::sync::{RwLock, mpsc::Sender};

use crate::{
    error::ManndError,
    netlink::NlRouterWrapper,
    read_global,
    state::signals::SignalUpdate,
    store::{NetworkInfo, WgMeta, WpaState},
    systemd::systemctl::is_service_active,
    wireguard::network::Wireguard,
    wireless::{
        agent::{AgentState, IwdAgent},
        iwd::Iwd,
        wpa_config::WpaConfig,
        wpa_supplicant::WpaSupplicant,
    },
};
use tracing::{info, instrument};
use zbus::Connection;

/// Used for matching when we don't have the full data
/// or don't want to send the full data like [`Capabilities`](crate::state::network::Capability)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WifiDaemonType {
    Iwd,
    Wpa,
}

#[derive(Debug)]
pub enum WirelessAdapter {
    Iwd(Iwd),
    Wpa(WpaSupplicant),
}

#[derive(Debug)]
pub struct Controller {
    pub wifi: Option<WirelessAdapter>,
    wg: Option<Wireguard<NlRouterWrapper>>,
    connection: Connection,
}

macro_rules! dispatch_wifi {
    ($wifi:expr, $iwd:ident => $iwd_expr:expr, $wpa:ident => $wpa_expr:expr, $none:expr) => {
        match $wifi {
            Some(WirelessAdapter::Iwd($iwd)) => $iwd_expr,
            Some(WirelessAdapter::Wpa($wpa)) => $wpa_expr,
            None => $none,
        }
    };
}

// Initialisations
impl Controller {
    #[instrument(err)]
    pub async fn new() -> Result<Self, ManndError> {
        let connection = Connection::system().await?;
        Ok(Self {
            wifi: None,
            wg: None,
            connection,
        })
    }

    /// Tries to connect to Wi-Fi adapter, does not emit
    /// error instead does a tracing::warn!()
    pub async fn connect_wifi_adapter(&mut self) {
        let mut opt = is_service_active(&self.connection, "iwd").await;
        if opt.is_some_and(|v| v) {
            match self.connect_iwd().await {
                Ok(()) => info!("Wi-Fi Daemon Connected: iwd"),
                Err(e) => tracing::error!("Failed to init iwd: {e}"),
            }
            return;
        }

        opt = is_service_active(&self.connection, "wpa_supplicant").await;
        if opt.is_some_and(|v| v) {
            match self.connect_wpa().await {
                Ok(()) => info!("Wi-Fi Daemon Connected: wpa_supplicant"),
                Err(e) => tracing::error!("Failed to init wpa_supplicant: {e}"),
            }
            return;
        }

        tracing::warn!("Could not connect to any Wi-Fi daemon.");
    }

    /// Connects to the `iwd` Wi-Fi adapter, if found and sets
    /// up an agent dbus for psk sharing
    #[instrument(err, skip(self))]
    async fn connect_iwd(&mut self) -> Result<(), ManndError> {
        let agent_state = Arc::new(RwLock::new(AgentState::default()));
        let conn = zbus::connection::Builder::system()?
            .serve_at("/org/mannd/IwdAgent", IwdAgent::new(agent_state.clone()))?
            .build()
            .await?;

        let mut iwd = Iwd::new(conn.clone(), agent_state.clone()).await?;
        self.connection = conn;
        iwd.register_agent().await?;
        iwd.sync_networks().await?;
        self.wifi = Some(WirelessAdapter::Iwd(iwd));
        Ok(())
    }

    #[instrument(err, skip(self))]
    async fn connect_wpa(&mut self) -> Result<(), ManndError> {
        let wpa_state = read_global(|state| state.db.get_wpa_state())
            .transpose()?
            .unwrap_or(WpaState::default());

        let config = WpaConfig::load_or_default()?;
        let mut wpa = WpaSupplicant::new(config, wpa_state, self.connection.clone()).await?;

        if let Err(e) = wpa.refresh_networks(true).await {
            tracing::warn!("WPA initalised but network sync failed: {e}");
        }

        self.wifi = Some(WirelessAdapter::Wpa(wpa));
        Ok(())
    }

    #[instrument(err, skip(self))]
    /// Starts the wireguard netlink interface, sets the status
    /// down to not ruin internet connectivity
    pub async fn start_wireguard(&mut self) -> Result<(), ManndError> {
        let wg = Wireguard::new().await?;
        self.wg = Some(wg);
        Ok(())
    }

    #[instrument(err, skip(self))]
    /// Updates the wireguad files in the state database
    pub fn update_wireguard_state(&self) -> Result<(Vec<String>, Vec<WgMeta>), ManndError> {
        read_global(|state| state.db.write_wg_files()).transpose()?;
        read_global(|state| state.db.ordered_wg_files()).ok_or(ManndError::OperationFailed(
            "Failed to get ordered WireGuard files".into(),
        ))?
    }

    #[instrument(err, skip(self))]
    pub async fn connect_wireguard_conf(&self, file: &Path) -> Result<(), ManndError> {
        match &self.wg {
            Some(wg) => {
                wg.connect_conf(file)?;
                Ok(())
            }
            _ => Err(ManndError::WgAccess),
        }
    }

    pub async fn networkd_status(&self) -> bool {
        is_service_active(&self.connection, "systemd-networkd")
            .await
            .unwrap_or(false)
    }
}

// run actions
impl Controller {
    #[instrument(err, skip(self))]
    pub async fn scan<'a>(&mut self, sock_tx: Sender<SignalUpdate<'a>>) -> Result<(), ManndError> {
        let res = dispatch_wifi!(
            &mut self.wifi,
            iwd => iwd.scan(sock_tx).await,
            wpa => wpa.scan(sock_tx).await,
            Err(ManndError::OperationFailed("No wifi daemon found".into()))
        );

        match &res {
            Ok(()) => info!("Scan succeeded"),
            Err(e) => tracing::error!("Error occured while scanning\n{e}"),
        }

        res
    }

    #[instrument(err, skip(self))]
    pub async fn connect_network(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        dispatch_wifi!(&self.wifi,
        iwd => iwd.connect_network(network).await,
        wpa => wpa.connect_network(network).await,
        {
            tracing::error!("No wireless daemon found.");
            Ok(())
        })
    }

    /// security required for iwd due to the way it stores network names
    #[instrument(err, skip(self))]
    pub async fn connect_known(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        dispatch_wifi!(&self.wifi,
        iwd => iwd.connect_network(network).await,
        wpa => wpa.connect_network(network).await,
        {
            tracing::error!("No wireless daemon found");
            Ok(())
        })
    }

    #[instrument(err, skip(self))]
    pub async fn disconnect_network(&self) -> Result<(), ManndError> {
        dispatch_wifi!(&self.wifi,
        iwd => iwd.disconnect().await,
        wpa => wpa.disconnect().await,
        {
            tracing::error!("Tried to disconnect but no wifi adapter was initalised.");
            return Err(ManndError::OperationFailed(
                "No adapter to be able to disconnect from networks".to_string(),
            ));
        })
    }

    #[instrument(err, skip(self))]
    pub async fn remove_network(&self, network: &NetworkInfo) -> Result<(), ManndError> {
        dispatch_wifi!(&self.wifi,
            iwd => iwd.remove_network(network).await,
            wpa => wpa.remove_network(network).await,
            Ok(())
        )
    }

    #[instrument(err, skip(self))]
    pub async fn remove_wireguard_iface(&mut self) -> Result<(), ManndError> {
        match &mut self.wg {
            Some(wg) => {
                wg.delete_interface().await?;
                self.wg = None;
                Ok(())
            }
            None => Err(ManndError::WgAccess),
        }
    }

    /// Performs cleanup before the app exits
    #[instrument(err, skip(self))]
    pub async fn exit(&self) -> Result<(), ManndError> {
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
    #[instrument(err, skip(self))]
    pub async fn get_all_networks(&mut self) -> Result<Vec<NetworkInfo>, ManndError> {
        dispatch_wifi!(&mut self.wifi,
            iwd => iwd.sync_networks().await,
            wpa => wpa.refresh_networks(false).await,
            Err(ManndError::NetworkNotFound))
    }

    #[instrument(err, skip(self))]
    pub async fn sync_all_networks(&mut self) -> Result<Vec<NetworkInfo>, ManndError> {
        dispatch_wifi!(&mut self.wifi,
            iwd => iwd.sync_networks().await,
            wpa => wpa.refresh_networks(true).await,
            Err(ManndError::NetworkNotFound))
    }

    pub fn get_wifi_daemon_type(&self) -> Option<WifiDaemonType> {
        match self.wifi {
            Some(WirelessAdapter::Iwd(_)) => Some(WifiDaemonType::Iwd),
            Some(WirelessAdapter::Wpa(_)) => Some(WifiDaemonType::Wpa),
            _ => None,
        }
    }

    pub fn is_wireguard_connected(&self) -> bool {
        self.wg.is_some()
    }
}

impl Controller {
    // wpa only
    #[instrument(err, skip(self))]
    pub async fn wpa_create_interface(&mut self, ifname: &str) -> Result<(), ManndError> {
        if let Some(WirelessAdapter::Wpa(wpa)) = &mut self.wifi {
            wpa.create_interface(ifname).await?;

            return Ok(());
        }

        Err(ManndError::OperationFailed(
            "Tried to use wpa only method while iwd active.".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[instrument(err)]
    async fn test_new() -> Result<(), ManndError> {
        let controller = Controller::new().await;
        match controller {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}
