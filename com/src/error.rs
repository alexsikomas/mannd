use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManndError {
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
    #[error("Signal failed to send over channel: {0}")]
    SignalSend(String),

    #[error("Network could not be found!")]
    NetworkNotFound,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Following text is in an invalid format {0}")]
    InvalidPropertyFormat(String),
    #[error("Adapter not found: {0}")]
    AdapterNotFound(String),
    #[error("Security type is invalid!")]
    InvalidSecurityType,

    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Section: {0} not found in configuration file!")]
    SectionNotFound(String),
    #[error("Property {0} not found in configuration file!")]
    PropertyNotFound(String),

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

    #[error("Error while builder access point struct: {0}")]
    AccessPointBuilder(#[from] crate::wireless::common::AccessPointBuilderError),
    #[error("Invalid password length")]
    PasswordLength,
    #[error("Connection timeout")]
    Timeout,

    #[error("Database error occured")]
    RedbDatabase(#[from] redb::DatabaseError),
    #[error("Database transaction error occured")]
    RedbTransaction(#[from] redb::TransactionError),
    #[error("Database commit error occured")]
    RedbCommit(#[from] redb::CommitError),
    #[error("Database table error occured")]
    RedbTable(#[from] redb::TableError),
    #[error("Database storage error occured")]
    RedbStorage(#[from] redb::StorageError),

    #[error("Wireguard accessed while not initialised")]
    WgAccess,
    #[error("No ips found in wireguard file")]
    WgIps,
    #[error("Ip address found is not valid")]
    StrToIp,

    #[error("Postcard error occured")]
    Postcard(#[from] postcard::Error),
    #[error("Not running as root!")]
    NotRoot,
    #[error("Cannot write to socket")]
    SocketWrite,

    #[error("Cannot parse to int")]
    ParseInt(#[from] std::num::ParseIntError),
    #[error("Serde issue deserialising: {0}")]
    SerdeDe(#[from] serde::de::value::Error),

    #[error("wpa_supplicant has no interfaces")]
    WpaNoInterfaces,
}

impl<T, P> From<neli::err::RouterError<T, P>> for ManndError
where
    T: Debug + Send + Sync + 'static,
    P: Debug + Send + Sync + 'static,
{
    fn from(err: neli::err::RouterError<T, P>) -> Self {
        ManndError::NeliRouter(Box::new(err))
    }
}

pub trait NeliError {
    fn to_wifi_error(&self, msg: &str) -> ManndError;
}

pub trait ThreadSafeError: std::error::Error + Send + Sync + 'static {}

impl<T> ThreadSafeError for T where T: std::error::Error + Send + Sync + 'static {}
//
// macro_rules! error_with_tracing {
//     ($from_type:ty, $enum_variant:ident, $log_message:literal) => {
//         impl From<$from_type> for ManndError {
//             fn from(err: $from_type) -> Self {
//                 tracing::error!($log_message, err);
//                 ManndError::$enum_variant(err)
//             }
//         }
//     };
// }
//
// error_with_tracing!(neli::err::MsgError, NeliMsg, "Netlink Message Error: {}");
// error_with_tracing!(
//     neli::err::DeError,
//     NeliDe,
//     "Netlink Deserialisation Error: {}"
// );
// error_with_tracing!(
//     neli::err::SerError,
//     NeliSer,
//     "Netlink Serialisation Error: {}"
// );
// error_with_tracing!(
//     neli::err::BuilderError,
//     NeliBuilder,
//     "Netlink Builder Error: {}"
// );
// error_with_tracing!(
//     neli::err::SocketError,
//     NeliSocket,
//     "Netlink Socket Error: {}"
// );
// error_with_tracing!(
//     neli::rtnl::RtattrBuilderError,
//     RtBuilder,
//     "Rt Builder Error: {}"
// );
// error_with_tracing!(
//     neli::rtnl::IfinfomsgBuilderError,
//     Ifinfomsg,
//     "Ifinfomsg error: {}"
// );
// error_with_tracing!(
//     neli::rtnl::IfaddrmsgBuilderError,
//     Ifaddrmsgbuilder,
//     "Ifaddr error: {}"
// );
// error_with_tracing!(
//     neli::genl::NlattrBuilderError,
//     Nlmsgbuilder,
//     "nlattrbuilder error: {}"
// );
// error_with_tracing!(
//     neli::genl::AttrTypeBuilderError,
//     AttrTypeBuilder,
//     "AttrType Builder Error: {}"
// );
// error_with_tracing!(
//     neli::rtnl::RtmsgBuilderError,
//     RtmMsgBuilder,
//     "Rtm Builder Error: {}"
// );
// error_with_tracing!(
//     neli::nl::NlmsghdrBuilderError,
//     NlmsgBuilder,
//     "Nlmsg Builder Error: {}"
// );
// error_with_tracing!(
//     neli::genl::GenlmsghdrBuilderError,
//     GenlmsgBuilder,
//     "Genlmsg Builder Error: {}"
// );
// error_with_tracing!(std::io::Error, IoError, "IO Error: {}");
// error_with_tracing!(zbus::Error, Zbus, "Zbus Error: {}");
// error_with_tracing!(
//     zbus::fdo::Error,
//     ZbusFreedesktop,
//     "Freedesktop Error from Zbus: {}"
// );
//
// error_with_tracing!(zbus::zvariant::Error, Zvariant, "Zbus zvariant Error: {}");
//
// error_with_tracing!(
//     crate::wireless::common::AccessPointBuilderError,
//     AccessPointBuilder,
//     "Error building ap: {}"
// );
//
// error_with_tracing!(redb::DatabaseError, RedbDatabase, "Redb database error: {}");
// error_with_tracing!(
//     redb::TransactionError,
//     RedbTransaction,
//     "Redb transaction error: {}"
// );
//
// error_with_tracing!(redb::CommitError, RedbCommit, "Redb commit error: {}");
// error_with_tracing!(redb::TableError, RedbTable, "Redb table error: {}");
// error_with_tracing!(redb::StorageError, RedbStorage, "Redb storage error: {}");
// error_with_tracing!(postcard::Error, Postcard, "Postcard error occured: {}");
// error_with_tracing!(
//     serde::de::value::Error,
//     SerdeDe,
//     "Serde Deserialisation error: {}"
// );
