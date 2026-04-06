//! # Utilities
//!
//! Various utilities that are used by many different parts of the program.
//!
//! Can be used by the frontend aswell.

use std::{
    ffi::CStr,
    fmt::Write,
    fs::{self, OpenOptions, read_dir},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    os::unix::fs::{PermissionsExt, chown},
    path::{Path, PathBuf},
    str::FromStr,
};

use neli::{
    consts::{
        nl::{NlTypeWrapper, NlmF},
        rtnl::{Ifla, Rtm},
    },
    nl::{NlPayload, NlmsghdrBuilder},
    rtnl::{Ifinfomsg, IfinfomsgBuilder, RtattrBuilder},
    socket::asynchronous::NlSocketHandle,
    types::RtBuffer,
    utils::Groups,
};
use tracing::{Level, instrument};
use tracing_error::ErrorLayer;
use tracing_subscriber::{FmtSubscriber, layer::SubscriberExt};

use crate::{
    error::ManndError,
    store::{NetworkInfo, NetworkSecurity},
};

const SYS_NET_PATH: &str = "/sys/class/net";
const SYS_VIRT_PATH: &str = "/sys/devices/virtual";

pub fn setup_logging(
    path: PathBuf,
    max_log_level: Level,
    uid: Option<u32>,
) -> Result<(), ManndError> {
    let mut in_root: Option<bool> = None;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }

        if uid.is_none() {
            in_root = Some(is_path_root(parent));
        }
    }

    let subscriber = FmtSubscriber::builder()
        .compact()
        .with_file(true)
        .with_writer(OpenOptions::new().append(true).create(true).open(&path)?)
        .with_max_level(max_log_level)
        .with_ansi(true)
        .with_line_number(true)
        .with_max_level(Level::INFO)
        .compact()
        .finish();

    let parent = path
        .parent()
        .ok_or_else(|| ManndError::OperationFailed("Cannot get parent directory".to_string()))?;

    if uid.is_some() {
        chown(parent, uid, None)?;
    }
    if uid.is_some() {
        chown(&path, uid, None)?;
    }
    // else if in_root.is_some_and(|r| r) {
    //     fs::set_permissions(parent, fs::Permissions::from_mode(0o644))?;
    //     fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
    // }

    let subscriber = subscriber.with(ErrorLayer::default());

    match tracing::subscriber::set_global_default(subscriber) {
        Err(e) => {
            tracing::error!(
                "{e}\nCould not set the default subscriber! Continuing without logging."
            );
            Err(ManndError::OperationFailed(e.to_string()))
        }
        _ => Ok(()),
    }
}

pub fn is_path_root(path: &Path) -> bool {
    path.starts_with("/root")
}

