use std::process::Command;

use tokio::sync::mpsc::Sender;
use tracing::{info, instrument};

use crate::{
    controller::{Controller, WirelessAdapter},
    error::ManndError,
    read_global,
    state::{
        messages::{
            Capability, Failure, NetworkAction, NetworkState, Process, Started, Success,
            WifiAction, WireguardAction, WireguardCapability, WpaAction,
        },
        signals::{SignalManager, SignalUpdate},
    },
};

#[derive(Debug)]
pub struct NetworkActor<'a> {
    pub controller: Controller,
    pub signal_manager: SignalManager<'a>,
    signal_tx: Sender<SignalUpdate<'a>>,
    sock_tx: Sender<NetworkState>,
}

impl<'a> NetworkActor<'a> {
    #[instrument(err)]
    pub async fn new(
        signal_tx: Sender<SignalUpdate<'a>>,
        sock_tx: Sender<NetworkState>,
    ) -> Result<Self, ManndError> {
        let mut controller = Controller::new().await?;
        if read_global(|state| state.app.wg_running).unwrap_or(false) {
            let _ = controller.start_wireguard().await;
        }

        controller.connect_wifi_adapter().await;
        let signal_manager = SignalManager::new();
        Ok(Self {
            controller,
            signal_manager,
            signal_tx,
            sock_tx,
        })
    }

    /// Returns true if we are quitting the application
    #[instrument(err, skip(self))]
    pub async fn handle_action(&mut self, action: NetworkAction) -> Result<bool, ManndError> {
        let mut state_send: Vec<NetworkState> = vec![];

        match action {
            NetworkAction::GetCapabilities => {
                let wifi_daemon = self.controller.get_wifi_daemon_type();
                let networkd_active = self.controller.networkd_status().await;
                let wg_installed = Command::new("wg")
                    .arg("--version")
                    .output()
                    .map_or(false, |_| true);

                // TODO: check state
                let caps = Capability::new(
                    wifi_daemon,
                    networkd_active,
                    WireguardCapability::new(wg_installed),
                );
                state_send.push(NetworkState::SetCapabilities(caps));
            }
            NetworkAction::Wifi(wifi_action) => {
                self.handle_wifi_action(&wifi_action, &mut state_send)
                    .await?;
            }
            NetworkAction::Wireguard(wg_action) => {
                self.handle_wireguard_action(&wg_action, &mut state_send)
                    .await?;
            }
            NetworkAction::Wpa(wpa_action) => {
                self.handle_wpa_action(&wpa_action, &mut state_send).await?;
            }
            // WIREGUARD
            NetworkAction::Exit => {
                if let Ok(()) = self.controller.exit().await {
                    return Ok(true);
                }
            }
            _ => {}
        };

        for req in state_send {
            let _ = self.sock_tx.send(req).await;
        }

        Ok(false)
    }

