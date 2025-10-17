use zbus::{DBusError, interface};

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

#[interface(name = "org.mannd.IwdAgent")]
impl IwdAgent {
    async fn release(&self) {}

    async fn requestPassphrase(&self) -> String {
        todo!()
    }

    async fn requestPrivateKeyPassphrase(&self) -> String {
        todo!()
    }

    async fn requestUserNameAndPassword(&self) -> (String, String) {
        todo!()
    }

    async fn requestUserPassword(&self) -> Result<String, IwdAgentError> {
        todo!()
    }

    async fn cancel(&self) {}
}
