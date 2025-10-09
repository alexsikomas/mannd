/*
* Might revisit in the future but as of now
* I do not plan to support wired networking
*/

// use neli::consts::genl::Cmd;
// use neli_proc_macros::neli_enum;
//
// pub const ETHTOOL_GENL_NAME: &str = "ethtool";
//
// pub struct EthtoolRequest {
//     pub header: Option<EthtoolHeader>,
// }
//
// pub struct EthtoolHeader {
//     pub dev_index: Option<u32>,
//     pub dev_name: Option<String>,
// }
//
// #[neli_enum(serialized_type = "u8")]
// pub enum EthtoolCmds {
//     MsgUserNone = 0,
//     MsgStrsetGet = 1,
//     MsgLinkinfoGet = 2,
//     MsgLinkinfoSet = 3,
//     MsgLinkmodesGet = 4,
//     MsgLinkmodesSet = 5,
//     MsgLinkstateGet = 6,
//     MsgDebugGet = 7,
//     MsgDebugSet = 8,
//     MsgWolGet = 9,
//     MsgWolSet = 10,
//     MsgFeaturesGet = 11,
//     MsgFeaturesSet = 12,
//     MsgPrivflagsGet = 13,
//     MsgPrivflagsSet = 14,
//     MsgRingsGet = 15,
//     MsgRingsSet = 16,
//     MsgChannelsGet = 17,
//     MsgChannelsSet = 18,
//     MsgCoalesceGet = 19,
//     MsgCoalesceSet = 20,
//     MsgPauseGet = 21,
//     MsgPauseSet = 22,
//     MsgEeeGet = 23,
//     MsgEeeSet = 24,
//     MsgTsinfoGet = 25,
//     MsgCableTestAct = 26,
//     MsgCableTestTdrAct = 27,
//     MsgTunnelInfoGet = 28,
//     MsgFecGet = 29,
//     MsgFecSet = 30,
//     MsgModuleEepromGet = 31,
//     MsgStatsGet = 32,
//     MsgPhcVclocksGet = 33,
//     MsgModuleGet = 34,
//     MsgModuleSet = 35,
//     MsgPseGet = 36,
//     MsgPseSet = 37,
//     MsgRssGet = 38,
//     MsgPlcaGetCfg = 39,
//     MsgPlcaSetCfg = 40,
//     MsgPlcaGetStatus = 41,
//     MsgMmGet = 42,
//     MsgMmSet = 43,
//     MsgModuleFwFlashAct = 44,
//     MsgPhyGet = 45,
//     MsgTsconfigGet = 46,
//     MsgTsconfigSet = 47,
//     MsgRssSet = 48,
//     MsgRssCreateAct = 49,
//     MsgRssDeleteAct = 50,
// }
//
// impl Cmd for EthtoolCmds {}
//
// #[neli_enum(serialized_type = "u16")]
// pub enum EthtoolHeaderAttr {
//     Header = 1,
//     DevIndex = 2,
//     DevName = 3,
//     Flags = 4,
//     PhyIndex = 5,
// }
//
// #[neli_enum(serialized_type = "u16")]
// pub enum EthtoolBisetBitAttr {
//     Index = 1,
//     Name = 2,
//     Value = 3,
// }
//
// #[neli_enum(serialized_type = "u16")]
// pub enum EthtoolBisetBitsAttr {
//     Bit = 1,
// }
//
// #[neli_enum(serialized_type = "u16")]
// pub enum EthtoolBisetAttr {
//     Nomask = 1,
//     Size = 2,
//     Bits = 3,
//     Value = 4,
//     Mask = 5,
// }
//
// #[neli_enum(serialized_type = "u16")]
// pub enum EthtoolStringAttr {
//     Index = 1,
//     Value = 2,
// }
