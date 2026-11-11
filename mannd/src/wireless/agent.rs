use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::info;
use zbus::{DBusError, OwnedObjectPath, interface};

#[derive(Debug)]
pub struct AgentState {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub struct IwdAgent {
    state: Arc<RwLock<AgentState>>,
}

#[derive(Debug, DBusError)]
#[zbus(prefix = "net.connman.iwd.Agent.Error")]
enum IwdAgentError {
    Canceled,
}

pub enum IwdAgentMsg {
    SetPassword(String),
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            username: None,
            password: None,
        }
    }
}

impl IwdAgent {
    pub const fn new(state: Arc<RwLock<AgentState>>) -> Self {
        Self { state }
    }
}

// [docs](https://kernel.googlesource.com/pub/scm/network/wireless/iwd/+/master/doc/agent-api.txt)
#[interface(name = "net.connman.iwd.Agent")]
// D-Bus methods need a receiver whether or not they use it
#[allow(non_snake_case, clippy::unused_self)]
impl IwdAgent {
    async fn release(&self) {
        info!("Relase");
        let mut writer = self.state.write().await;
        writer.username = None;
        writer.password = None;
    }

    fn requestPassphrase(&self, _network: OwnedObjectPath) -> String {
        info!("Passphrase has been requested");
        self.state.try_read().map_or_else(
            |_| {
                tracing::error!("Error obtaining reading lock.");
                String::new()
            },
            |reader| {
                reader.password.as_ref().map_or_else(
                    || {
                        info!("Sending empty string because password is not set.");
                        String::new()
                    },
                    Clone::clone,
                )
            },
        )
    }

    // EAP is not supported yet, so the requests below are turned down

    fn requestPrivateKeyPassphrase(
        &self,
        _network: OwnedObjectPath,
    ) -> Result<String, IwdAgentError> {
        info!("Private Key Passphrase");
        Err(IwdAgentError::Canceled)
    }

    fn requestUserNameAndPassword(
        &self,
        _network: OwnedObjectPath,
    ) -> Result<(String, String), IwdAgentError> {
        info!("Username and Password");
        Err(IwdAgentError::Canceled)
    }

    fn requestUserPassword(&self, _network: OwnedObjectPath) -> Result<String, IwdAgentError> {
        info!("User Password");
        Err(IwdAgentError::Canceled)
    }

    fn cancel(&self) {
        info!("Cancel");
    }
}
