use crate::{
    error::{NdError, NeliError, ThreadSafeError},
    wireless::{iwd::Iwd, netlink::Netlink, wpa_supplicant::WpaSupplicant, WifiAdapter},
};
use neli::err::MsgError;
use std::sync::Arc;
use zbus::Connection;

enum Adapter {
    iwd(Iwd),
    wpa(WpaSupplicant),
}

struct Controller {
    // Wireless Daemons
    wifi: Option<Adapter>,
    /// Used for ethernet and wireless information iwd/wpa don't provide
    netlink: Netlink,
    connection: Arc<Connection>,
}

impl Controller {
    async fn new() -> Result<Self, NdError> {
        let conn = zbus::Connection::system().await?;
        let mut netlink = Netlink::wireless_connect().await?;
        if netlink.get_interfaces().await?.is_empty() {
            let mut netlink = Netlink::wired_connect().await?;
        }
        // Init wifi later
        Ok(Self {
            wifi: None,
            netlink: netlink,
            connection: Arc::new(conn),
        })
    }
}

#[tokio::test]
async fn new() -> Result<(), NdError> {
    let controller = Controller::new().await;
    match controller {
        Ok(val) => Ok(()),
        Err(e) => Err(NdError::OperationFailed("Test".to_string())),
    }
}
