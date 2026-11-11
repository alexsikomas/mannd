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
//! Because the daemon runs as root, the files it creates (logs, app state, the socket) would be owned by root. Every binary is therefore told which user it serves, the daemon through its required `--target-uid` argument, and uses [`chown`](std::os::unix::fs::chown) to hand what it creates to that user.
//!
//! ## Paths
//! Fail-fast approach, when an essential hardcoded path cannot be resolved panic.
//!
//!  | Name              | Type           | Location                                                                 |
//!  |-------------------|--------------- |--------------------------------------------------------------------------|
//!  | Unix Socket       | Hardcoded      | [`UNIX_SOCK_PATH`]                                                       |
//!  | WG Files          | Hardcoded (\*) | [`WG_DIR`](crate::store::WG_DIR)                                         |
//!  | App Config        | Dynamic        | `$HOME/.config/mannd/settings.conf`                                      |
//!  | App State & Logs  | Configurable   | `storage.path` in the app config, by default `$HOME/.local/state/mannd`  |
//!
//! (\*) May become a field in the configuration file in the future
//!
//! There are other paths not hardcoded into this crate but instead from the installation script or from your package manager. These are the TUI at `/usr/local/bin/mannd`, the daemon at `/usr/local/libexec/mannd-socket` and the optional systemd unit at `/etc/systemd/system/mannd.service`.
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
    sync::{Arc, OnceLock, RwLock},
};

use crate::{
    config::{AppConfig, CONFIG_DIR},
    error::ManndError,
    store::{ApplicationState, ManndStore},
    wireless::wifi_config::WifiConfig,
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

pub const UNIX_SOCK_PATH: &str = "/run/mannd.sock";

#[derive(Debug)]
pub struct GlobalContext {
    /// The user being running app, who owns everything written under `home`
    pub uid: u32,
    pub home: PathBuf,
    pub config_home: PathBuf,
    /// Expanded `storage.path` from the app config
    pub storage: PathBuf,
}

#[derive(Debug)]
pub struct GlobalState {
    pub db: ManndStore,
    pub app: ApplicationState,
    pub wifi_config: Arc<WifiConfig>,
}

pub static APP_CTX: OnceLock<GlobalContext> = OnceLock::new();

/// Resolves the home directory of `uid`, loads that user's settings and
/// sets the global context. Shared entry point of the daemon and the TUI.
pub fn init_ctx(uid: u32) -> Result<AppConfig, ManndError> {
    let home = uid_to_home(uid)?;
    let config = AppConfig::load(&home, uid)?;

    let ctx = GlobalContext {
        uid,
        config_home: home.join(CONFIG_DIR),
        storage: PathBuf::from(&config.storage.path),
        home,
    };

    APP_CTX.set(ctx).map_err(|_| {
        ManndError::OperationFailed("Application context already initialized".into())
    })?;
    Ok(config)
}

static APP_STATE: RwLock<Option<GlobalState>> = RwLock::new(None);

pub struct GlobalStateGuard;
impl GlobalStateGuard {
    pub fn init() -> Result<Self, ManndError> {
        let db = ManndStore::init()?;
        let app = db.get_app_state()?;
        let wifi_config = Arc::new(WifiConfig::load_or_default()?);

        *APP_STATE.write().unwrap() = Some(GlobalState {
            db,
            app,
            wifi_config,
        });

        Ok(Self)
    }
}

impl Drop for GlobalStateGuard {
    fn drop(&mut self) {
        let mut state_lock = APP_STATE.write().unwrap();
        if let Some(ref mut state) = *state_lock
            && let Err(e) = state.db.write_app_state(&state.app)
        {
            tracing::warn!("Failed to save during drop: {}", e.to_string());
        }
        state_lock.take();
    }
}

/// Returns false on None states
pub fn modify_state<F>(modifier: F) -> bool
where
    F: FnOnce(&mut GlobalState),
{
    let mut state_lock = APP_STATE.write().unwrap();
    state_lock.as_mut().map_or(false, |state| {
        modifier(state);
        true
    })
}

pub fn with_state<F, R>(reader: F) -> Option<R>
where
    F: FnOnce(&GlobalState) -> R,
{
    let state_lock = APP_STATE.read().unwrap();
    (*state_lock).as_ref().map(reader)
}

/// # Panics
///
/// If [`GlobalContext`] hasn't been initialised
pub fn context() -> &'static GlobalContext {
    APP_CTX
        .get()
        .expect("Global Context has yet to be initialised")
}

/// Resolves a home directory for a given uid via `getpwuid`.
fn uid_to_home(uid: u32) -> Result<PathBuf, ManndError> {
    let pwd = unsafe { libc::getpwuid(uid) };
    if pwd.is_null() {
        return Err(ManndError::UidHome);
    }
    let home_str = unsafe { CStr::from_ptr((*pwd).pw_dir) };
    Ok(PathBuf::from(home_str.to_string_lossy().into_owned()))
}
