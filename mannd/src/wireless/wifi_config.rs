use crate::{
    STORAGE_PATH, context, error::ManndError, store::NetworkInfo, wireless::common::NetworkFlags,
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    fs::{self},
    io::ErrorKind,
    os::unix::fs::chown,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WifiConfig {
    #[serde(default)]
    pub ui: WifiUi,
    #[serde(default)]
    pub general: WifiGeneral,
    #[serde(default)]
    pub wpa: WpaPolicy,
    #[serde(default)]
    pub iwd: IwdPolicy,
}

impl WifiConfig {
    pub fn load_or_default() -> Result<Self, ManndError> {
        let path = Self::path();
        let uid = context().uid;

        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // likely already created but doesn't hurt to add
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
        let storage = STORAGE_PATH.get().expect("STORAGE_PATH not initialised");
        let mut p = PathBuf::from(storage);
        p.push("wifi.conf");
        p
    }

    fn validate(&self) -> Result<(), ManndError> {
        if let Some(country) = &self.general.country
            && (country.len() != 2 || !country.is_ascii())
        {
            return Err(ManndError::InvalidPropertyFormat(format!(
                "Country must be 2 ASCII characters, got {country}"
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WifiUi {
    pub show_hidden_networks: bool,
    pub sort_networks_by: WifiUiSort,
}

impl Default for WifiUi {
    fn default() -> Self {
        Self {
            show_hidden_networks: false,
            sort_networks_by: WifiUiSort::SignalStrength,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WifiUiSort {
    SignalStrength,
    NameAsc,
    NameDesc,
}

impl WifiUiSort {
    pub fn sort_networks(&self, networks: &mut [NetworkInfo]) {
        let get_tier = |n: &NetworkInfo| -> u8 {
            let is_conn = n.flags.contains(NetworkFlags::CONNECTED);
            let is_known = n.flags.contains(NetworkFlags::KNOWN);
            let is_near = n.flags.contains(NetworkFlags::NEARBY);

            if is_conn {
                0
            } else if is_known && is_near {
                1
            } else if is_near {
                2
            } else {
                3
            }
        };

        match self {
            Self::SignalStrength => {
                networks.sort_by(|a, b| {
                    get_tier(a).cmp(&get_tier(b)).then_with(|| {
                        let signal_ord = match (a.signal_dbm, b.signal_dbm) {
                            (Some(a_sig), Some(b_sig)) => b_sig.cmp(&a_sig),
                            (Some(_), None) => Ordering::Less,
                            (None, Some(_)) => Ordering::Greater,
                            (None, None) => Ordering::Equal,
                        };
                        signal_ord.then_with(|| {
                            a.ssid
                                .to_ascii_lowercase()
                                .cmp(&b.ssid.to_ascii_lowercase())
                        })
                    })
                });
            }
            Self::NameAsc => {
                networks.sort_by(|a, b| {
                    get_tier(a).cmp(&get_tier(b)).then_with(|| {
                        a.ssid
                            .to_ascii_lowercase()
                            .cmp(&b.ssid.to_ascii_lowercase())
                    })
                });
            }
            Self::NameDesc => {
                networks.sort_by(|a, b| {
                    get_tier(a).cmp(&get_tier(b)).then_with(|| {
                        b.ssid
                            .to_ascii_lowercase()
                            .cmp(&a.ssid.to_ascii_lowercase())
                    })
                });
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WifiGeneral {
    pub country: Option<String>,
    pub preferred_interface: Option<String>,
    pub mac_randomization: MacRandomization,
    pub band_type: Option<BandType>,
}

impl Default for WifiGeneral {
    fn default() -> Self {
        Self {
            preferred_interface: None,
            country: None,
            band_type: None,
            mac_randomization: MacRandomization::Always,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WpaPolicy {
    pub allow_freq_mhz: Vec<u32>,
    pub scan_interval_sec: Option<u32>,
    pub autoscan: WpaAutoscan,
    pub fast_reauth: bool,
}

impl Default for WpaPolicy {
    fn default() -> Self {
        Self {
            allow_freq_mhz: Vec::new(),
            scan_interval_sec: Some(5),
            autoscan: WpaAutoscan::Disabled,
            fast_reauth: true,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IwdPolicy {
    pub parse_main_conf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MacRandomization {
    Always,
    Never,
    PeerNetwork, // wpa
    Once,        // iwd
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WpaAutoscan {
    Exponential { base: u32, limit: u32 },
    Periodic { interval: u32 },
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BandType {
    Prefer5GHz,
    Restrict5GHz,
    Prefer2GHz,
    Restrict2GHz,
}
