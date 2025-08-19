mod components;
use crate::components::*;
use dioxus::{
    desktop::{Config, WindowBuilder},
    dioxus_core::LaunchConfig, logger::tracing::info, prelude::*,
};
const FAVICON: Asset = asset!("/assets/favicon.ico");
const HEADER_SVG: Asset = asset!("/assets/header.svg");
const TAILWIND_CSS: Asset = asset!("/assets/styles/tailwind.css");
fn main() {
    let window = WindowBuilder::new().with_decorations(false);
    LaunchBuilder::desktop().with_cfg(Config::new().with_window(window)).launch(App);
}
#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: asset!("input.css") }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        MenuHeader {}
    }
}
