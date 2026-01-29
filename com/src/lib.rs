pub mod controller;
pub mod error;
pub mod ini_parse;
pub mod state;
pub mod systemd;
pub mod utils;
pub mod wireguard;
pub mod wireless;

pub const UNIX_SOCK_PATH: &str = "/tmp/mannd.sock";
