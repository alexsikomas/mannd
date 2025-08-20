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
const TAILWIND_CSS: Asset = asset!("/assets/styles/tailwind.css");
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
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: asset!("input.css") }
        document::Link { rel: "stylesheet", href: asset!("assets/styles/main.css") }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        MenuHeader {}
    }
}
