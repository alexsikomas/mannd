use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ComError {
    #[error("Netlink Message Error: {0}")]
    NeliMsg(neli::err::MsgError),
    #[error("Netlink Deserialisation Error: {0}")]
    NeliDe(neli::err::DeError),
    #[error("Netlink Serialisation Error: {0}")]
    NeliSer(neli::err::SerError),
    #[error("Netlink Builder Error: {0}")]
    NeliBuilder(neli::err::BuilderError),
    #[error("Netlink Router Error: {0}")]
    NeliRouter(Box<dyn ThreadSafeError>),
    #[error("Netlink Socket Error: {0}")]
    NeliSocket(neli::err::SocketError),
    #[error("Rt Builder Error: {0}")]
    RtBuilder(neli::rtnl::RtattrBuilderError),
    #[error("Ifinfomsg error: {0}")]
    Ifinfomsg(neli::rtnl::IfinfomsgBuilderError),
    #[error("Ifaddr error: {0}")]
    Ifaddrmsgbuilder(neli::rtnl::IfaddrmsgBuilderError),
    #[error("nlattrbuilder error: {0}")]
    Nlmsgbuilder(neli::genl::NlattrBuilderError),
    #[error("AttrType Builder Error: {0}")]
    AttrTypeBuilder(neli::genl::AttrTypeBuilderError),
    #[error("Rtm Builder Error: {0}")]
    RtmMsgBuilder(neli::rtnl::RtmsgBuilderError),
    #[error("Nlmsg Builder Error: {0}")]
    NlmsgBuilder(neli::nl::NlmsghdrBuilderError),
    #[error("Genlmsg Builder Error: {0}")]
    GenlmsgBuilder(neli::genl::GenlmsghdrBuilderError),
    #[error("Signal failed to send over channel: {0}")]
    SignalSend(String),

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
    IoError(std::io::Error),

    // zbus errors
    #[error("Zbus Error: {0}")]
    Zbus(zbus::Error),
    #[error("Freedesktop Error from Zbus: {0}")]
    ZbusFreedesktop(zbus::fdo::Error),
    #[error("Zbus zvariant Error: {0}")]
    Zvariant(zbus::zvariant::Error),

    #[error("Error while builder access point struct: {0}")]
    AccessPointBuilder(crate::wireless::common::AccessPointBuilderError),
    #[error("Invalid password length")]
    PasswordLength,
    #[error("Connection timeout")]
    Timeout,

    #[error("Database error occured")]
    RedbDatabase(redb::DatabaseError),
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

macro_rules! error_with_tracing {
    ($from_type:ty, $enum_variant:ident, $log_message:literal) => {
        impl From<$from_type> for ComError {
            fn from(err: $from_type) -> Self {
                tracing::error!($log_message, err);
                ComError::$enum_variant(err)
            }
        }
    };
}

error_with_tracing!(neli::err::MsgError, NeliMsg, "Netlink Message Error: {}");
error_with_tracing!(
    neli::err::DeError,
    NeliDe,
    "Netlink Deserialisation Error: {}"
);
error_with_tracing!(
    neli::err::SerError,
    NeliSer,
    "Netlink Serialisation Error: {}"
);
error_with_tracing!(
    neli::err::BuilderError,
    NeliBuilder,
    "Netlink Builder Error: {}"
);
error_with_tracing!(
    neli::err::SocketError,
    NeliSocket,
    "Netlink Socket Error: {}"
);
error_with_tracing!(
    neli::rtnl::RtattrBuilderError,
    RtBuilder,
    "Rt Builder Error: {}"
);
error_with_tracing!(
    neli::rtnl::IfinfomsgBuilderError,
    Ifinfomsg,
    "Ifinfomsg error: {}"
);
error_with_tracing!(
    neli::rtnl::IfaddrmsgBuilderError,
    Ifaddrmsgbuilder,
    "Ifaddr error: {}"
);
error_with_tracing!(
    neli::genl::NlattrBuilderError,
    Nlmsgbuilder,
    "nlattrbuilder error: {}"
);
error_with_tracing!(
    neli::genl::AttrTypeBuilderError,
    AttrTypeBuilder,
    "AttrType Builder Error: {}"
);
error_with_tracing!(
    neli::rtnl::RtmsgBuilderError,
    RtmMsgBuilder,
    "Rtm Builder Error: {}"
);
error_with_tracing!(
    neli::nl::NlmsghdrBuilderError,
    NlmsgBuilder,
    "Nlmsg Builder Error: {}"
);
error_with_tracing!(
    neli::genl::GenlmsghdrBuilderError,
    GenlmsgBuilder,
    "Genlmsg Builder Error: {}"
);
error_with_tracing!(std::io::Error, IoError, "IO Error: {}");
error_with_tracing!(zbus::Error, Zbus, "Zbus Error: {}");
error_with_tracing!(
    zbus::fdo::Error,
    ZbusFreedesktop,
    "Freedesktop Error from Zbus: {}"
);

error_with_tracing!(zbus::zvariant::Error, Zvariant, "Zbus zvariant Error: {}");

error_with_tracing!(
    crate::wireless::common::AccessPointBuilderError,
    AccessPointBuilder,
    "Error building ap: {}"
);

error_with_tracing!(redb::DatabaseError, RedbDatabase, "Redb database error: {}");