pub fn ssid_to_hex(ssid: &str) -> String {
    let bytes = ssid.as_bytes();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

#[instrument(err)]
pub async fn get_name(index: u32) -> Result<String, ManndError> {
    let socket =
        NlSocketHandle::connect(neli::consts::socket::NlFamily::Route, None, Groups::empty())?;
    let ifinfomsg = IfinfomsgBuilder::default()
        .ifi_index(index.cast_signed())
        .ifi_family(neli::consts::rtnl::RtAddrFamily::Unspecified)
        .build()?;

    let nlmsg = NlmsghdrBuilder::default()
        .nl_type(Rtm::Getlink)
        .nl_flags(NlmF::REQUEST)
        .nl_payload(NlPayload::Payload(ifinfomsg))
        .build()?;

    socket.send(&nlmsg).await?;

    let messages = socket.recv_all::<NlTypeWrapper, Ifinfomsg>().await?;
    for msg in messages.0 {
        if let Some(payload) = msg.get_payload()
            && let Some(name) = payload
                .rtattrs()
                .get_attr_handle()
                .get_attribute(Ifla::Ifname)
            && let Ok(mut name) = String::from_utf8((*name.rta_payload().as_ref()).to_vec())
        {
            // remove the \0 char
            name.remove(name.len() - 1);
            return Ok(name);
        }
    }

    Ok(String::new())
}

#[instrument(err)]
pub async fn get_index(interface: &'static str) -> Result<u32, ManndError> {
    let socket =
        NlSocketHandle::connect(neli::consts::socket::NlFamily::Route, None, Groups::empty())?;

    let mut index: u32 = 0;
    let mut buf = RtBuffer::new();
    buf.push(
        RtattrBuilder::default()
            .rta_type(Ifla::Ifname)
            .rta_payload(interface)
            .build()?,
    );
    let ifinfomsg = IfinfomsgBuilder::default()
        .ifi_family(neli::consts::rtnl::RtAddrFamily::Unspecified)
        .rtattrs(buf)
        .build()?;

    let nlmsg = NlmsghdrBuilder::default()
        .nl_type(Rtm::Getlink)
        .nl_flags(NlmF::REQUEST | NlmF::DUMP)
        .nl_payload(NlPayload::Payload(ifinfomsg))
        .build()?;

    socket.send(&nlmsg).await?;

    while let Ok(messages) = socket.recv::<NlTypeWrapper, Ifinfomsg>().await {
        for msg in messages.0 {
            if let Some(payload) = msg?.get_payload() {
                let cur_index = payload.ifi_index();
                if let Some(name) = payload
                    .rtattrs()
                    .get_attr_handle()
                    .get_attribute(Ifla::Ifname)
                {
                    let bytes = name.rta_payload().as_ref();

                    match CStr::from_bytes_until_nul(bytes) {
                        Ok(v) => {
                            if v.to_string_lossy().into_owned() == interface {
                                index = (*cur_index).cast_unsigned();
                                return Ok(index);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Could not convert into Cstr. {e}");
                        }
                    }
                }
            }
        }
    }

    if index == 0 {
        return Err(ManndError::OperationFailed(
            "Cannot find wg-mannd index!".to_string(),
        ));
    }
    Ok(index)
}

pub fn list_interfaces() -> Vec<String> {
    let mut res: Vec<String> = vec![];
    let virt_path = PathBuf::from(SYS_VIRT_PATH);
    let virt_path_comp: Vec<_> = virt_path.components().collect();

    if let Ok(dir) = read_dir(SYS_NET_PATH) {
        for entry in dir {
            if entry.is_err() {
                continue;
            }
            let entry = entry.unwrap();

            // check if device is virtual
            if let Ok(real_path) = entry.path().canonicalize() {
                let path_comp: Vec<_> = real_path.components().collect();
                if !path_comp
                    .as_slice()
                    .windows(virt_path_comp.len())
                    .any(|w| w == virt_path_comp.as_slice())
                {
                    res.push(entry.file_name().into_string().unwrap());
                }
            }
        }
    }
    res
}

#[instrument(err)]
pub fn str_to_ip(inp: &str) -> Result<IpAddr, ManndError> {
    if inp.contains('.') {
        Ipv4Addr::from_str(inp).map_or_else(|_| Err(ManndError::StrToIp), |ip| Ok(IpAddr::V4(ip)))
    } else if inp.contains(':') {
        Ipv6Addr::from_str(inp).map_or_else(|_| Err(ManndError::StrToIp), |ip| Ok(IpAddr::V6(ip)))
    } else {
        Err(ManndError::StrToIp)
    }
}

pub fn format_mac_address(mac: &[u8]) -> String {
    if mac.is_empty() {
        return "N/A".to_string();
    }
    mac.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<String>>()
        .join(":")
}

pub fn validate_network(network: &NetworkInfo) -> Result<(), ManndError> {
    if network.ssid.trim().is_empty() {
        return Err(ManndError::InvalidPropertyFormat(
            "SSID cannot be empty".to_string(),
        ));
    }
    if let Some(bssid) = &network.bssid {
        validate_mac_addr(bssid, "bssid")?;
    }
    for entry in &network.bssid_blacklist {
        validate_mac_addr(entry, "bssid_blacklist")?;
    }
    if let Some(bssid) = &network.bssid {
        if network
            .bssid_blacklist
            .iter()
            .any(|x| x.eq_ignore_ascii_case(bssid))
        {
            return Err(ManndError::InvalidPropertyFormat(
                "bssid cannot also exist in bssid_blacklist".to_string(),
            ));
        }
    }
    // Security credentials
    validate_security(&network.security)
}

pub fn validate_security(security: &NetworkSecurity) -> Result<(), ManndError> {
    match security {
        NetworkSecurity::Open | NetworkSecurity::Owe => Ok(()),
        NetworkSecurity::Wpa2 { passphrase } => {
            let len = passphrase.as_bytes().len();
            if !(8..=63).contains(&len) {
                return Err(ManndError::PasswordLength);
            }
            if !passphrase.is_ascii() {
                return Err(ManndError::InvalidPropertyFormat(
                    "WPA2 passphrase must be ASCII (8-63 chars)".to_string(),
                ));
            }
            Ok(())
        }
        NetworkSecurity::Wpa2Hex { psk_hex } => {
            let ok =
                psk_hex.len() == 64 && psk_hex.as_bytes().iter().all(|b| b.is_ascii_hexdigit());
            if !ok {
                return Err(ManndError::InvalidPropertyFormat(
                    "WPA2 hex PSK must be exactly 64 hex characters".to_string(),
                ));
            }
            Ok(())
        }
        NetworkSecurity::Wpa3Sae { password, .. } => {
            if password.is_empty() {
                return Err(ManndError::InvalidPropertyFormat(
                    "WPA3 SAE password cannot be empty".to_string(),
                ));
            }
            Ok(())
        }
        NetworkSecurity::Wpa3Transition { password } => {
            let len = password.as_bytes().len();
            if !(8..=63).contains(&len) {
                return Err(ManndError::PasswordLength);
            }
            if !password.is_ascii() {
                return Err(ManndError::InvalidPropertyFormat(
                    "WPA3 transition password must be ASCII (8-63 chars)".to_string(),
                ));
            }
            Ok(())
        }
    }
}

pub fn validate_mac_addr(value: &str, field: &str) -> Result<(), ManndError> {
    let parts: Vec<&str> = value.split(':').collect();
    let ok = parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.as_bytes().iter().all(|b| b.is_ascii_hexdigit()));
    if ok {
        Ok(())
    } else {
        Err(ManndError::InvalidPropertyFormat(format!(
            "{field} must be MAC format XX:XX:XX:XX:XX:XX"
        )))
    }
}
