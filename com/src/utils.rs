use std::{
    env,
    ffi::CStr,
    fs::{File, OpenOptions},
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use neli::{
    consts::{
        nl::{NlTypeWrapper, NlmF},
        rtnl::{Ifla, Rtm},
    },
    nl::{NlPayload, NlmsghdrBuilder},
    rtnl::{Ifinfomsg, IfinfomsgBuilder},
    socket::asynchronous::NlSocketHandle,
    utils::Groups,
};

use crate::error::ManndError;

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

pub async fn get_index(interface: &'static str) -> Result<u32, ManndError> {
    let socket =
        NlSocketHandle::connect(neli::consts::socket::NlFamily::Route, None, Groups::empty())?;

    let mut index: u32 = 0;
    let ifinfomsg = IfinfomsgBuilder::default()
        .ifi_family(neli::consts::rtnl::RtAddrFamily::Unspecified)
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

pub fn format_mac_address(mac: &[u8]) -> String {
    if mac.is_empty() {
        return "N/A".to_string();
    }
    mac.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<String>>()
        .join(":")
}

pub struct NamedTempFile {
    pub path: PathBuf,
    pub file: File,
}

impl NamedTempFile {
    pub fn new() -> io::Result<Self> {
        let tmp_dir = env::temp_dir();

        let mut tries = 0;
        loop {
            let mut path = tmp_dir.clone();
            path.push(format!(
                "tmp_{}_{}",
                std::process::id(),
                generate_random_suffix()
            ));

            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok(Self { path, file }),
                Err(_) => {
                    tries += 1;
                    if tries > 100 {
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            "Cannot create unique temporary file name.",
                        ));
                    }
                    continue;
                }
            }
        }
    }
}

impl Drop for NamedTempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn generate_random_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
