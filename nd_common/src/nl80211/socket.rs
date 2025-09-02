/*
* Portions of this code are taken from neli-wifi: https://docs.rs/neli-wifi/latest/neli_wifi/
* They are edited to work with the newest version of neli for compatability with this project
*
* All rights of the original code belong to the author(s)
*/

use neli::{
    consts::{
        nl::{NlmF, Nlmsg},
        socket::NlFamily,
    },
    err::{DeError, MsgError},
    genl::{Genlmsghdr, GenlmsghdrBuilder, NlattrBuilder, NoUserHeader},
    nl::NlPayload,
    router::asynchronous::{NlRouter, NlRouterReceiverHandle},
    types::GenlBuffer,
    utils::Groups,
};

use crate::{
    error::NetworkdLibError,
    nl80211::defs::{Attrs, Nl80211Attr, Nl80211Cmd, NL_80211_GENL_NAME},
};

pub struct Wifi {
    router: NlRouter,
    handle: NlRouterReceiverHandle<u16, Genlmsghdr<u8, u16, NoUserHeader>>,
    family_id: u16,
}

impl Wifi {
    /// Creates a new nl80211 socket with netlink
    pub async fn connect() -> Result<Self, NetworkdLibError> {
        let (mut router, mut handle) =
            NlRouter::connect(NlFamily::Generic, None, Groups::empty()).await?;
        let family_id = router.resolve_genl_family(NL_80211_GENL_NAME).await?;

        Ok(Self {
            router,
            handle,
            family_id,
        })
    }

    /// Querys the nl80211 subsystem for a dump of information based on them `cmd` argument,
    /// pass `None` for `interface_index` for all interfaces
    pub async fn get_info_vec<T>(
        &mut self,
        interface_index: Option<i32>,
        cmd: Nl80211Cmd,
    ) -> Result<Vec<T>, NetworkdLibError>
    where
        T: for<'a> TryFrom<Attrs<'a, Nl80211Attr>, Error = DeError>,
    {
        let msghdr = GenlmsghdrBuilder::<Nl80211Cmd, Nl80211Attr>::default()
            .cmd(cmd)
            .attrs({
                let mut attrs = GenlBuffer::new();
                if let Some(interface_index) = interface_index {
                    attrs.push(
                        NlattrBuilder::default()
                            .nla_type(
                                neli::genl::AttrTypeBuilder::default()
                                    .nla_type(Nl80211Attr::AttrIfindex)
                                    .build()
                                    .unwrap(),
                            )
                            .nla_payload(interface_index)
                            .build()
                            .unwrap(),
                    );
                }
                attrs
            })
            .build()
            .unwrap();

        let mut recv: NlRouterReceiverHandle<Nlmsg, Genlmsghdr<Nl80211Cmd, Nl80211Attr>> = self
            .router
            .send(
                self.family_id,
                NlmF::REQUEST | NlmF::DUMP,
                NlPayload::Payload(msghdr),
            )
            .await?;

        let mut retval = Vec::new();

        while let Some(response) = recv
            .next::<Nlmsg, Genlmsghdr<Nl80211Cmd, Nl80211Attr>>()
            .await
        {
            let response = response?;
            match response.nl_type() {
                Nlmsg::Noop => (),
                Nlmsg::Error => {
                    return Err(NetworkdLibError::NeliMsgError(MsgError::new(
                        "Parsing response.nl_type in get_info_vec",
                    )))
                }
                Nlmsg::Done => return Ok(retval),
                _ => retval.push(
                    response
                        .get_payload()
                        .unwrap()
                        .attrs()
                        .get_attr_handle()
                        .try_into()?,
                ),
            };
        }

        Ok(retval)
    }
}
