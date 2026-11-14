use std::process::Command;

use tracing::{info, instrument};
use zbus::Message;

use crate::{
    controller::{Controller, WifiDaemonType, WirelessAdapter},
    error::ManndError,
    state::{
        messages::{
            Capability, Failure, NetworkAction, NetworkState, Process, Started, Success,
            WifiAction, WireguardAction, WireguardCapability, WpaAction,
        },
        signals::SignalManager,
    },
    with_state,
};

/// Lives for as long as the daemon does, whichever client is connected
#[derive(Debug)]
pub struct NetworkActor {
    pub controller: Controller,
    pub signal_manager: SignalManager<'static>,
}

impl NetworkActor {
    #[instrument(err)]
    pub async fn new() -> Result<Self, ManndError> {
        let mut controller = Controller::new().await?;
        if with_state(|state| state.app.wg_running).unwrap_or(false) {
            let _ = controller.start_wireguard().await;
        }

        controller.connect_wifi_adapter().await;
        Ok(Self {
            controller,
            signal_manager: SignalManager::new(),
        })
    }

    /// Runs `action`, returning the states the client should be told about.
    ///
    /// Actions carry passphrases, so no argument may be recorded in the span.
    #[instrument(skip_all)]
    pub async fn handle_action(
        &mut self,
        action: NetworkAction,
    ) -> Result<Vec<NetworkState>, ManndError> {
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
                let caps = Capability {
                    wifi_daemon,
                    networkd_active,
                    wireguard: WireguardCapability { installed: wg_installed },
                };
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
                self.handle_wpa_action(&wpa_action, &mut state_send).await;
            }
        }

        Ok(state_send)
    }

    /// Turns a D-Bus signal into the action that should follow it, if any
    pub fn handle_signal(&mut self, msg: (usize, Message)) -> Option<NetworkAction> {
        match self.controller.get_wifi_daemon_type()? {
            WifiDaemonType::Iwd => self.signal_manager.process_iwd_msg(msg),
            WifiDaemonType::Wpa => self.signal_manager.process_wpa_msg(msg),
        }
    }

    #[instrument(skip_all)]
    async fn handle_wifi_action(
        &mut self,
        action: &WifiAction,
        state_send: &mut Vec<NetworkState>,
    ) -> Result<(), ManndError> {
        match action {
            WifiAction::Scan => {
                state_send.push(NetworkState::Start(Started(Process::WifiScan)));

                match self.controller.scan().await {
                    // the signal tells us when the results are ready
                    Ok(Some(scan_done)) => {
                        self.signal_manager.add(scan_done);
                    }
                    Ok(None) => info!("[Wi-Fi]: A scan is already running"),
                    Err(e) => {
                        tracing::error!("[Wi-Fi]: Scan failed. {e}");
                        state_send.push(NetworkState::Failed(Failure::new(
                            Process::WifiScan,
                            e.to_string(),
                        )));
                    }
                }
            }
            // get networks and send them to the TUI
            WifiAction::GetNetworks => match self.controller.get_networks().await {
                Ok(aps) => {
                    state_send.push(NetworkState::SetNetworks(aps));
                    state_send.push(NetworkState::Success(Success::Generic));
                }
                Err(e) => {
                    tracing::error!("[Wi-Fi]: Failed to get networks. {e}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::WifiScan,
                        e.to_string(),
                    )));
                }
            },
            WifiAction::Connect(network) => {
                state_send.push(NetworkState::Start(Started(Process::WifiConnect)));
                match self.controller.connect_network(network).await {
                    Ok(()) => {
                        info!("Connection to network was successful");
                        state_send.push(NetworkState::Success(Success::Connected(network.clone())));
                    }
                    Err(e) => {
                        tracing::error!(ssid = %network.ssid, "[Wi-Fi]: Connection to network was not successful. {e}");
                        state_send.push(NetworkState::Failed(Failure::new(
                            Process::WifiConnect,
                            e.to_string(),
                        )));
                    }
                }
            }
            WifiAction::ConnectKnown(network) => match self.controller.connect_network(network).await
            {
                Ok(()) => {
                    state_send.push(NetworkState::Success(Success::Connected(network.clone())));
                }
                Err(e) => {
                    tracing::warn!(ssid = %network.ssid, "[Wi-Fi]: Could not connect to a known network. Error: {e}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::WifiConnect,
                        e.to_string(),
                    )));
                }
            },
            WifiAction::Disconnect => match self.controller.disconnect_network().await {
                Ok(()) => {
                    info!("[Wi-Fi]: Disconnected from active network");
                    state_send.push(NetworkState::Success(Success::Disconnected));
                }
                Err(e) => {
                    tracing::error!("[Wi-Fi]: Disconnect failed. {e}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::Generic,
                        e.to_string(),
                    )));
                }
            },
            WifiAction::Forget(network) => match self.controller.remove_network(network).await {
                Ok(()) => {
                    info!("[Wi-Fi]: Network {} forgotten", network.ssid);
                    state_send.push(NetworkState::Success(Success::ForgotNetwork(
                        network.clone(),
                    )));
                }
                Err(e) => {
                    tracing::error!(ssid = %network.ssid, "[Wi-Fi]: Failed to forget network: {e}");
                    state_send.push(NetworkState::Failed(Failure::new(
                        Process::Generic,
                        e.to_string(),
                    )));
                }
            },
        };

        Ok(())
    }

    /// Gets wireguard info no matter the `WireguardAction` to simplify logic and remove
    /// repititions
    #[instrument(skip_all)]
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
                        tracing::error!(file = %file.display(), "[Wireguard]: Connection failed {e}");
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

    /// Runs the operation, then reports the interfaces as they now stand
    #[instrument(skip_all)]
    async fn handle_wpa_action(&mut self, action: &WpaAction, state_send: &mut Vec<NetworkState>) {
        let Some(WirelessAdapter::Wpa(wpa)) = self.controller.wifi.as_mut() else {
            return;
        };

        let res = match action {
            WpaAction::GetInterfaces => Ok(()),
            WpaAction::CreateInterface(ifname) => wpa.create_interface(ifname).await,
            WpaAction::RemoveInterface(ifname) => wpa.remove_interface(ifname).await,
        };
        if let Err(e) = res {
            tracing::error!(?action, "[WPA]: Action failed. {e}");
            state_send.push(NetworkState::Failed(Failure::new(
                Process::Generic,
                e.to_string(),
            )));
        }

        match wpa.get_interfaces().await {
            Ok(interfaces) => state_send.push(NetworkState::SetWpaInterfaces(interfaces)),
            Err(e) => {
                tracing::error!("[WPA]: Failed to get interfaces. {e}");
                state_send.push(NetworkState::Failed(Failure::new(
                    Process::Generic,
                    e.to_string(),
                )));
            }
        }
    }
}
