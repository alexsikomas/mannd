use dioxus::prelude::*;
use dioxus_free_icons::{icons::fa_solid_icons::FaGear, Icon};
#[component]
pub fn MenuHeader() -> Element {
    rsx! {
        header { class: "sticky top-0 z-50 border-b border-border bg-card/50 backdrop-blur-sm",
            div { class: "container mx-auto flex items-center justify-between px-4 py-4",
                div { class: "flex items-center gap-3",
                    div {
                        h1 { class: "text-2xl font-bold text-foreground", "WireGuard Manager" }
                        p { class: "text-sm text-muted-foreground", "For Networkd" }
                    }
                }
                button {
                    Icon {
                        width: 30,
                        height: 30,
                        fill: "black",
                        icon: FaGear,
                    }
                }
            }
        }
    }
}
