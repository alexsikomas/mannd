//! # Store
//!
//! State persistence for mannd.
//!
//! ## Tables
//! The store currently has the following tables:
//! [`APPLICATION_TABLE`], [`WG_TABLE`], [`WPA_TABLE`]
//!
//! [`APPLICATION_TABLE`] is used for storing the application's
//! last state, ensures continity between sessions.
//!
//! [`WG_TABLE`] stores metadata for wireguard configuration files.
//! It caches data found in [`WG_DIR`], i.e. files ending in .conf
//! and their attributes.
//!
//! It also contains a country code, this is still under-development.
//! See [wireguard](crate::wireguard) for more.
//!
//! ## Hashing
//! This module uses [`ahash`] crate for all hashmap operations.
//!
//! While [Hasher](std::hash::Hasher) is likely sufficient [`ahash`]
//! was chosen to minimise latency for processing large directories.
//!
//! For instance, if [`WG_DIR`] contains thosuands of configurations,
//! the overhead of creating [`WgMeta`] entries may be large enough
//! to impact the TUI and how long it takes to display the entires.

use ahash::RandomState;
use derive_builder::Builder;
use postcard::{from_bytes, to_allocvec};
use redb::{
    Database, ReadOnlyTable, ReadTransaction, ReadableDatabase, ReadableTable,
    ReadableTableMetadata, TableDefinition, TableError,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, os::unix::fs::chown, path::PathBuf};
use tracing::instrument;

use crate::{
    context,
    error::ManndError,
    wireless::{
        common::NetworkFlags,
        wpa_config::{BandType, MacRandomization, WpaPolicy},
        wpa_supplicant::ManagedInterface,
    },
};

const APP_STATE_KEY: &str = "app_state";
const WPA_STATE_KEY: &str = "wpa_state";

#[derive(Debug)]
pub struct ManndStore {
    database: Database,
}

impl ManndStore {
    #[instrument(err)]
    pub fn init() -> Result<Self, ManndError> {
        let settings = &context().settings;
        let mut home = PathBuf::from(&settings.storage.state);
        fs::create_dir_all(&home)?;
        chown(&home, context().uid, None)?;
        home.push("mannd.redb");
        let database = Database::create(&home)?;
        chown(&home, context().uid, None)?;

        Ok(ManndStore { database })
    }

    pub fn init_from_db(database: Database) -> Self {
        Self { database }
    }
}

const APPLICATION_TABLE: TableDefinition<String, &[u8]> = TableDefinition::new("app_state_table");

/// All state here is taken from the last
/// time the app was run
#[derive(Debug, Serialize, Deserialize)]
pub struct ApplicationState {
    pub wg_running: bool,
    /// Networks that have been connected to
    #[serde(default)]
    pub saved_networks: Vec<NetworkInfo>,
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            wg_running: false,
            saved_networks: vec![],
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WpaState {
    /// Interfaces that were historically connected to but may
    /// not be present anymore
    #[serde(default)]
    pub desired_interfaces: Vec<String>,
    /// Interfaces that you can trust to be present
    #[serde(default)]
    pub managed_interfaces: Vec<ManagedInterface>,
    #[serde(default)]
    pub active_interface: Option<ManagedInterface>,
    // I've decided against having per-interface, possibly
    // if I find myself needing it or it's requested it may
    // be added
    // #[serde(default)]
    // pub interface_configurations: HashMap<String, WpaPolicy, RandomState>,
}

/// Application State
impl ManndStore {
    // returns default app state if table hasn't been made
    #[instrument(err, skip(self))]
    pub fn get_app_state(&self) -> Result<ApplicationState, ManndError> {
        let read = self.database.begin_read()?;
        let Some(table) = read.open_table_opt(APPLICATION_TABLE)? else {
            return Ok(ApplicationState::default());
        };

        if let Some(data) = table.get(APP_STATE_KEY.to_string())? {
            Ok(from_bytes(data.value())?)
        } else {
            Ok(ApplicationState::default())
        }
    }

