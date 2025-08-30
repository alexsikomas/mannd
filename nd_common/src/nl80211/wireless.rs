use std::error::Error;

use crate::error::{NeliError, NetworkdLibError};
use neli::{
    consts::socket::NlFamily, err::SocketError, socket::asynchronous::NlSocketHandle, utils::Groups,
};
use neli_wifi::{AsyncSocket, Interface};

struct Wireless {
    socket: AsyncSocket,
}

impl Wireless {
    async fn new() -> Result<Self, NetworkdLibError> {
        match AsyncSocket::connect() {
            Ok(neli_handle) => Ok(Self {
                socket: neli_handle,
            }),
            Err(e) => Err(e.to_wifi_error("Could not initalise AsyncSocket")),
        }
    }
    /// Returns information about wireless hardware and its capabilities.
    async fn query_devices(&mut self) -> Option<Vec<Interface>> {
        match self.socket.get_interfaces_info().await {
            Ok(info) => Some(info),
            Err(e) => {
                eprintln!("Could not query devices via neli_wifi! {e}");
                None
            }
        }
    }

    /// Returns all the available wireless networks
    async fn get_networks() {
        todo!()
    }

    /// Connects to a wireless network
    async fn connect() {
        todo!()
    }

    /// Changes the power management mode
    async fn power_management() {
        todo!()
    }

    // Interface commands
    async fn get_interface() {}

    async fn set_interface() {}

    async fn new_interface() {}

    async fn del_interface() {}
}
