use bitflags::bitflags;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use zbus::{Connection, zvariant::Value};

use crate::error::ManndError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Security {
    Open,
    Psk,
    Ieee8021x,
    Unknown,
}

impl std::fmt::Display for Security {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Psk => write!(f, "psk"),
            Self::Ieee8021x => write!(f, "8021x"),
            Self::Unknown => write!(f, ""),
        }
    }
}

impl From<&str> for Security {
    fn from(value: &str) -> Self {
        match value {
            "open" => Self::Open,
            "psk" => Self::Psk,
            "8021x" => Self::Ieee8021x,
            _ => Self::Unknown,
        }
    }
}

/// Returns the value of a property found under the `self.path` interfaces
/// Trait bounds follow from `zbus` downcast
#[instrument(err, skip(conn))]
pub async fn get_prop<'a, T>(
    conn: &Connection,
    service: String,
    path: String,
    subpath: &str,
    prop: &str,
) -> Result<T, ManndError>
where
    T: TryFrom<Value<'a>>,
    <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
{
    let interface_path = format!("{service}.{subpath}");
    let proxy = zbus::Proxy::new(conn, service, path, interface_path.clone()).await?;

    match proxy.get_property(prop).await? {
        Some(val) => Ok(<zbus::zvariant::Value<'_> as Clone>::clone(&val).downcast::<T>()?),
        None => Err(ManndError::PropertyNotFound(format!(
            "Could not find given property {prop} at {interface_path}"
        ))),
    }
}

/// Returns the value of a property found under the `self.path` interfaces
/// Trait bounds follow from `zbus` downcast
#[instrument(err, skip(proxy))]
pub async fn get_prop_from_proxy<'a, T>(
    proxy: &zbus::Proxy<'a>,
    prop: &str,
) -> Result<T, ManndError>
where
    T: TryFrom<Value<'a>>,
    <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
{
    match proxy.get_property(prop).await {
        Ok(val) => Ok(<zbus::zvariant::Value<'_> as Clone>::clone(&val).downcast::<T>()?),
        Err(_e) => Err(ManndError::PropertyNotFound(format!(
            "Could not find given property {} at {}",
            prop,
            proxy.path()
        ))),
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct NetworkFlags: u8 {
        const KNOWN = 0b0000_0001;
        const CONNECTED = 0b0000_0010;
        const NEARBY = 0b0000_0100;
        const HIDDEN = 0b0000_1000;
    }
}

#[derive(Builder, Debug, Clone, Serialize, Deserialize)]
#[builder(pattern = "owned")]
pub struct AccessPoint {
    pub ssid: String,
    pub security: Security,
    #[builder(default = NetworkFlags::empty())]
    pub flags: NetworkFlags,
}
