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
    let config = use_resource(move || async move {utils::config::Config::new().await});

    match config.value().as_ref() {
        None => rsx! {
            // TOOD: replace with actual load component
            h1 { "Loading config" }
        },
        Some(config_data) => {
            use_context_provider(|| Signal::new(config_data.clone()));
            rsx! {
                style { {include_str!("../assets/styles/tailwind.css")} }
                style { {include_str!("../assets/styles/main.css")} }
                style { {include_str!("../input.css")} }
                document::Link { rel: "icon", href: FAVICON }
                MenuHeader {}
            }
        },
    }
}

