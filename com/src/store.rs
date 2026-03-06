use ahash::RandomState;
use postcard::{from_bytes, to_allocvec};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition, TableError,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, os::unix::fs::PermissionsExt};

use crate::{STATE_HOME, error::ManndError};

#[derive(Debug)]
pub struct ManndStore {
    database: Database,
}

impl ManndStore {
    pub fn init() -> Result<Self, ManndError> {
        let home = &STATE_HOME.0;
        let in_root = STATE_HOME.1;
        let _ = fs::create_dir_all(home);
        if !in_root {
            fs::set_permissions(home, fs::Permissions::from_mode(0o777))?;
        }

        let home = STATE_HOME.0.join("mannd.redb");
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
