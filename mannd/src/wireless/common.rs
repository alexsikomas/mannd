use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use zbus::zvariant::Value;

use crate::error::ManndError;

/// Returns the value of a property found under a proxy's interface.
/// Trait bounds follow from `zbus` downcast.
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
        Err(e) => Err(ManndError::OperationFailed(format!(
            "Could not read property '{prop}' at {}: {e}",
            proxy.path()
        ))),
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct NetworkFlags: u8 {
        const KNOWN     = 0b0000_0001;
        const CONNECTED = 0b0000_0010;
        const NEARBY    = 0b0000_0100;
        const HIDDEN    = 0b0000_1000;
    }
}
