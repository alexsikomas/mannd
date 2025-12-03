use std::ffi::CStr;

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

use crate::error::ComError;

pub async fn get_name(index: u32) -> Result<String, ComError> {
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

pub async fn get_index(interface: &'static str) -> Result<u32, ComError> {
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
        return Err(ComError::OperationFailed(
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
