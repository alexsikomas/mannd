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
//! through the `SUDO_UID` environment variable
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
//! | Unix Debug Socket | Hardcoded | [`UNIX_SOCK_PATH`] |
//! | WG Files | Hardcoded* | [`WG_DIR`](crate::store::WG_DIR) |
//! | App Config | Dynamic | `$HOME/.config/mannd` |
//!
//! \* May become a field in the configuration file in the future
//!
//! There are other paths not hardcoded into this crate but instead from the
//! installation script or from your package manager. These include the directory
//! where the binary is stored, usually `/usr/local/bin`, and the directory where
//! the installed (not debug) socket is stored, usually `/usr/libexec/`.
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
    ffi::CStr,
    path::PathBuf,
    sync::{LazyLock, OnceLock},
};

use crate::{error::ManndError, ini_parse::IniConfig};

pub mod controller;
pub mod error;
pub mod ini_parse;
pub mod netlink;
pub mod state;
pub mod store;
pub mod systemd;
pub mod utils;
pub mod wireguard;
pub mod wireless;

pub const UNIX_SOCK_PATH: &str = "/tmp/mannd.sock";
pub static HOME: OnceLock<PathBuf> = OnceLock::new();

pub fn init_home_path(uid: Option<u32>) -> Result<(), ManndError> {
    let home: PathBuf;
    match uid {
        Some(id) => {
            let pwd = unsafe { libc::getpwuid(id) };
            if pwd.is_null() {
                return Err(ManndError::UidHome);
            }

            let home_cstr = unsafe { CStr::from_ptr((*pwd).pw_dir) };

            let home_str = home_cstr.to_string_lossy();
            home = PathBuf::from(home_str.into_owned());
        }
        None => {
            match std::env::var("SUDO_UID") {
                Ok(uid_str) => {
                    let uid = uid_str.parse::<u32>()?;
                    let pwd = unsafe { libc::getpwuid(uid) };
                    if pwd.is_null() {
                        return Err(ManndError::UidHome);
                    }

                    let home_cstr = unsafe { CStr::from_ptr((*pwd).pw_dir) };

                    let home_str = home_cstr.to_string_lossy();
                    home = PathBuf::from(home_str.into_owned());
                }
                Err(_) => {
                    println!(
                        "Cannot get the UID of the user who called sudo... using /root/ as home."
                    );
                    home = std::path::PathBuf::from("/root")
                }
            };
        }
    };

    if HOME.set(home).is_err() {
        Err(ManndError::HomeInitialised)
    } else {
        Ok(())
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
