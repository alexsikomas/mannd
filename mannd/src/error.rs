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

    #[error("Error while building network info struct: {0}")]
    NetworkInfoBuilder(#[from] crate::store::NetworkInfoBuilderError),
    #[error("Error while building wpa policy override struct: {0}")]
    WpaNetworkPolicyOverrideBuilder(#[from] crate::store::WpaNetworkPolicyOverrideBuilderError),
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
    #[error("Issue with int conversion")]
    TryFromIntErr(#[from] std::num::TryFromIntError),
    #[error("Time error")]
    SystemTime(#[from] std::time::SystemTimeError),

    #[error("wpa_supplicant has no interfaces")]
    WpaNoInterfaces,
    #[error("Could not remove wpa interface as there are no interfaces.")]
    WpaRemoveEmpty,
    #[error("Could not remove wpa interface as it could not be found.")]
    WpaRemoveNotFound,
    #[error("Hole between interface entries.")]
    WpaInterfaceHole,

    #[error("Malformed keybind")]
    InputKey,
    #[error("Home has already been initialised")]
    HomeInitialised,
    #[error("UID without a home")]
    UidHome,
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
