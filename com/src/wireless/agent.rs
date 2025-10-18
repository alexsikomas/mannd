use std::sync::{Arc, Mutex};

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
    state: Arc<Mutex<AgentState>>,
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
    pub fn new(state: Arc<Mutex<AgentState>>) -> Self {
        Self { state }
    }
}

// docs: https://kernel.googlesource.com/pub/scm/network/wireless/iwd/+/master/doc/agent-api.txt
#[interface(name = "org.mannd.IwdAgent")]
impl IwdAgent {
    async fn release(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.username = None;
        state.password = None;
    }

    async fn requestPassphrase(&self, network: OwnedObjectPath) -> String {
        let mut state = self.state.lock().unwrap();
        match &state.password {
            Some(pass) => pass.clone(),
            None => "".to_string(),
        }
    }

    async fn requestPrivateKeyPassphrase(&self, network: OwnedObjectPath) -> String {
        todo!()
    }

    async fn requestUserNameAndPassword(&self, network: OwnedObjectPath) -> (String, String) {
        todo!()
    }

    async fn requestUserPassword(&self, network: OwnedObjectPath) -> Result<String, IwdAgentError> {
        todo!()
    }

    async fn cancel(&self) {}
}
