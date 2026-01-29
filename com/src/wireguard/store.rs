use ahash::RandomState;
use nyquest::{Body, r#async::Request};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition, TypeName,
    Value,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, os::unix::fs::PermissionsExt, path::PathBuf};
use tokio::fs::metadata;
use tracing::{info, warn};

use crate::{error::ManndError, utils::get_user_home_by_uid};

const WG_TABLE: TableDefinition<String, WgMeta> = TableDefinition::new("wg_table");
const WG_DIR: &'static str = "/etc/wireguard/";
const IP_API: &'static str = "http://ip-api.com/batch?fields=status,countryCode";

pub struct WgStore {
    database: Database,
}

impl WgStore {
    pub fn init() -> Result<Self, ManndError> {
        // get uid of user who called sudo and make db
        // in their XDG_STATE, note because we are running
        // as sudo we can't respect the XDG_STATE if it has
        // actually been set
        let (mut home, in_root) = match env::var_os("SUDO_UID") {
            Some(uid_str) => {
                let uid_str = uid_str.to_str().unwrap();
                let uid = u32::from_str_radix(uid_str, 10).unwrap();
                match get_user_home_by_uid(uid) {
                    Some(path) => (path, false),
                    None => {
                        tracing::warn!(
                            "Got UID of the user who called sudo but cannot find home..."
                        );
                        (PathBuf::from("root"), false)
                    }
                }
            }
            None => {
                tracing::warn!(
                    "Cannot get the UID of the user who called sudo... DB will be in /root/"
                );
                (PathBuf::from("root"), false)
            }
        };

        home.push(".local/state/mannd");
        let _ = fs::create_dir_all(&home);
        if !in_root {
            fs::set_permissions(&home, fs::Permissions::from_mode(0o777))?;
        }

        home.push("wg.redb");
        let database = Database::create(&home)?;
        if !in_root {
            fs::set_permissions(&home, fs::Permissions::from_mode(0o777))?;
        }
        Ok(WgStore { database })
    }

    pub fn init_from_db(database: Database) -> Self {
        Self { database }
    }

    /// Searches WG_DIR for .conf file, returning a hashmap of the filename
    /// and metadata information in the form of WgMeta, country field is
    /// uninitialised
    pub fn update_files(&self) -> Result<(), ManndError> {
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

        let db_data = match self.db_data() {
            Ok(data) => Some(data),
            Err(e) => match e {
                ManndError::RedbTable(ref table) => {
                    // if wg_table does not exist then we haven't written any data
                    // which is not an error
                    if table.to_string().eq("Table 'wg_table' does not exist") {
                        None
                    } else {
                        return Err(e);
                    }
                }
                _ => {
                    return Err(e);
                }
            },
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
                                let _ = table.insert(file.to_string(), meta);
                            }
                            _ => {
                                let _ = table.insert(file.to_string(), data);
                            }
                        };
                    }
                    None => {
                        let _ = table.insert(file.to_string(), data);
                    }
                }
            }
        }
        write.commit()?;
        Ok(())
    }

    fn db_data(&self) -> Result<HashMap<String, WgMeta, RandomState>, ManndError> {
        let read = self.database.begin_read()?;

        let table = read.open_table(WG_TABLE)?;
        let mut data: HashMap<String, WgMeta, RandomState> =
            HashMap::with_capacity_and_hasher(table.len()? as usize, RandomState::new());

        for item in table.iter()? {
            let (k, v) = item?;
            data.insert(k.value(), v.value());
        }
        Ok(data)
    }

    pub fn get_ordered_files(&self) -> Result<(Vec<String>, Vec<WgMeta>), ManndError> {
        let mut names: Vec<String> = vec![];
        let mut meta: Vec<WgMeta> = vec![];

        let read = self.database.begin_read()?;
        let table = read.open_table(WG_TABLE)?;

        for item in table.iter()? {
            let (k, v) = item?;
            names.push(k.value());
            meta.push(v.value());
        }

        // sort by filename then use time
        let mut zipped: Vec<_> = names.into_iter().zip(meta).collect();
        zipped.sort_by(|(n1, m1), (n2, m2)| n1.cmp(n2).then(m2.cmp(m1)));

        Ok(zipped.into_iter().unzip())
    }

    pub async fn get_countries() {
        // Request::post(IP_API).with_body(Body::json_bytes)
    }

    fn map_ips_to_json(&self) -> Result<(), ManndError> {
        let info = self.db_data()?;
        todo!()
    }
}

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

impl Value for WgMeta {
    type SelfType<'a> = Self;
    type AsBytes<'a> = [u8; 24]; // used: 8 + mod: 8 + country: 2, then rust rounds to 24

    fn fixed_width() -> Option<usize> {
        Some(24)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let mut meta = WgMeta {
            last_used: 0,
            last_modified: 0,
            country: [0, 0],
        };

        let (used_time, cont) = data
            .split_first_chunk::<{ size_of::<u64>() }>()
            .expect("Too short; cannot read last used time");
        meta.last_used = u64::from_bytes(used_time);

        let (modified_time, cont) = cont
            .split_first_chunk::<{ size_of::<u64>() }>()
            .expect("Too short; cannot get access time");

        let modified_time = u64::from_le_bytes(*modified_time);
        meta.last_modified = modified_time;

        let (c1_byte, cont) = cont
            .split_first_chunk::<{ size_of::<u8>() }>()
            .expect("Too short; cannot read country");
        let c1 = u8::from_bytes(c1_byte);

        let (c2_byte, cont) = cont
            .split_first_chunk::<{ size_of::<u8>() }>()
            .expect("Too short; cannot read country");
        let c2 = u8::from_bytes(c2_byte);

        meta.country = [c1, c2];

        if cont.len() > (24 - (size_of::<u64>() * 2 + size_of::<u8>() * 2)) {
            warn!("Trailing data is longer than expected, continuing anyway...");
        }
        meta
    }

    // Encoding: [last_used][last_modified][country]
    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = [0u8; 24];
        let remaining = &mut bytes[..];

        let (last_used, cont) = remaining.split_at_mut(size_of::<u64>());
        last_used.copy_from_slice(&value.last_used.to_le_bytes());
        let (last_modif, cont) = cont.split_at_mut(size_of::<u64>());
        last_modif.copy_from_slice(&value.last_modified.to_le_bytes());
        let (country, _) = cont.split_at_mut(size_of::<u8>() * 2);
        country.copy_from_slice(&value.country);
        bytes
    }

    fn type_name() -> redb::TypeName {
        TypeName::new("WgMeta")
    }
}