    #[instrument(err, skip(self))]
    pub fn write_app_state(&self, state: &ApplicationState) -> Result<(), ManndError> {
        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(APPLICATION_TABLE)?;
            let data = to_allocvec(&state)?;
            table.insert(APP_STATE_KEY.to_string(), data.as_slice())?;
        }
        write.commit()?;
        Ok(())
    }

    pub fn get_wpa_state(&self) -> Result<WpaState, ManndError> {
        let read = self.database.begin_read()?;

        let Some(table) = read.open_table_opt(APPLICATION_TABLE)? else {
            return Ok(WpaState::default());
        };

        if let Some(data) = table.get(WPA_STATE_KEY.to_string())? {
            Ok(from_bytes(data.value())?)
        } else {
            Ok(WpaState::default())
        }
    }

    pub fn write_wpa_state(&self, state: &WpaState) -> Result<(), ManndError> {
        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(APPLICATION_TABLE)?;
            let data = to_allocvec(&state)?;
            table.insert(WPA_STATE_KEY.to_string(), data.as_slice())?;
        }
        write.commit()?;
        Ok(())
    }
}

const WG_TABLE: TableDefinition<String, &[u8]> = TableDefinition::new("wg_table");
pub const WG_DIR: &str = "/etc/wireguard/";

// since last_used is the first item derive eq
// to compare by time
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WgMeta {
    // unix timestamp if 0 not used
    pub last_used: u64,
    // ISO 3166-1 alpha-2
    pub country: [u8; 2],
}

/// Wiregard store
impl ManndStore {
    /// Searches WG_DIR for .conf file, returning a hashmap of the filename
    /// and metadata information in the form of WgMeta, country field is
    /// uninitialised
    #[instrument(err, skip(self))]
    pub fn write_wg_files(&self) -> Result<(), ManndError> {
        let mut files: HashMap<String, WgMeta, RandomState> = HashMap::default();
        let dir = match fs::read_dir(WG_DIR) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(ManndError::IoError(e)),
        };

        for entry in dir {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "conf") {
                let meta = fs::metadata(entry.path())?;
                if !meta.is_file() {
                    continue;
                }
                let filename = entry.file_name().into_string().map_err(|_| {
                    ManndError::OperationFailed(
                        "OsString couldn't be converted to string".to_string(),
                    )
                })?;

                files.insert(
                    filename,
                    WgMeta {
                        last_used: 0,
                        country: [0, 0],
                    },
                );
            }
        }

        let db_data = match self.get_wg_data() {
            Ok(Some(data)) => Some(data),
            Ok(None) => None,
            Err(e) => {
                return Err(e);
            }
        };

        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(WG_TABLE)?;
            for file in files.keys() {
                let data = files.get(file).unwrap();
                let name = file.clone();

                // check against db data to not overwrite
                // stored country flags
                if let Some(found_data) = &db_data {
                    if let Some(stored_data) = found_data.get(&name) {
                        let meta = WgMeta {
                            country: stored_data.country,
                            ..*data
                        };
                        let meta_bytes = to_allocvec(&meta)?;
                        table.insert(file.to_string(), meta_bytes.as_slice())?;
                    } else {
                        let meta_bytes = to_allocvec(&data)?;
                        table.insert(file.to_string(), meta_bytes.as_slice())?;
                    }
                } else {
                    let meta_bytes = to_allocvec(&data)?;
                    table.insert(file.clone(), meta_bytes.as_slice())?;
                }
            }
        }
        write.commit()?;
        Ok(())
    }

    #[instrument(err, skip(self))]
    fn get_wg_data(&self) -> Result<Option<HashMap<String, WgMeta, RandomState>>, ManndError> {
        let read = self.database.begin_read()?;

        let Some(table) = read.open_table_opt(WG_TABLE)? else {
            return Ok(None);
        };

        let mut data: HashMap<String, WgMeta, RandomState> =
            HashMap::with_capacity_and_hasher(usize::try_from(table.len()?)?, RandomState::new());

        for item in table.iter()? {
            let (k, v) = item?;
            data.insert(k.value(), from_bytes(v.value())?);
        }
        Ok(Some(data))
    }

    #[instrument(err, skip(self))]
    pub fn ordered_wg_files(&self) -> Result<(Vec<String>, Vec<WgMeta>), ManndError> {
        let mut names: Vec<String> = vec![];
        let mut meta: Vec<WgMeta> = vec![];

        let read = self.database.begin_read()?;
        let table = match read.open_table(WG_TABLE) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok((vec![], vec![])),
            Err(e) => return Err(ManndError::RedbTable(e)),
        };

        for item in table.iter()? {
            let (k, v) = item?;
            names.push(k.value());
            meta.push(from_bytes(v.value())?);
        }

        // sort by filename then use time
        let mut zipped: Vec<_> = names.into_iter().zip(meta).collect();
        zipped.sort_by(|(n1, m1), (n2, m2)| n1.cmp(n2).then(m2.cmp(m1)));

        Ok(zipped.into_iter().unzip())
    }
}

