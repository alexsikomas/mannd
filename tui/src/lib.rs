pub mod app;
pub mod components;
pub mod keys;
pub mod state;
pub mod ui;

pub static CONFIG_HOME: std::sync::LazyLock<std::path::PathBuf> =
    std::sync::LazyLock::new(|| match std::env::var("XDG_CONFIG_HOME") {
        Ok(val) => std::path::PathBuf::from(val),
        Err(_) => {
            let home = std::env::var_os("HOME");
            match home {
                Some(val) => {
                    let mut path = std::path::PathBuf::from(val);
                    path.push(".config");
                    path
                }
                None => {
                    panic!("Cannot find $HOME directory!");
                }
            }
        }
    });
