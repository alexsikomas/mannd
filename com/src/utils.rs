use std::ffi::CStr;

use neli::{
    consts::{
        nl::NlmF,
        rtnl::{Ifla, Rtm},
    },
    nl::NlPayload,
    router::asynchronous::{NlRouter, NlRouterReceiverHandle},
    rtnl::{Ifinfomsg, IfinfomsgBuilder},
};

use crate::error::ComError;

/// Gets `INTERFACE` index
pub async fn get_index(router: &NlRouter, interface: &'static str) -> Result<u32, ComError> {
    let mut index: u32 = 0;
    let ifinfomsg = IfinfomsgBuilder::default()
        .ifi_family(neli::consts::rtnl::RtAddrFamily::Unspecified)
        .build()?;

    let mut msg: NlRouterReceiverHandle<Rtm, Ifinfomsg> = router
        .send(
            Rtm::Getlink,
            NlmF::REQUEST | NlmF::DUMP,
            NlPayload::Payload(ifinfomsg),
        )
        .await?;

    while let Some(Ok(res)) = msg.next::<Rtm, Ifinfomsg>().await {
        if let Some(payload) = res.get_payload() {
            let cur_index = payload.ifi_index();
            for attr in res.get_payload().unwrap().rtattrs().iter() {
                if (*attr.rta_type() == Ifla::Ifname) {
                    let bytes = attr.rta_payload().as_ref();

                    match CStr::from_bytes_until_nul(bytes) {
                        Ok(v) => {
                            if (v.to_string_lossy().into_owned() == interface) {
                                index = cur_index.clone() as u32;
                                break;
                            }
                        }
                        Err(e) => {}
                    };
                }
            }
        }
    }

    println!("wg-mannd is index: {}", index);

    if (index == 0) {
        return Err(ComError::OperationFailed(
            "Cannot find wg-mannd index!".to_string(),
        ));
    }

    Ok(index)
}
