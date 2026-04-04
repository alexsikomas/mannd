use std::{
    fs::{self, OpenOptions},
    io::ErrorKind,
    os::unix::fs::chown,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{context, error::ManndError};

const WPA_CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WpaConfig {
    #[serde(default)]
    pub ui: WpaUi,
    #[serde(default)]
    pub policy: WpaPolicy,
    #[serde(default)]
    pub interfaces: WpaIfaceConf,
}

impl Default for WpaConfig {
    fn default() -> Self {
        Self {
            ui: WpaUi::default(),
            policy: WpaPolicy::default(),
            interfaces: WpaIfaceConf::default(),
        }
    }
}

impl WpaConfig {
    pub fn load_or_default() -> Result<Self, ManndError> {
        let path = WpaConfig::path();
        let uid = context().uid;

        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // likely already initialised but doesn't hurt to add
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                    chown(parent, uid, None)?;
                }
                Self::write_default(&path)?;
                chown(&path, context().uid, None)?;
                return Ok(Self::default());
            }
            Err(e) => return Err(ManndError::IoError(e)),
        };

        if raw.trim().is_empty() {
            Self::write_default(&path)?;
            chown(&path, uid, None)?;
            return Ok(Self::default());
        }

        let config: Self =
            ron::from_str(&raw).map_err(|e| ManndError::InvalidPropertyFormat(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    fn write_default(path: &Path) -> Result<(), ManndError> {
        let default_conf = Self::default();
        let serial = ron::to_string(&default_conf)
            .map_err(|e| ManndError::InvalidPropertyFormat(e.to_string()))?;

        fs::write(path, serial)?;
        Ok(())
    }

    fn path() -> PathBuf {
        let mut p = PathBuf::from(&context().settings.storage.state);
        p.push("wpa.conf");
        p
    }

    fn validate(&self) -> Result<(), ManndError> {
        if let Some(country) = &self.policy.country {
            if country.len() != 2 || !country.is_ascii() {
                return Err(ManndError::InvalidPropertyFormat(format!(
                    "Country must be 2 ASCII characters, got {country}"
                )));
            }
        }

        // if let Some(default_iface) = &self.interfaces.default {
        //     if !self.interfaces.managed.is_empty()
        //         && !self.interfaces.managed.contains(default_iface)
        //     {
        //         // return Err;
        //     }
        // }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WpaUi {
    show_hidden_networks: bool,
    sort_networks_by: WpaUiSort,
}

impl Default for WpaUi {
    fn default() -> Self {
        Self {
            show_hidden_networks: false,
            sort_networks_by: WpaUiSort::SignalStrength,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WpaUiSort {
    SignalStrength,
    NameAsc,
    NameDesc,
}

// Name is more verbose to distinguish from [`WpaInterface`]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WpaIfaceConf {
    pub preferred_interface: Option<String>,
}

impl Default for WpaIfaceConf {
    fn default() -> Self {
        Self {
            preferred_interface: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WpaPolicy {
    pub apply_scope: ApplyScope,
    pub country: Option<String>,
    pub band_type: Option<BandType>,
    #[serde(default)]
    pub allow_freq_mhz: Vec<u32>,
    pub scan_interval_sec: Option<u32>,
    pub autoscan: WpaAutoscan,
    pub fast_reauth: bool,
    pub mac_randomization: MacRandomization,
}

impl Default for WpaPolicy {
    fn default() -> Self {
        Self {
            apply_scope: ApplyScope::AllInterfaces,
            country: None,
            band_type: None,
            allow_freq_mhz: Vec::new(),
            scan_interval_sec: Some(5),
            autoscan: WpaAutoscan::Disabled,
            fast_reauth: true,
            mac_randomization: MacRandomization::Always,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MacRandomization {
    Always,
    Never,
    PeerNetwork,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WpaAutoscan {
    Exponential { base: u32, limit: u32 },
    Periodic { interval: u32 },
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApplyScope {
    AllInterfaces,
    Interfaces(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BandType {
    Prefer5GHz,
    Restrict5GHz,
    Prefer2GHz,
    Restrict2GHz,
}
