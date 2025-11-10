use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ComError {
    #[error("Netlink Message Error: {0}")]
    NeliMsg(#[from] neli::err::MsgError),
    #[error("Netlink Deserialisation Error: {0}")]
    NeliDe(#[from] neli::err::DeError),
    #[error("Netlink Serialisation Error: {0}")]
    NeliSer(#[from] neli::err::SerError),
    #[error("Netlink Builder Error: {0}")]
    NeliBuilder(#[from] neli::err::BuilderError),
    #[error("Netlink Router Error: {0}")]
    NeliRouter(Box<dyn ThreadSafeError>),
    #[error("Netlink Socket Error: {0}")]
    NeliSocket(#[from] neli::err::SocketError),
    #[error("Rt Builder Error: {0}")]
    RtBuilder(#[from] neli::rtnl::RtattrBuilderError),
    #[error("Ifinfomsg error: {0}")]
    Ifinfomsg(#[from] neli::rtnl::IfinfomsgBuilderError),
    #[error("Ifaddr error: {0}")]
    Ifaddrmsgbuilder(#[from] neli::rtnl::IfaddrmsgBuilderError),
    #[error("nlattrbuilder error: {0}")]
    Nlmsgbuilder(#[from] neli::genl::NlattrBuilderError),
    #[error("AttrType Builder Error: {0}")]
    AttrTypeBuilder(#[from] neli::genl::AttrTypeBuilderError),
    #[error("Rtm Builder Error: {0}")]
    RtmMsgBuilder(#[from] neli::rtnl::RtmsgBuilderError),
    #[error("Nlmsg Builder Error: {0}")]
    NlmsgBuilder(#[from] neli::nl::NlmsghdrBuilderError),
    #[error("Genlmsg Builder Error: {0}")]
    GenlmsgBuilder(#[from] neli::genl::GenlmsghdrBuilderError),

    #[error("Network could not be found!")]
    NetworkNotFound,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Property not found: {0}")]
    PropertyNotFound(String),
    #[error("Adapter not found: {0}")]
    AdapterNotFound(String),
    #[error("Security type is invalid!")]
    InvalidSecurityType,

    #[error("File not found: {0}")]
    FileNotFound(String),
    // io errors
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    // zbus errors
    #[error("Zbus Error: {0}")]
    Zbus(#[from] zbus::Error),
    #[error("Freedesktop Error from Zbus: {0}")]
    ZbusFreedesktop(#[from] zbus::fdo::Error),
    #[error("Zbus zvariant Error: {0}")]
    Zvariant(#[from] zbus::zvariant::Error),
}

impl<T, P> From<neli::err::RouterError<T, P>> for ComError
where
    T: Debug + Send + Sync + 'static,
    P: Debug + Send + Sync + 'static,
{
    fn from(err: neli::err::RouterError<T, P>) -> Self {
        ComError::NeliRouter(Box::new(err))
    }
}

pub trait NeliError {
    fn to_wifi_error(&self, msg: &str) -> ComError;
}

pub trait ThreadSafeError: std::error::Error + Send + Sync + 'static {}

impl<T> ThreadSafeError for T where T: std::error::Error + Send + Sync + 'static {}
