use dioxus::{logger::tracing::info, prelude::*};
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaGear, FaXmark},
    Icon,
};
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
                button {
                    onclick: move |_| { settings_open.set(!settings_open()) },
                    class: "p-1",
                    Icon {
                        width: 30,
                        height: 30,
                        fill: "black",
                        icon: FaGear,
                    }
                }
            }
        }
        SettingsMenu { open: settings_open }
    }
}
#[component]
fn SettingsMenu(open: Signal<bool>) -> Element {
    let mut opts = use_signal(|| vec!["WireGuard", "Network"]);
    let mut current = use_signal(|| "WireGuard");
    if open() {
        rsx! {
            div { class: "fixed top-0 left-0 z-100 bg-black w-full h-full opacity-10" }
            div { class: "fixed z-150 w-5/6 h-5/6 top-1/2 left-1/2 translate-[-50%] rounded-lg bg-[#fffbf4]",
                button {
                    onclick: move |_| {
                        open.set(!open());
                    },
                    class: "absolute top-2 right-2",
                    Icon {
                        width: 24,
                        height: 24,
                        fill: "black",
                        icon: FaXmark,
                    }
                }
                div { class: "gap-2 flex p-2",
                    for & item in opts.read().iter() {
                        if *current.read() == item {
                            button { class: "p-1 bg-[#f9c647] rounded-lg", "{item}" }
                        } else {
                            button {
                                onclick: move |_| { current.set(item) },
                                class: "p-1",
                                "{item}"
                            }
                        }
                    }
                }
            }
        }
    } else {
        rsx! {}
    }
}
