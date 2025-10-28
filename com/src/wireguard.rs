use neli::{
    consts::{
        nl::NlmF,
        rtnl::{Ifla, IflaInfo, Rtm},
    },
    nl::{NlPayload, NlmsghdrBuilder},
    router::asynchronous::NlRouter,
    rtnl::{Ifinfomsg, IfinfomsgBuilder, Rtattr, RtattrBuilder},
    types::{GenlBuffer, RtBuffer},
    utils::Groups,
};

use crate::error::ComError;

struct Wireguard {}

impl Wireguard {
    /// Connects socket and sets up wg0
    async fn start_interface() -> Result<(), ComError> {
        let (router, handle) =
            NlRouter::connect(neli::consts::socket::NlFamily::Route, None, Groups::empty()).await?;

        let mut linked_attrs = RtBuffer::new();
        linked_attrs.push(
            RtattrBuilder::default()
                .rta_type(IflaInfo::Kind)
                .rta_payload("wireguard")
                .build()?,
        );

        let mut attrs = RtBuffer::new();
        attrs.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Ifname)
                .rta_payload("wg0")
                .build()?,
        );

        attrs.push(
            RtattrBuilder::default()
                .rta_type(Ifla::Linkinfo)
                .rta_payload(linked_attrs)
                .build()?,
        );

        let ifinfomsg = IfinfomsgBuilder::default()
            .ifi_family(neli::consts::rtnl::RtAddrFamily::Unspecified)
            .ifi_type(neli::consts::rtnl::Arphrd::Netrom)
            .ifi_index(0)
            .rtattrs(attrs)
            .build()?;

        router
            .send::<Rtm, Ifinfomsg, (), ()>(
                Rtm::Newlink,
                NlmF::REQUEST | NlmF::ACK | NlmF::EXCL | NlmF::CREATE,
                NlPayload::Payload(ifinfomsg),
            )
            .await?;

        Ok(())
    }
}

// tests
#[tokio::test]
async fn init_wg_test() -> Result<(), ComError> {
    Wireguard::start_interface().await?;
    Ok(())
}
