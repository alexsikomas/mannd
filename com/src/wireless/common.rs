use bitflags::bitflags;
use derive_builder::Builder;
use zbus::{Connection, zvariant::Value};

use crate::error::ComError;

#[derive(Debug, Clone)]
pub enum Security {
    Open,
    Psk,
    Ieee8021x,
    Unknown,
}

impl std::fmt::Display for Security {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Security::Open => write!(f, "open"),
            Security::Psk => write!(f, "psk"),
            Security::Ieee8021x => write!(f, "8021x"),
            Security::Unknown => write!(f, ""),
        }
    }
}

impl Security {
    pub fn from_str(str: &str) -> Self {
        match str {
            "open" => Security::Open,
            "psk" => Security::Psk,
            "8021x" => Security::Ieee8021x,
            _ => Security::Unknown,
        }
    }
}

/// Returns the value of a property found under the `self.path` interfaces
/// Trait bounds follow from `zbus` downcast
pub async fn get_prop<'a, T>(
    conn: &Connection,
    service: String,
    path: String,
    subpath: &str,
    prop: &str,
) -> Result<T, ComError>
where
    T: TryFrom<Value<'a>>,
    <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
{
    let interface_path = format!("{}.{}", service, subpath);
    let proxy = zbus::Proxy::new(conn, service, path, interface_path.clone()).await?;

    match proxy.get_property(prop).await? {
        Some(val) => Ok(<zbus::zvariant::Value<'_> as Clone>::clone(&val).downcast::<T>()?),
        None => Err(ComError::PropertyNotFound(format!(
            "Could not find given property {} at {}",
            prop, interface_path
        ))),
    }
}

/// Returns the value of a property found under the `self.path` interfaces
/// Proxy must be passed in, use this to reduce overhead
/// Trait bounds follow from `zbus` downcast
pub async fn get_prop_from_proxy<'a, T>(proxy: &zbus::Proxy<'a>, prop: &str) -> Result<T, ComError>
where
    T: TryFrom<Value<'a>>,
    <T as TryFrom<Value<'a>>>::Error: Into<zbus::zvariant::Error>,
{
    match proxy.get_property(prop).await {
        Ok(val) => Ok(<zbus::zvariant::Value<'_> as Clone>::clone(&val).downcast::<T>()?),
        Err(_e) => Err(ComError::PropertyNotFound(format!(
            "Could not find given property {} at {}",
            prop,
            proxy.path()
        ))),
    }
}

pub fn ssid_to_hex(ssid: String) -> String {
    let bytes = ssid.as_bytes();
    bytes.into_iter().map(|b| format!("{:02x}", b)).collect()
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct NetworkFlags: u8 {
        const KNOWN = 0b00000001;
        const CONNECTED = 0b00000010;
        const NEARBY = 0b00000100;
        const HIDDEN = 0b00001000;
    }
}

#[derive(Builder, Debug, Clone)]
#[builder(pattern = "owned")]
pub struct AccessPoint {
    pub ssid: String,
    pub security: Security,
    #[builder(default = NetworkFlags::empty())]
    pub flags: NetworkFlags,
}