    async fn handle_wifi_action(
        &mut self,
        action: &WifiAction,
        state_send: &mut Vec<NetworkState>,
    ) -> Result<(), ManndError> {
        let mut should_full_refresh = false;

        match action {
            WifiAction::Scan => {
                state_send.push(NetworkState::Start(Started(Process::WifiScan)));

                if let Err(e) = self.controller.scan(self.signal_tx.clone()).await {
                    tracing::error!("[Wi-Fi]: Scan failed. {e:?}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::WifiScan,
                        e.to_string(),
                    )));
                }
            }
            WifiAction::GetNetworks => match self.controller.get_all_networks().await {
                Ok(aps) => {
                    state_send.push(NetworkState::SetNetworks(aps));
                    state_send.push(NetworkState::Success(Success::Generic));
                }
                Err(e) => {
                    tracing::error!("[Wi-Fi]: Failed to get networks. {e:?}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::WifiScan,
                        e.to_string(),
                    )));
                }
            },
            WifiAction::Connect(info) => {
                state_send.push(NetworkState::Start(Started(Process::WifiConnect)));
                match self.controller.connect_network(info).await {
                    Ok(()) => {
                        info!("Connection to network was successful");
                        should_full_refresh = true;
                        state_send.push(NetworkState::Success(Success::Generic));
                    }
                    Err(e) => {
                        tracing::error!("[Wi-Fi]: Connection to network was not successful. {e:?}");
                        state_send.push(NetworkState::Failed(Failure::new(
                            Process::WifiConnect,
                            e.to_string(),
                        )));
                    }
                }
            }
            WifiAction::ConnectKnown(network) => match self.controller.connect_known(network).await
            {
                Ok(()) => {
                    should_full_refresh = true;
                }
                Err(e) => {
                    tracing::warn!("[Wi-Fi]: Could not connect to a known network. Error: {e:?}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::WifiConnect,
                        e.to_string(),
                    )));
                }
            },
            WifiAction::Disconnect => match self.controller.disconnect_network().await {
                Ok(()) => {
                    info!("[Wi-Fi]: Disconnected from active network");
                    should_full_refresh = true;
                }
                Err(e) => {
                    tracing::error!("[Wi-Fi]: Disconnect failed. {e:?}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::Generic,
                        e.to_string(),
                    )));
                }
            },
            WifiAction::Forget(network) => match self.controller.remove_network(network).await {
                Ok(()) => {
                    info!("[Wi-Fi]: Network {} forgotten", network.ssid);
                    should_full_refresh = true;
                }
                Err(e) => {
                    tracing::error!("[Wi-Fi]: Failed to forget network {}. {e:?}", network.ssid);
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::Generic,
                        e.to_string(),
                    )));
                }
            },
        };

        if should_full_refresh {
            match self.controller.sync_all_networks().await {
                Ok(networks) => state_send.push(NetworkState::SetNetworks(networks)),
                Err(e) => {
                    tracing::error!("[Wi-Fi]: Failed to refresh networks after action. {e:?}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::Generic,
                        e.to_string(),
                    )));
                }
            }
        }

        Ok(())
    }

    /// Gets wireguard info no matter the `WireguardAction` to simplify logic and remove
    /// repititions
    async fn handle_wireguard_action(
        &mut self,
        action: &WireguardAction,
        state_send: &mut Vec<NetworkState>,
    ) -> Result<(), ManndError> {
        match action {
            WireguardAction::GetInfo => {
                // the exact GetInfo procedure is done at the end
            }
            WireguardAction::Connect(file) => {
                match self.controller.connect_wireguard_conf(file).await {
                    Ok(_res) => {
                        info!("[Wireguard]: Successfully connection to a configuration.");
                    }
                    Err(e) => {
                        tracing::error!("{e:?}");
                        return Err(e);
                    }
                }
            }
            WireguardAction::Toggle => {
                if !self.controller.is_wireguard_connected() {
                    if let Ok(()) = self.controller.start_wireguard().await {}
                } else {
                    if let Ok(()) = self.controller.remove_wireguard_iface().await {}
                }
            }
        }

        if let Ok((names, meta)) = self.controller.update_wireguard_state() {
            let active = self.controller.is_wireguard_connected();
            state_send.push(NetworkState::SetWireguardInfo {
                names,
                meta,
                active,
            });
        }

        Ok(())
    }

    async fn handle_wpa_action(
        &mut self,
        action: &WpaAction,
        state_send: &mut Vec<NetworkState>,
    ) -> Result<(), ManndError> {
        let mut should_refresh_ifaces = false;

        match action {
            WpaAction::GetInterfaces => {
                if let Some(WirelessAdapter::Wpa(wpa)) = &self.controller.wifi {
                    match wpa.get_interfaces().await {
                        Ok(interfaces) => {
                            state_send.push(NetworkState::SetWpaInterfaces(interfaces))
                        }
                        Err(e) => {
                            tracing::error!("[WPA]: Failed to get interfaces. {e:?}");
                            state_send.push(NetworkState::Failed(Failure::new(
                                Process::Generic,
                                e.to_string(),
                            )));
                        }
                    }
                }
            }
            WpaAction::CreateInterface(ifname) => {
                if let Some(WirelessAdapter::Wpa(wpa)) = self.controller.wifi.as_mut() {
                    match wpa.create_interface(ifname).await {
                        Ok(()) => should_refresh_ifaces = true,
                        Err(e) => {
                            tracing::error!("[WPA]: Failed to create interface {ifname}. {e:?}");
                            state_send.push(NetworkState::Failed(Failure::new(
                                Process::Generic,
                                e.to_string(),
                            )));
                        }
                    }
                }
            }
            WpaAction::RemoveInterface(ifname) => {
                if let Some(WirelessAdapter::Wpa(wpa)) = self.controller.wifi.as_mut() {
                    match wpa.remove_interface(ifname).await {
                        Ok(()) => should_refresh_ifaces = true,
                        Err(e) => {
                            tracing::error!("[WPA]: Failed to remove interface {ifname}. {e:?}");
                            state_send.push(NetworkState::Failed(Failure::new(
                                Process::Generic,
                                e.to_string(),
                            )));
                        }
                    }
                }
            }
            WpaAction::TogglePersist => {
                if let Some(WirelessAdapter::Wpa(wpa)) = &mut self.controller.wifi {
                    wpa.toggle_persist();
                }
            }
        }

        if let Some(WirelessAdapter::Wpa(wpa)) = &self.controller.wifi {
            match wpa.get_interfaces().await {
                Ok(interfaces) => state_send.push(NetworkState::SetWpaInterfaces(interfaces)),
                Err(e) => {
                    tracing::error!("[WPA]: Failed to refresh interfaces. {e:?}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::Generic,
                        e.to_string(),
                    )));
                }
            }
        }
        Ok(())
    }
}
