use core::error;
use std::{
    fmt::{format, Debug},
    io::Result,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkdLibError {
    // neli errors
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
    // #[error("Netlink Header Error: {0}")]
    // NeliHeaderError(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Netlink Socket Error: {0}")]
    NeliSocketError(#[from] neli::err::SocketError),
    #[error("{0}")]
    NeliScanError(#[from] neli::err::RouterError)

    #[error("Error Resolving Value: {0}")]
    ResolveError(String),

    // This is a generic error was used before for neli-wifi errors
    #[error("Wifi Error from neli-wifi: {0}")]
    WifiError(String),

    // zbus errors
    #[error("Zbus Error: {0}")]
    ZbusError(#[from] zbus::Error),
    #[error("Freedesktop Error from Zbus: {0}")]
    ZbusFDOError(#[from] zbus::fdo::Error),
}

impl<T, P> From<neli::err::RouterError<T, P>> for NetworkdLibError
where
    T: Debug + Send + Sync + 'static,
    P: Debug + Send + Sync + 'static,
{
    fn from(err: neli::err::RouterError<T, P>) -> Self {
        NetworkdLibError::NeliRouterError(Box::new(err))
    }
}

impl<M> From<neli::err::Nlmsgerr<M>> for NetworkdLibError
where
    M: Debug + Send + Sync + 'static,
{
    fn from(err: neli::err::Nlmsgerr<M>) -> Self {
        NetworkdLibError::NeliPacketError(Box::new(err))
    }
}

// FIX: Nlmsghdr does not implement std::errror::Error
//
// impl<T, P> From<neli::err::NlmsghdrErr<T, P>> for NetworkdLibError
// where
//     T: Debug + Send + Sync + 'static,
//     P: Debug + Send + Sync + 'static,
// {
//     fn from(err: neli::err::NlmsghdrErr<T, P>) -> Self {
//         NetworkdLibError::NeliHeaderError(Box::new(err))
//     }
// }

/// To circumvent duplication of match statements from neli-wifi which uses the depracted NlError
pub trait NeliError {
    fn to_wifi_error(&self, msg: &str) -> NetworkdLibError;
}

impl<T> NeliError for T
where
    T: ThreadSafeError,
{
    fn to_wifi_error(&self, msg: &str) -> NetworkdLibError {
        match self.source() {
            Some(err) => NetworkdLibError::WifiError(format!("{msg}: {self}")),
            _ => NetworkdLibError::WifiError(format!("{msg}: Error source could not be found!")),
        }
    }
}

pub trait ThreadSafeError: std::error::Error + Send + Sync + 'static {}

impl<T> ThreadSafeError for T where T: std::error::Error + Send + Sync + 'static {}
