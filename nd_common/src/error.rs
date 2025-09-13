use std::{
    fmt::{format, Debug},
    io::Result,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NdError {
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

    // zbus errors
    #[error("Zbus Error: {0}")]
    Zbus(#[from] zbus::Error),
    #[error("Freedesktop Error from Zbus: {0}")]
    ZbusFreedesktop(#[from] zbus::fdo::Error),
}

impl<T, P> From<neli::err::RouterError<T, P>> for NdError
where
    T: Debug + Send + Sync + 'static,
    P: Debug + Send + Sync + 'static,
{
    fn from(err: neli::err::RouterError<T, P>) -> Self {
        NdError::NeliRouterError(Box::new(err))
    }
}

impl<M> From<neli::err::Nlmsgerr<M>> for NdError
where
    M: Debug + Send + Sync + 'static,
{
    fn from(err: neli::err::Nlmsgerr<M>) -> Self {
        NdError::NeliPacketError(Box::new(err))
    }
}

pub trait NeliError {
    fn to_wifi_error(&self, msg: &str) -> NdError;
}

pub trait ThreadSafeError: std::error::Error + Send + Sync + 'static {}

impl<T> ThreadSafeError for T where T: std::error::Error + Send + Sync + 'static {}
