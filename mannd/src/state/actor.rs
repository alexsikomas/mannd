use std::process::Command;

use tokio::sync::mpsc::Sender;
use tracing::{info, instrument};

use crate::{
    controller::{Controller, WirelessAdapter},
    error::ManndError,
    state::{
        messages::{
            Capability, Failure, NetworkAction, NetworkState, Process, Started, Success,
            WifiAction, WireguardAction, WpaAction,
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
        // check if wifi then allow wifi requests
        let mut state_send: Vec<NetworkState> = vec![];

        // Wi-Fi not needed
        match action {
            NetworkAction::GetCapabilities => {
                let wifi_daemon = self.controller.get_wifi_daemon_type();
                let networkd_active = self.controller.networkd_status().await;
                let wg_installed = Command::new("wg")
                    .arg("--version")
                    .output()
                    .map_or(false, |_| true);

                // TODO: check state
                let caps = Capability::new(wifi_daemon, networkd_active, (wg_installed, false));
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
                if let Ok(()) = self.controller.disconenct_network().await {
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

    async fn handle_wireguard_action(
        &mut self,
        action: &WireguardAction,
        state_send: &mut Vec<NetworkState>,
    ) -> Result<(), ManndError> {
        match action {
            WireguardAction::ConnectWireguard(file) => {
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
            WireguardAction::ToggleWireguard => {
                if !self.controller.is_wireguard_connected() {
                    if let Ok(()) = self.controller.start_wireguard().await {
                        state_send.push(NetworkState::Success(Success::EnableWireguard));
                        // self.handle_net_ctx(NetCtxFlags::Wireguard, &mut state_send)
                        //     .await?;
                    }
                } else {
                    if let Ok(()) = self.controller.remove_wireguard_iface().await {
                        state_send.push(NetworkState::Success(Success::DisableWireguard));
                    }
                }
            }
            WireguardAction::GetWireguardInfo => {}
        }

        Ok(())
    }

    async fn handle_wpa_action(&mut self, action: &WpaAction, state_send: &mut Vec<NetworkState>) {
        match action {
            WpaAction::CreateInterface(ifname) => {
                if let Some(WirelessAdapter::Wpa(wpa)) = self.controller.wifi.as_mut() {
                    let _ = wpa.create_interface(ifname).await;
                }
            }
            WpaAction::TogglePersist => {
                if let Some(WirelessAdapter::Wpa(wpa)) = &mut self.controller.wifi {
                    wpa.toggle_persist();
                    state_send.push(NetworkState::ToggleWpaPesist);
                }
            }
            WpaAction::GetInterfaces => todo!(),
        }
    }
}
