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
