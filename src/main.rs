mod components;
mod utils;

use std::path::PathBuf;

use crate::components::*;
use crate::utils::*;

use dioxus::{
    desktop::{Config, LogicalSize, WindowBuilder},
    prelude::*,
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const HEADER_SVG: Asset = asset!("/assets/header.svg");

fn main() {
    let window = WindowBuilder::new()
        .with_decorations(false)
        .with_inner_size(LogicalSize::new(400, 600))
        .with_min_inner_size(LogicalSize::new(486, 729))
        .with_minimizable(true);
    LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(App);
}

#[component]
fn App() -> Element {
    use_context_provider(|| Signal::new(utils::config::Config::new().unwrap()));

    rsx! {
        style { {include_str!("../assets/styles/tailwind.css")} }
        style { {include_str!("../assets/styles/main.css")} }
        style { {include_str!("../input.css")} }
        document::Link { rel: "icon", href: FAVICON }
        MenuHeader {}
    }
}

pub enum Message {
    Config(ConfigMessage),
}

pub enum MessageError {}

/// Operations that may be performed on a path,
/// used by `UpdateWgPath`
pub enum PathOptions {
    Add,
    Remove,
    RemoveAll,
}

pub enum ConfigMessage {
    UpdateWgPath(PathBuf, PathOptions),
    UpdateNetworkPath(PathBuf),
    UpdateInterface(String),
    UpdateBoot(bool),
}
