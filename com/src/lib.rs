pub mod controller;
pub mod error;
pub mod ini_parse;
pub mod state;
pub mod systemd;
pub mod utils;
pub mod wireguard;
pub mod wireless;

pub const UNIX_SOCK_PATH: &str = "/tmp/mannd.sock";

/// Get uid of user who called sudo and make db
/// in their XDG_STATE
pub static STATE_HOME: std::sync::LazyLock<(std::path::PathBuf, bool)> =
    std::sync::LazyLock::new(|| {
        let (mut home, in_root) = match std::env::var_os("SUDO_UID") {
            Some(uid_str) => {
                let uid_str = uid_str.to_str().unwrap();
                let uid = u32::from_str_radix(uid_str, 10).unwrap();
                match get_user_home_by_uid(uid) {
                    Some(path) => (path, false),
                    None => {
                        tracing::warn!(
                            "Got UID of the user who called sudo but cannot find home..."
                        );
                        (std::path::PathBuf::from("root"), false)
                    }
                }
            }
            None => {
                tracing::warn!(
                    "Cannot get the UID of the user who called sudo... DB will be in /root/"
                );
                (std::path::PathBuf::from("root"), false)
            }
        };

        home.push(".local/state/mannd");
        (home, in_root)
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
