use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::info;
use zbus::{
    Connection, DBusError, interface,
    zvariant::{ObjectPath, OwnedObjectPath},
};

// Do compiler optimisations make it so that
// the password may be repeated in memory even if later
// cleared?
#[derive(Debug)]
pub struct AgentState {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub struct IwdAgent {
    state: Arc<RwLock<AgentState>>,
}

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

// docs: https://kernel.googlesource.com/pub/scm/network/wireless/iwd/+/master/doc/agent-api.txt
#[interface(name = "net.connman.iwd.Agent")]
impl IwdAgent {
    async fn release(&mut self) {
        info!("Relase");
        let mut writer = self.state.write().await;
        writer.username = None;
        writer.password = None;
    }

    async fn requestPassphrase(&self, network: OwnedObjectPath) -> String {
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

    async fn requestPrivateKeyPassphrase(&self, network: OwnedObjectPath) -> String {
        info!("Private Key Passphrase");
        todo!()
    }

    async fn requestUserNameAndPassword(&self, network: OwnedObjectPath) -> (String, String) {
        info!("Username and Password");
        todo!()
    }

    async fn requestUserPassword(&self, network: OwnedObjectPath) -> Result<String, IwdAgentError> {
        info!("User Password");
        todo!()
    }

    async fn cancel(&self) {
        info!("Cancel");
    }
}
