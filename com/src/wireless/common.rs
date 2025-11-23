use derive_builder::Builder;
use zbus::{zvariant::Value, Connection};

use crate::error::ComError;

#[derive(Debug, Clone)]
pub enum Security {
    Open,
    Psk,
    Ieee8021x,
}

impl std::fmt::Display for Security {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Security::Open => write!(f, "open"),
            Security::Psk => write!(f, "psk"),
            Security::Ieee8021x => write!(f, "8021x"),
        }
    }
}

impl Security {
    pub fn from_str(str: &str) -> Option<Self> {
        match str {
            "open" => Some(Security::Open),
            "psk" => Some(Security::Psk),
            "8021x" => Some(Security::Ieee8021x),
            _ => None,
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
    match proxy.get_property(prop).await? {
        Some(val) => Ok(<zbus::zvariant::Value<'_> as Clone>::clone(&val).downcast::<T>()?),
        None => Err(ComError::PropertyNotFound(format!(
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

#[derive(Builder, Debug, Clone)]
#[builder(pattern = "owned")]
pub struct AccessPoint {
    pub ssid: String,
    // OPTIONAL ARGUMENTS:
    #[builder(setter(into, strip_option), default)]
    pub security: Option<Security>,
    #[builder(setter(into, strip_option), default)]
    pub known: Option<bool>,
    #[builder(setter(into, strip_option), default)]
    pub connected: Option<bool>,
    #[builder(setter(into, strip_option), default)]
    pub nearby: Option<bool>,
    /// In some cases an empty string does
    /// not imply a hidden network depending
    /// on the type of request i.e beacon
    /// frame vs probe
    #[builder(setter(into, strip_option), default)]
    pub hidden: Option<bool>,
}
