//! # Store Design
//!
//! State persistence for mannd.
//!
//! ## Tables
//! The store currently has the following tables:
//! [`APPLICATION_TABLE`] and [`WG_TABLE`]
//!
//! [`APPLICATION_TABLE`] is used for storing... application state.
//! It ensures continity between sessions. For example, if the last
//! time `mannd` was run the VPN, this table can be used to restore
//! that state without relying on inconsistent netlink queries.
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
use postcard::{from_bytes, to_allocvec};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition, TableError,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, os::unix::fs::PermissionsExt, path::PathBuf};
use tracing::instrument;

use crate::{SETTINGS, error::ManndError, utils::is_path_root};

#[derive(Debug)]
pub struct ManndStore {
    database: Database,
}

impl ManndStore {
    #[instrument(err)]
    pub fn init() -> Result<Self, ManndError> {
        let mut home = PathBuf::from(SETTINGS.get("storage", "state")?);
        let in_root = is_path_root(&home);
        let _ = fs::create_dir_all(&home);
        if !in_root {
            fs::set_permissions(&home, fs::Permissions::from_mode(0o777))?;
        }

        home.push("mannd.redb");
        let database = Database::create(&home)?;
        if !in_root {
            fs::set_permissions(&home, fs::Permissions::from_mode(0o777))?;
        }

        Ok(ManndStore { database })
    }

    pub fn init_from_db(database: Database) -> Self {
        Self { database }
    }
}

const APPLICATION_TABLE: TableDefinition<String, &[u8]> = TableDefinition::new("app_state_table");

/// Application State
impl ManndStore {
    // Returns Ok(None) if app state has not yet been initialised, i.e. table not found.
    #[instrument(err, skip(self))]
    pub fn read_app_state(&self) -> Result<Option<ApplicationState>, ManndError> {
        let read = self.database.begin_read()?;
        match read.open_table(APPLICATION_TABLE) {
            Ok(table) => {
                // count is inclusive
                if let Some(config) = table.get("wpa_interface_count".to_string())? {
                    let app_state: ApplicationState = from_bytes(config.value())?;
                    Ok(Some(app_state))
                } else {
                    Ok(None)
                }
            }
            Err(TableError::TableDoesNotExist(_)) => Ok(None),
            Err(e) => Err(ManndError::RedbTable(e)),
        }
    }

    #[instrument(err, skip(self))]
    pub fn update_wg_app_state(&self, running: bool) -> Result<(), ManndError> {
        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(APPLICATION_TABLE)?;
            let data = to_allocvec(&running)?;
            table.insert("wireguard_running".to_string(), data.as_slice())?;
        }
        write.commit()?;
        Ok(())
    }

    #[instrument(err, skip(self))]
    pub fn add_interface_app_state(&self, interface: &str) -> Result<(), ManndError> {
        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(APPLICATION_TABLE)?;
            let mut count: usize =
                if let Some(count_u8) = table.get("wpa_interface_count".to_string())? {
                    from_bytes(count_u8.value())?
                } else {
                    0
                };

            count += 1;
            let data = to_allocvec(interface)?;
            table.insert(format!("wpa_interface_{count}"), data.as_slice())?;

            table.insert(
                "wpa_interface_count".to_string(),
                to_allocvec(&count)?.as_slice(),
            )?;
        }
        write.commit()?;
        Ok(())
    }

    #[instrument(err, skip(self))]
    pub fn remove_interface_app_state(&self, interface: &str) -> Result<(), ManndError> {
        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(APPLICATION_TABLE)?;

            let mut count: usize =
                if let Some(count_u8) = table.get("wpa_interface_count".to_string())? {
                    from_bytes(count_u8.value())?
                } else {
                    return Err(ManndError::WpaRemoveEmpty);
                };

            if count == 0 {
                return Err(ManndError::WpaRemoveEmpty);
            }

            let mut remove_idx: Option<usize> = None;
            for i in 0..count {
                if let Some(value_u8) = table.get(format!("wpa_interface_{i}"))? {
                    let value: String = from_bytes(value_u8.value())?;
                    if value == interface {
                        remove_idx = Some(i);
                        break;
                    }
                }
            }

            let remove_idx = match remove_idx {
                Some(idx) => idx,
                None => return Err(ManndError::WpaRemoveNotFound),
            };

            if remove_idx != count {
                let last_bytes = {
                    let last_val = table
                        .get(format!("wpa_interface_{count}"))?
                        .ok_or_else(|| ManndError::WpaInterfaceHole)?;
                    last_val.value().to_vec()
                };

                table.insert(format!("wpa_interface_{remove_idx}"), last_bytes.as_slice())?;
            }

            table.remove(format!("wpa_interface_{count}"))?;
            count -= 1;
            table.insert(
                "wpa_interface_count".to_string(),
                to_allocvec(&count)?.as_slice(),
            )?;
        }
        write.commit()?;
        Ok(())
    }
}

