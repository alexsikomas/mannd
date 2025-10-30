use neli::err::RouterError;
use std::fmt::{Debug, Display};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ComError {
    #[error("Netlink Message Error: {0}")]
    NeliMsgError(#[from] neli::err::MsgError),
    #[error("Netlink Deserialisation Error: {0}")]
    NeliDeError(#[from] neli::err::DeError),
    #[error("Netlink Serialisation Error: {0}")]
    NeliSerError(#[from] neli::err::SerError),
    #[error("Netlink Builder Error: {0}")]
    NeliBuilderError(#[from] neli::err::BuilderError),
    #[error("Netlink Router Error: {0}")]
    NeliRouterError(Box<dyn ThreadSafeError>),
    #[error("Netlink Packet Error: {0}")]
    NeliPacketError(Box<dyn ThreadSafeError>),
    #[error("Netlink Socket Error: {0}")]
    NeliSocketError(#[from] neli::err::SocketError),
    #[error("Rt Builder Error: {0}")]
    RtBuilder(#[from] neli::rtnl::RtattrBuilderError),
    #[error("Ifinfomsg error: {0}")]
    Ifinfomsg(#[from] neli::rtnl::IfinfomsgBuilderError),
    #[error("Ifaddr error: {0}")]
    Ifaddrmsgbuilder(#[from] neli::rtnl::IfaddrmsgBuilderError),

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
        ComError::NeliRouterError(Box::new(err))
    }
}

impl<M> From<neli::err::Nlmsgerr<M>> for ComError
where
    M: Debug + Send + Sync + 'static,
{
    fn from(err: neli::err::Nlmsgerr<M>) -> Self {
        ComError::NeliPacketError(Box::new(err))
    }
}

pub trait NeliError {
    fn to_wifi_error(&self, msg: &str) -> ComError;
}

pub trait ThreadSafeError: std::error::Error + Send + Sync + 'static {}

impl<T> ThreadSafeError for T where T: std::error::Error + Send + Sync + 'static {}
