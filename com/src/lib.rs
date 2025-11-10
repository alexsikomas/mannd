#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

pub mod controller;
pub mod error;
pub mod netlink;
pub mod systemd;
pub mod utils;
pub mod wireguard;
pub mod wireless;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
