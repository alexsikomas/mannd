use bitflags::bitflags;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zbus::Type;

use crate::error::ManndError;

/// Returns the value of a property found under a proxy's interface.
/// Trait bounds follow from `zbus` downcast.
pub async fn get_prop_from_proxy<'a, T>(
    proxy: &zbus::Proxy<'a>,
    prop: &str,
) -> Result<T, ManndError>
where
    T: DeserializeOwned + Type,
{
    proxy
        .get_property::<T>(prop)
        .await
        .map_err(|e| ManndError::OperationFailed(format!(
            "Could not read property '{prop}' at {}: {e}",
            proxy.path()
        )))
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
