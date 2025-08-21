mod components;
use crate::components::*;
use dioxus::{
    desktop::{
        tao::window::WindowAttributes, Config, LogicalPosition, LogicalSize,
        WindowBuilder,
    },
    dioxus_core::LaunchConfig, logger::tracing::info, prelude::*,
};
const FAVICON: Asset = asset!("/assets/favicon.ico");
const HEADER_SVG: Asset = asset!("/assets/header.svg");
fn main() {
    let window = WindowBuilder::new()
        .with_decorations(false)
        .with_inner_size(LogicalSize::new(400, 600))
        .with_min_inner_size(LogicalSize::new(486, 729))
        .with_minimizable(true);
    LaunchBuilder::desktop().with_cfg(Config::new().with_window(window)).launch(App);
}
#[component]
fn App() -> Element {
    rsx! {
        style { {include_str!("../assets/styles/tailwind.css")} }
        style { {include_str!("../assets/styles/main.css")} }
        style { {include_str!("../input.css")} }
        document::Link { rel: "icon", href: FAVICON }
        MenuHeader {}
    }
}