#[derive(Builder, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkInfo {
    pub ssid: String,
    #[builder(default = "false")]
    pub hidden: bool,
    pub security: NetworkSecurity,

    #[builder(default = "true")]
    pub autoconnect: bool,
    #[builder(default = "0")]
    pub priority: i32,
    #[builder(default = "None")]
    pub bssid: Option<String>,
    #[builder(default = "vec![]")]
    pub bssid_blacklist: Vec<String>,
    #[builder(default = "None")]
    pub signal_dbm: Option<i16>,

    #[builder(default = "None")]
    pub mac_randomization: Option<MacRandomization>,
    #[builder(default = "None")]
    pub pmf: Option<PmfMode>,
    #[builder(default = "None")]
    pub wpa_policy_override: Option<WpaNetworkPolicyOverride>,
    #[builder(default = "NetworkFlags::empty()")]
    pub flags: NetworkFlags,
}

#[derive(Builder, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WpaNetworkPolicyOverride {
    #[builder(default = "vec![]")]
    pub allow_freq_mhz: Vec<u32>,
    #[builder(default = "vec![]")]
    pub scan_freq_mhz: Vec<u32>,
    #[builder(default = "None")]
    pub band_type: Option<BandType>,

    #[builder(default = "None")]
    pub pmf: Option<PmfMode>,
    #[builder(default = "None")]
    pub sae_pwe: Option<SaePwe>,

    #[builder(default = "None")]
    pub mac_randomization: Option<MacRandomization>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PmfMode {
    Disabled, // pmf=0
    Optional, // pmf=1 (WPA2)
    Required, // pmf=2 (WPA3)
}

// Eap later
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkSecurity {
    Open,
    Owe,
    Wpa2 {
        passphrase: String,
    },
    Wpa2Hex {
        psk_hex: String,
    },
    Wpa3Sae {
        password: String,
        pwe: Option<SaePwe>,
    },
    Wpa3Transition {
        password: String,
    },
}

impl NetworkSecurity {
    pub fn key_string(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Owe => "owe",
            Self::Wpa2 { .. } | Self::Wpa2Hex { .. } => "wpa2",
            Self::Wpa3Sae { .. } | Self::Wpa3Transition { .. } => "wpa3",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SaePwe {
    HuntAndPeck,   // sae_pwe=0    older
    HashToElement, // sae_pwe=1
    Both,          // sae_pwe=2    wpa_supplicant default
}

pub trait ReadTransactionExt {
    fn open_table_opt<'a, K, V>(
        &'a self,
        table: TableDefinition<'_, K, V>,
    ) -> Result<Option<ReadOnlyTable<K, V>>, TableError>
    where
        K: redb::Key + 'static,
        V: redb::Value + 'static;
}

impl ReadTransactionExt for ReadTransaction {
    fn open_table_opt<'a, K, V>(
        &'a self,
        table: TableDefinition<'_, K, V>,
    ) -> Result<Option<ReadOnlyTable<K, V>>, TableError>
    where
        K: redb::Key + 'static,
        V: redb::Value + 'static,
    {
        match self.open_table(table) {
            Ok(table) => Ok(Some(table)),
            Err(TableError::TableDoesNotExist(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
