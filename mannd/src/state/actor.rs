use std::process::Command;

use tokio::sync::mpsc::Sender;
use tracing::{info, instrument};

use crate::{
    controller::{Controller, WirelessAdapter},
    error::ManndError,
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
        let app_state = controller.load_app_state()?.unwrap_or_default();
        if app_state.wg_running {
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
                    WireguardCapability::new(
                        wg_installed,
                        self.controller.is_wireguard_connected(),
                    ),
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
        // a lot of times we want to update network list
        // after an action
        let mut should_refresh = false;

        match action {
            WifiAction::Scan => {
                state_send.push(NetworkState::Start(Started(Process::WifiScan)));
                let _ = self.controller.scan(self.signal_tx.clone()).await;
            }
            WifiAction::GetNetworks => {
                if let Ok(aps) = self.controller.get_all_networks().await {
                    state_send.push(NetworkState::SetNetworks(aps));
                    state_send.push(NetworkState::Success(Success::Generic));
                }
            }
            WifiAction::Connect(info) => {
                state_send.push(NetworkState::Start(Started(Process::WifiConnect)));

                match self.controller.network_connect(info).await {
                    Ok(()) => {
                        info!("Connection to network was successful");
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
            WifiAction::ConnectKnown(ssid, security) => {
                match self.controller.connect_known(ssid, security).await {
                    Ok(()) => {
                        // state_send.push(NetworkState::CallAction(
                        //     NetworkAction::GetNetworkContext(NetCtxFlags::Network),
                        // ));
                        should_refresh = true;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[Wi-Fi]: Could not connect to a known network. Error: {e:?}"
                        );
                        state_send.push(NetworkState::Failed(Failure::new(
                            Process::WifiConnect,
                            e.to_string(),
                        )));
                    }
                }
            }

            WifiAction::Disconnect => {
                if let Ok(()) = self.controller.disconnect_network().await {
                    info!("[Wi-Fi]: Disconnected from active network");
                    should_refresh = true;
                }
            }
            WifiAction::Forget(ssid, sec) => {
                if let Ok(()) = self.controller.remove_network(ssid, sec).await {
                    info!("[Wi-Fi]: Network {ssid} forgotten");
                    should_refresh = true
                }
            }
            _ => {}
        };

        if should_refresh {
            if let Ok(networks) = self.controller.get_all_networks().await {
                state_send.push(NetworkState::SetNetworks(networks));
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
        match action {
            WpaAction::GetInterfaces => {
                if let Some(WirelessAdapter::Wpa(wpa)) = &self.controller.wifi {
                    let interfaces = wpa.get_interfaces().await?;
                    state_send.push(NetworkState::SetWpaInterfaces(interfaces));
                }
            }
            WpaAction::CreateInterface(ifname) => {
                if let Some(WirelessAdapter::Wpa(wpa)) = self.controller.wifi.as_mut() {
                    wpa.create_interface(ifname).await?;
                }
            }
            WpaAction::TogglePersist => {
                if let Some(WirelessAdapter::Wpa(wpa)) = &mut self.controller.wifi {
                    wpa.toggle_persist();
                }
            }
        }
        Ok(())
    }
}
