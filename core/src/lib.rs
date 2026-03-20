//! # Core Networking Package Design
//!
//! Networking backend of the `mannd` application,
//! entirely agnostic to the frontend.
//!
//! ## Privileges
//! Binaries within this crate are expected to be run with UID 0,
//! i.e. root.
//!
//! ### Why root?
//! Before, it was expected to use `setcap` to add admin networking
//! capabilities to the program. This caused issues when eventually
//! moving to the general `wpa_supplicant` dbus service which required
//! the root user to access it. To avoid users needing to edit the
//! dbus security policy & fragmenting the backend between `iwd`
//! and `wpa_supplicant` the backend binaries must be run as root.
//!
//! ### File Ownership
//! Because the bianries run as root, files created by them like: logs,
//! app state, etc.. are all owned by root by default. To ensure these
//! files remain accessible for the calling user:
//! - Binaries provide a UID CLI argument to explicitly set the UID of
//! the user. This should be used in almost all cases.
//! - If for some reason the above was not done try to detect the user
//! through the`SUDO_UID` environment variable
//! - If both of the above fail we write inside of /root/ and *DO NOT*
//! change any permissions.
//!
//! If a UID for the user has been obtained then [`chown`](std::os::unix::fs::chown)
//! should be used to give permissions to the user.
//!
//! ## Paths
//! Fail-fast approach, when an essential hardcoded path cannot be
//! resolved panic.
//!
//! | Name | Type | Location |
//! |------|------|----------|
//! | Unix Socket | Hardcoded | [`UNIX_SOCK_PATH`] |
//! | WG Files | Hardcoded* | [`WG_DIR`](crate::store::WG_DIR) |
//! | App Config | Dynamic | `$HOME/.config/mannd` |
//!
//! \* May become a field in the configuration file in the future
//!
//! ## Error Handling
//! The majority of functions will return an [`crate::error::ManndError`], which is a wrapper
//! around other errors with a few custom ones for `mannd`.
//!
//! There are occassions especially in binaries where you may see a `Box<dyn Error>`, this is
//! likely because one of the error cases was a generic error. In general try to avoid dynamic
//! dispatch unless it's unavoidable.
//!
//! It should be noted that the frontend also uses this error type.

#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

use std::{
    path::PathBuf,
    sync::{LazyLock, OnceLock},
};

use crate::ini_parse::IniConfig;

pub mod controller;
pub mod error;
pub mod ini_parse;
pub mod state;
pub mod store;
pub mod systemd;
pub mod utils;
pub mod wireguard;
pub mod wireless;

pub const UNIX_SOCK_PATH: &str = "/tmp/mannd.sock";
pub static HOME: OnceLock<PathBuf> = OnceLock::new();

pub fn init_home_path(uid: Option<u32>) {
    let home: PathBuf;
    match uid {
        Some(id) => match get_user_home_by_uid(id) {
            Some(path) => home = path,
            None => {
                panic!("Got UID of the user who called sudo but cannot find HOME");
            }
        },
        None => {
            match std::env::var("SUDO_UID") {
                Ok(uid_str) => {
                    let uid = u32::from_str_radix(&uid_str, 10).unwrap();
                    match get_user_home_by_uid(uid) {
                        Some(path) => home = path,
                        None => {
                            panic!("Got UID of the user who called sudo but cannot find HOME");
                        }
                    }
                }
                Err(_) => {
                    println!(
                        "Cannot get the UID of the user who called sudo... using /root/ as home."
                    );
                    home = std::path::PathBuf::from("root")
                }
            };
        }
    };

    if HOME.set(home).is_err() {
        panic!("Home has already been initialised")
    }
}

pub static CONFIG_HOME: LazyLock<PathBuf> = LazyLock::new(|| match HOME.get() {
    Some(home) => {
        let mut home_path = home.clone();
        home_path.push(".config/mannd");
        home_path
    }
    None => {
        panic!("Home has not been initialised yet!")
    }
});

pub static SETTINGS: LazyLock<IniConfig> = LazyLock::new(|| {
    let mut config_path = CONFIG_HOME.clone();
    config_path.push("settings.conf");
    IniConfig::new(config_path).unwrap()
});

#[repr(C)]
pub struct passwd {
    pub pw_name: *mut std::ffi::c_char,
    pub pw_passwd: *mut std::ffi::c_char,
    pub pw_uid: std::ffi::c_uint,
    pub pw_gid: std::ffi::c_uint,
    pub pw_gecos: *mut std::ffi::c_char,
    pub pw_dir: *mut std::ffi::c_char,
    pub pw_shell: *mut std::ffi::c_char,
}

#[link(name = "c")]
unsafe extern "C" {
    fn getpwuid(uid: std::ffi::c_uint) -> *mut passwd;
}

#[link(name = "c")]
unsafe extern "C" {
    pub fn geteuid() -> u32;
}

pub fn get_user_home_by_uid(uid: u32) -> Option<std::path::PathBuf> {
    let pwd_ptr = unsafe { getpwuid(uid) };

    if pwd_ptr.is_null() {
        return None;
    }

    let pw_dir_ptr = unsafe { (*pwd_ptr).pw_dir };
    if pw_dir_ptr.is_null() {
        return None;
    }

    let c_str = unsafe { std::ffi::CStr::from_ptr(pw_dir_ptr) };
    Some(std::path::PathBuf::from(
        c_str.to_string_lossy().into_owned(),
    ))
}