/// All state here is taken from the last
/// time the app was run
#[derive(Debug, Serialize, Deserialize)]
pub struct ApplicationState {
    pub wg_running: bool,
    pub managed_interfaces: Vec<String>,
}

impl ApplicationState {
    fn new() -> Self {
        Self {
            wg_running: false,
            managed_interfaces: vec![],
        }
    }
}

const WG_TABLE: TableDefinition<String, &[u8]> = TableDefinition::new("wg_table");
pub const WG_DIR: &'static str = "/etc/wireguard/";

// since last_used is the first item derive eq
// to compare by time
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WgMeta {
    // unix timestamp if 0 not used
    pub last_used: u64,
    pub last_modified: u64,
    // ISO 3166-1 alpha-2
    pub country: [u8; 2],
    // TODO: encode ipv4 ips
}

/// Wiregard store
impl ManndStore {
    /// Searches WG_DIR for .conf file, returning a hashmap of the filename
    /// and metadata information in the form of WgMeta, country field is
    /// uninitialised
    #[instrument(err, skip(self))]
    pub fn update_wg_files(&self) -> Result<(), ManndError> {
        let mut files: HashMap<String, WgMeta, RandomState> = HashMap::default();
        let mut dir = fs::read_dir(WG_DIR)?;
        while let Some(entry) = dir.next() {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "conf") {
                let meta = fs::metadata(entry.path())?;
                if !meta.is_file() {
                    continue;
                }
                let time = meta.modified().unwrap().elapsed().unwrap().as_secs();
                let filename = entry.file_name().into_string().unwrap();

                files.insert(
                    filename,
                    WgMeta {
                        last_used: 0,
                        last_modified: time,
                        country: [0, 0],
                    },
                );
            }
        }

        let db_data = match self.read_wg_data() {
            Ok(data) => Some(data),
            Err(ManndError::RedbTable(TableError::TableDoesNotExist(ref name))) => {
                if name == "wg_table" {
                    // if wg_table doesn't exist then
                    // no data written which isn't an error
                    None
                } else {
                    return Err(ManndError::RedbTable(TableError::TableDoesNotExist(
                        name.clone(),
                    )));
                }
            }
            Err(e) => {
                return Err(e);
            }
        };

        let write = self.database.begin_write()?;
        {
            let mut table = write.open_table(WG_TABLE)?;
            for file in files.keys() {
                let data = files.get(file).unwrap();
                let name = file.to_string();

                // check against db data to not overwrite
                // stored country flags
                match &db_data {
                    Some(found_data) => {
                        match found_data.get(&name) {
                            Some(stored_data) => {
                                let meta = WgMeta {
                                    country: stored_data.country,
                                    ..*data
                                };
                                let meta_bytes = to_allocvec(&meta)?;
                                let _ = table.insert(file.to_string(), meta_bytes.as_slice());
                            }
                            _ => {
                                let meta_bytes = to_allocvec(&data)?;
                                let _ = table.insert(file.to_string(), meta_bytes.as_slice());
                            }
                        };
                    }
                    None => {
                        let meta_bytes = to_allocvec(&data)?;
                        let _ = table.insert(file.to_string(), meta_bytes.as_slice());
                    }
                }
            }
        }
        write.commit()?;
        Ok(())
    }

    #[instrument(err, skip(self))]
    fn read_wg_data(&self) -> Result<HashMap<String, WgMeta, RandomState>, ManndError> {
        let read = self.database.begin_read()?;

        let table = read.open_table(WG_TABLE)?;
        let mut data: HashMap<String, WgMeta, RandomState> =
            HashMap::with_capacity_and_hasher(table.len()? as usize, RandomState::new());

        for item in table.iter()? {
            let (k, v) = item?;
            data.insert(k.value(), from_bytes(v.value())?);
        }
        Ok(data)
    }

    #[instrument(err, skip(self))]
    pub fn get_ordered_wg_files(&self) -> Result<(Vec<String>, Vec<WgMeta>), ManndError> {
        let mut names: Vec<String> = vec![];
        let mut meta: Vec<WgMeta> = vec![];

        let read = self.database.begin_read()?;
        let table = read.open_table(WG_TABLE)?;

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
