use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::info;
use zbus::{DBusError, interface, zvariant::OwnedObjectPath};

#[derive(Debug)]
pub struct AgentState {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub struct IwdAgent {
    state: Arc<RwLock<AgentState>>,
}

#[allow(dead_code)]
#[derive(Debug, DBusError)]
enum IwdAgentError {
    Canceled,
}

pub enum IwdAgentMsg {
    SetPassword(String),
}

impl AgentState {
    pub fn new() -> Self {
        Self {
            username: None,
            password: None,
        }
    }
}

impl IwdAgent {
    pub fn new(state: Arc<RwLock<AgentState>>) -> Self {
        Self { state }
    }
}

// [docs](https://kernel.googlesource.com/pub/scm/network/wireless/iwd/+/master/doc/agent-api.txt)
#[interface(name = "net.connman.iwd.Agent")]
#[allow(non_snake_case)]
impl IwdAgent {
    async fn release(&mut self) {
        info!("Relase");
        let mut writer = self.state.write().await;
        writer.username = None;
        writer.password = None;
    }

    async fn requestPassphrase(&self, _network: OwnedObjectPath) -> String {
        info!("Passphrase has been requested");
        match self.state.try_read() {
            Ok(reader) => match &reader.password {
                Some(pass) => pass.clone(),
                None => {
                    info!("Sending empty string because password is not set.");
                    "".to_string()
                }
            },
            Err(e) => {
                tracing::error!("Error obtaining reading lock.");
                "".to_string()
            }
        }
    }

    async fn requestPrivateKeyPassphrase(&self, _network: OwnedObjectPath) -> String {
        info!("Private Key Passphrase");
        todo!()
    }

    async fn requestUserNameAndPassword(&self, _network: OwnedObjectPath) -> (String, String) {
        info!("Username and Password");
        todo!()
    }

    async fn requestUserPassword(
        &self,
        _network: OwnedObjectPath,
    ) -> Result<String, IwdAgentError> {
        info!("User Password");
        todo!()
    }

    async fn cancel(&self) {
        info!("Cancel");
    }
}
