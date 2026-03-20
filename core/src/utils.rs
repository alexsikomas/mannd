use std::{
    env,
    ffi::CStr,
    fs::{self, File, OpenOptions, read_dir},
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
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
use tracing_subscriber::{FmtSubscriber, fmt::format::FmtSpan, layer::SubscriberExt};

use crate::error::ManndError;

const SYS_NET_PATH: &'static str = "/sys/class/net";
const SYS_VIRT_PATH: &'static str = "/sys/devices/virtual";

pub fn setup_logging(path: PathBuf, max_log_level: Level) {
    let mut in_root = false;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            // potential error
            fs::create_dir_all(parent);
        }
        in_root = is_path_root(&parent.to_path_buf());
    }
    let subscriber = FmtSubscriber::builder()
        .compact()
        .with_file(true)
        .with_writer(
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(&path)
                .unwrap(),
        )
        .with_max_level(max_log_level)
        .with_ansi(true)
        .with_line_number(true)
        .with_max_level(Level::INFO)
        .compact()
        .finish();

    if !in_root {
        fs::set_permissions(path.parent().unwrap(), fs::Permissions::from_mode(0o777));
        fs::set_permissions(path, fs::Permissions::from_mode(0o777));
    }

    let subscriber = subscriber.with(ErrorLayer::default());

    match tracing::subscriber::set_global_default(subscriber) {
        Err(e) => {
            tracing::error!(
                "{e}\nCould not set the default subscriber! Continuing without logging."
            )
        }
        _ => {}
    }
}

pub fn is_path_root(path: &PathBuf) -> bool {
    if path.starts_with("/root") {
        true
    } else {
        false
    }
}

#[instrument(err)]
pub async fn get_name(index: u32) -> Result<String, ManndError> {
    let socket =
        NlSocketHandle::connect(neli::consts::socket::NlFamily::Route, None, Groups::empty())?;
    let ifinfomsg = IfinfomsgBuilder::default()
        .ifi_index(index as i32)
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
        if let Some(payload) = msg.get_payload() {
            if let Some(name) = payload
                .rtattrs()
                .get_attr_handle()
                .get_attribute(Ifla::Ifname)
            {
                if let Ok(mut name) = String::from_utf8((*name.rta_payload().as_ref()).to_vec()) {
                    // remove the \0 char
                    name.remove(name.len() - 1);
                    return Ok(name);
                }
            }
        }
    }

    Ok("".to_string())
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
        for msg in messages.0.into_iter() {
            if let Some(payload) = msg.unwrap().get_payload() {
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
                                index = cur_index.clone() as u32;
                                return Ok(index);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Could not convert into Cstr. {e}");
                        }
                    };
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

    if let Ok(mut dir) = read_dir(SYS_NET_PATH) {
        while let Some(entry) = dir.next() {
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
    // we expect ipv4
    if inp.contains(".") {
        match Ipv4Addr::from_str(inp) {
            Ok(ip) => Ok(IpAddr::V4(ip)),
            Err(_) => Err(ManndError::StrToIp),
        }
    } else if inp.contains(":") {
        match Ipv6Addr::from_str(inp) {
            Ok(ip) => Ok(IpAddr::V6(ip)),
            Err(_) => Err(ManndError::StrToIp),
        }
    } else {
        Err(ManndError::StrToIp)
    }
}

pub fn format_mac_address(mac: &[u8]) -> String {
    if mac.is_empty() {
        return "N/A".to_string();
    }
    mac.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<String>>()
        .join(":")
}
