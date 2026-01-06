pub mod de;
pub mod ser;
pub mod wireguard;

use std::{net::IpAddr, str::FromStr};

use redb::{TypeName, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct WgFileTable {
    filename: String,
    // unix timestamp
    last_accessed: i64,
    // ISO 3166-1 alpha-2
    country: [u8; 2],
}

// Binary format as follows:
// [u32 = length of path][u8 = path][i64 = last_accessed][u32; 2 = char]
// the brackets are only for illustration
impl Value for WgFileTable {
    type AsBytes<'a> = Vec<u8>;
    type SelfType<'a> = Self;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let (len, cont) = data
            .split_first_chunk::<{ size_of::<u32>() }>()
            .expect("Too short; cannot read length");
        let len = u32::from_le_bytes(*len) as usize;

        if len > cont.len() {
            panic!("Cannot parse path, too long")
        }

        let (filename, cont) = cont.split_at(len);
        let filename = String::from_bytes(filename);

        let (last_accessed, cont) = cont
            .split_first_chunk::<{ size_of::<i64>() }>()
            .expect("Too short; cannot read access time");
        let last_accessed = i64::from_le_bytes(*last_accessed);

        let (c1_bytes, cont) = cont
            .split_first_chunk::<{ size_of::<u8>() }>()
            .expect("Data too short for country char 1");
        let (c2_bytes, cont) = cont
            .split_first_chunk::<{ size_of::<u8>() }>()
            .expect("Data too short for country char 2");

        if !cont.is_empty() {
            panic!("Unexpected trailing data");
        }

        let c1 = u8::from_le_bytes(*c1_bytes);
        let c2 = u8::from_le_bytes(*c2_bytes);

        Self {
            filename,
            last_accessed,
            country: [c1, c2],
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let file_bytes = value.filename.as_bytes();
        let file_len = file_bytes.len();
        let cap = size_of::<u32>() + file_len + size_of::<i64>() + 2 * size_of::<u32>();

        let mut bytes = Vec::with_capacity(cap);
        bytes.extend_from_slice(&(file_len as u32).to_le_bytes());
        bytes.extend_from_slice(file_bytes);
        bytes.extend_from_slice(&value.last_accessed.to_le_bytes());

        bytes.extend_from_slice(&(value.country[0] as u8).to_le_bytes());
        bytes.extend_from_slice(&(value.country[1] as u8).to_le_bytes());
        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("WgFile")
    }
}

pub struct WgFile {
    interface: WgInterface,
    peer: Vec<WgPeer>,
}

pub struct Cidr {
    ip: IpAddr,
    mask: u8,
}

pub struct WgInterface {
    private_key: String,
    address: Vec<Cidr>,
    listen_port: Option<u16>,
    dns: Option<Vec<IpAddr>>,
    mtu: Option<u16>,
    table: Option<u64>,
    // we ignore PreUp, PostUp, PreDown, PostDown
    save_config: Option<bool>,
}

pub struct WgPeer {
    public_key: String,
    preshared_key: Option<String>,
    allowed_ips: Vec<Cidr>,
    endpoint: Option<String>,
    persistnet_keep_alive: Option<u32>,
}

impl FromStr for Cidr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split("/").collect();
        if parts.len() != 2 {
            return Err("Invalid CIDR format".to_string());
        }

        Ok(Self {
            ip: IpAddr::from_str(parts[0]).map_err(|_| "Invalid IP")?,
            mask: parts[1].parse().map_err(|_| "Invalid mask")?,
        })
    }
}
