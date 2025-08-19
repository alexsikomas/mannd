use dioxus::{
    desktop::{Config, WindowBuilder},
    dioxus_core::LaunchConfig, prelude::*,
};
const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
fn main() {
    let window = WindowBuilder::new().with_decorations(false);
    LaunchBuilder::desktop().with_cfg(Config::new().with_window(window)).launch(App);
}
#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Hero {}
    }
}
#[component]
pub fn Hero() -> Element {
    rsx! {
        div { id: "main",
            div {
                p { "Test" }
            }
        }
    }
}
