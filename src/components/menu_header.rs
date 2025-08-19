use dioxus::{logger::tracing::info, prelude::*};
use dioxus_free_icons::{icons::fa_solid_icons::FaGear, Icon};
#[component]
pub fn MenuHeader() -> Element {
    let mut settings_open = use_signal(|| false);
    rsx! {
        header { class: "sticky top-0 z-50 w-full border-b",
            div { class: "container mx-auto flex items-center justify-between px-4 py-4",
                div { class: "flex items-center gap-3",
                    div {
                        h1 { class: "text-2xl font-bold text-foreground", "WireGuard Manager" }
                        p { class: "text-sm text-muted-foreground", "For Networkd" }
                    }
                }
                button { onclick: move |_| { settings_open.set(!settings_open()) },
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
#[component]
fn SettingsMenu(open: Signal<bool>) -> Element {
    if open() {
        rsx! {
            h2 { "TEST" }
        }
    } else {
        rsx! {}
    }
}
