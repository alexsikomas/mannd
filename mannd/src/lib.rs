//! # Core Networking Package Design
//!
//! Networking backend of the `mannd` application, entirely agnostic to the frontend.
//!
//! ## Privileges
//! Binaries within this crate are expected to be run with UID 0, i.e. root.
//!
//! ### Why root?
//! Before, it was expected to use `setcap` to add admin networking capabilities to the program. This caused issues when eventually moving to the general `wpa_supplicant` dbus service which required the root user to access it.
//!
//! To avoid users needing to edit the dbus security policy & fragmenting the backend between `iwd` and `wpa_supplicant` the backend binaries must be run as root.
//!
//! ### File Ownership
//! Because the bianries run as root, files created by them like: logs, app state, etc.. are all owned by root by default. To ensure these files remain accessible for the calling user:
//! - Binaries provide a UID CLI argument to explicitly set the UID of the user. This should be used in almost all cases.
//! - If for some reason the above was not done try to detect the user through the `SUDO_UID` environment variable
//! - If both of the above fail we write inside of /root/ and *DO NOT* change any permissions.
//!
//! If a UID for the user has been obtained then [`chown`](std::os::unix::fs::chown) should be used to give permissions to the user.
//!
//! ## Paths
//! Fail-fast approach, when an essential hardcoded path cannot be resolved panic.
//!
//! | Name | Type | Location |
//! |------|------|----------|
//! | Unix Debug Socket | Hardcoded | [`UNIX_SOCK_PATH`] |
//! | WG Files | Hardcoded* | [`WG_DIR`](crate::store::WG_DIR) |
//! | App Config | Dynamic | `$HOME/.config/mannd` |
//!
//! \* May become a field in the configuration file in the future
//!
//! There are other paths not hardcoded into this crate but instead from the installation script or from your package manager. These include the directory where the binary is stored, usually `/usr/local/bin`, and the directory where the installed (not debug) socket is stored, usually `/usr/libexec/`.
//!
//! ## Error Handling
//! The majority of functions will return an [`crate::error::ManndError`], which is a wrapper around other errors with a few custom ones for `mannd`.
//!
//! There are occassions especially in binaries where you may see a `Box<dyn Error>`, this is likely because one of the error cases was a generic error. In general try to avoid dynamic dispatch unless it's unavoidable.
//!
//! It should be noted that the frontend also uses this error type.

#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

use std::{
    ffi::CStr,
    path::PathBuf,
    sync::{OnceLock, RwLock},
};

use crate::{
    config::AppConfig,
    error::ManndError,
    store::{ApplicationState, ManndStore},
};

pub mod config;
pub mod controller;
pub mod error;
pub mod netlink;
pub mod state;
pub mod store;
pub mod systemd;
pub mod utils;
pub mod wireguard;
pub mod wireless;

pub const UNIX_SOCK_PATH: &str = "/tmp/mannd.sock";

#[derive(Debug)]
pub struct GlobalContext {
    pub uid: Option<u32>,
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub settings: AppConfig,
}

#[derive(Debug)]
pub struct GlobalState {
    pub db: ManndStore,
    pub app: ApplicationState,
}

pub static APP_CTX: OnceLock<GlobalContext> = OnceLock::new();

pub fn init_ctx(uid: Option<u32>) -> Result<(), ManndError> {
    let home = home_path(uid)?;
    let config_home = home.join(".config/mannd");
    let settings_path = config_home.join("settings.conf");
    let settings = AppConfig::load(settings_path, Some(&home))?;

    let ctx = GlobalContext {
        uid,
        home,
        config_home,
        settings,
    };

    APP_CTX.set(ctx).map_err(|_| {
        ManndError::OperationFailed("Application context already initialized".into())
    })?;
    Ok(())
}

static APP_STATE: RwLock<Option<GlobalState>> = RwLock::new(None);

pub struct GlobalStateGuard;

impl GlobalStateGuard {
    // As well as GlobalState inits a few oncelocks
    pub fn init() -> Result<Self, ManndError> {
        let db = ManndStore::init()?;
        let app = db.get_app_state()?;

        *APP_STATE.write().unwrap() = Some(GlobalState { db, app });

        Ok(GlobalStateGuard)
    }
}

impl Drop for GlobalStateGuard {
    fn drop(&mut self) {
        let mut state_lock = APP_STATE.write().unwrap();
        if let Some(ref mut state) = *state_lock {
            if let Err(e) = state.db.write_app_state(&state.app) {
                tracing::warn!("Failed to save during drop: {}", e.to_string());
            }
        }
        state_lock.take();
    }
}

// false on None state
pub fn modify_global<F>(modifier: F) -> bool
where
    F: FnOnce(&mut GlobalState),
{
    let mut state_lock = APP_STATE.write().unwrap();
    if let Some(ref mut state) = *state_lock {
        modifier(state);
        true
    } else {
        false
    }
}

// often actually used to invoke methods probably going to
// make a new function to be explicit about usage
pub fn read_global<F, R>(reader: F) -> Option<R>
where
    F: FnOnce(&GlobalState) -> R,
{
    let state_lock = APP_STATE.read().unwrap();

    if let Some(state) = &*state_lock {
        Some(reader(state))
    } else {
        None
    }
}

pub fn context() -> &'static GlobalContext {
    APP_CTX
        .get()
        .expect("Global Context has yet to be initialised")
}

fn home_path(uid: Option<u32>) -> Result<PathBuf, ManndError> {
    match uid {
        Some(id) => {
            let pwd = unsafe { libc::getpwuid(id) };
            if pwd.is_null() {
                return Err(ManndError::UidHome);
            }

            let home_str = unsafe { CStr::from_ptr((*pwd).pw_dir) };

            let home_str = home_str.to_string_lossy();
            Ok(PathBuf::from(home_str.into_owned()))
        }
        None => {
            if let Ok(uid_str) = std::env::var("SUDO_UID") {
                let uid = uid_str.parse::<u32>()?;
                let pwd = unsafe { libc::getpwuid(uid) };
                if pwd.is_null() {
                    return Err(ManndError::UidHome);
                }

                let home_str = unsafe { CStr::from_ptr((*pwd).pw_dir) };

                let home_str = home_str.to_string_lossy();
                Ok(PathBuf::from(home_str.into_owned()))
            } else {
                println!("Cannot get the UID of the user who called sudo... using /root/ as home.");
                Ok(std::path::PathBuf::from("/root"))
            }
        }
    }
}
