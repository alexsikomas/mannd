use zbus::{
    DBusError, interface,
    zvariant::{ObjectPath, OwnedObjectPath},
};

// Do compiler optimisations make it so that
// the password may be repeated in memory even if later
// cleared?
pub struct IwdAgent {
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug, DBusError)]
enum IwdAgentError {
    Canceled,
}

impl IwdAgent {
    pub fn new() -> Self {
        Self {
            username: None,
            password: None,
        }
    }
}

// docs: https://kernel.googlesource.com/pub/scm/network/wireless/iwd/+/master/doc/agent-api.txt
#[interface(name = "org.mannd.IwdAgent")]
impl IwdAgent {
    async fn release(&mut self) {
        self.username = None;
        self.password = None;
    }

    async fn requestPassphrase(&self, network: OwnedObjectPath) -> String {
        todo!()
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
