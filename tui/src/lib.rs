pub mod app;
pub mod components;
pub mod keys;
pub mod state;
pub mod ui;

pub static SETTINGS: std::sync::OnceLock<mannd::config::AppConfig> = std::sync::OnceLock::new();
