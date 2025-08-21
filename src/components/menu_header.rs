use dioxus::prelude::*;
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaChevronDown, FaFolderPlus, FaGear, FaXmark},
    Icon,
};
#[component]
/// Header component containing the application name, subtitle and settings icon
pub fn MenuHeader() -> Element {
    let mut settings_open = use_signal(|| false);
    rsx! {
        header { class: "sticky top-0 z-50 w-full border-b-1 border-gray-500/50 bg-muted shadow-sm",
            div { class: "container mx-auto flex items-center justify-between px-6 py-5",
                div { class: "flex items-center gap-3",
                    div {
                        h1 { class: "text-3xl font-bold text-foreground", "WireGuard Manager" }
                        p { class: "text-sm text-foreground/80 font-medium tracking-wide",
                            "For Networkd"
                        }
                    }
                }
                button {
                    onclick: move |_| {
                        settings_open.toggle();
                    },
                    class: "p-3 bg-accent hover:bg-accent-2 rounded-xl shadow-lg hover:shadow-xl transition-all duration-200 transform hover:scale-105 active:scale-95 hover:outline-2 focus:outline-none",
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
/// Renders the settings menu, takes in boolean to determine if to render
fn SettingsMenu(open: Signal<bool>) -> Element {
    let opts = use_signal(|| vec!["WireGuard", "Network"]);
    let mut current = use_signal(|| "WireGuard");
    if open() {
        rsx! {
            div { class: "fixed inset-0 z-50 transition-all ease-out duration-500 bg-black/60 backdrop-blur-sm hover:bg-black/70",
                button {
                    onclick: move |_| {
                        open.toggle();
                    },
                    class: "w-full h-full",
                }
            }
            div { class: "fixed z-50 w-11/12 max-w-4xl h-5/6 top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 rounded-2xl bg-gradient-to-br from-[#fffbf4] to-[#fbf8f1] shadow-2xl border border-gray-200/50 animate-in fade-in-0 zoom-in-95 duration-300",
                button {
                    onclick: move |_| {
                        open.toggle();
                    },
                    class: "absolute top-6 right-4 p-2 bg-warn hover:bg-warn-2 rounded-xl shadow-md hover:shadow-lg transition-all duration-100 transform hover:scale-105 hover:outline-2 focus:outline-none",
                    Icon {
                        width: 32,
                        height: 32,
                        fill: "black",
                        icon: FaXmark,
                    }
                }
                div { class: "flex gap-3 p-6 pb-4 border-b border-gray-200/50",
                    for & item in opts.read().iter() {
                        if *current.read() == item {
                            button { class: "px-6 py-3 bg-accent-2 text-gray-800 font-semibold rounded-xl shadow-lg transform scale-105 transition-all duration-100 outline-2",
                                "{item}"
                            }
                        } else {
                            button {
                                onclick: move |_| { current.set(item) },
                                class: "px-6 py-3 bg-white/80 hover:bg-accent/20 text-gray-600 hover:text-gray-800 font-medium rounded-xl shadow-md hover:shadow-lg transition-all duration-100 transform hover:scale-105",
                                "{item}"
                            }
                        }
                    }
                }
                div { class: "p-6 overflow-y-auto max-h-full",
                    if *current.read() == "WireGuard" {
                        WireguardMenu {}
                    } else {
                        NetworkMenu {}
                    }
                }
            }
        }
    } else {
        rsx! {}
    }
}
#[component]
/// Renders the WireGuard section in the settings menu
fn WireguardMenu() -> Element {
    let mut is_open = use_signal(|| false);
    rsx! {
        div {
            class: format!(
                "transition-all ease-out duration-300 w-full mx-auto bg-white/80 shadow-lg rounded-2xl overflow-hidden border border-gray-200/50 {}",
                if *is_open.read() { "" } else { "hover:border-accent/20" },
            ),
            div { class: "rounded-2xl",
                h2 { class: "mb-0",
                    button {
                        class: format!(
                            "flex items-center justify-between w-full p-6 font-semibold text-left rounded-2xl {}",
                            if *is_open.read() {
                                "bg-accent text-gray-800 shadow-lg"
                            } else {
                                "bg-gradient-to-r from-white/90 to-[#fbf8f1] text-gray-700 hover:from-[#f9c647]/10 hover:to-[#f7b731]/10"
                            },
                        ),
                        onclick: move |_| {
                            is_open.toggle();
                        },
                        span { class: "text-lg", "Folders" }
                        span {
                            class: format!(
                                "text-xl transition-transform {}",
                                if *is_open.read() { "rotate-180" } else { "" },
                            ),
                            Icon {
                                width: 16,
                                height: 16,
                                fill: "black",
                                icon: FaChevronDown,
                            }
                        }
                    }
                }
                div {
                    class: format!(
                        "grid transition-[grid-template-rows] duration-300 ease-in-out {}",
                        if *is_open.read() { "grid-rows-[1fr]" } else { "grid-rows-[0fr]" },
                    ),
                    div { class: "overflow-hidden",
                        div { class: "p-6 bg-gradient-to-b from-white/50 to-[#fbf8f1]/30",
                            button { class: "flex items-center gap-3 px-6 py-4 bg-accent hover:bg-accent-2 text-gray-800 font-semibold rounded-xl shadow-lg hover:shadow-xl hover:scale-105 active:scale-95",
                                Icon {
                                    width: 16,
                                    height: 16,
                                    fill: "black",
                                    icon: FaFolderPlus,
                                }
                                "Add Folder"
                            }
                        }
                    }
                }
            }
        }
    }
}
#[component]
/// Renders the network side in the settings menu
fn NetworkMenu() -> Element {
    let mut start_on_boot = use_signal(|| false);
    let mut selected_interface = use_signal(|| "wg0".to_string());
    let mut config_path = use_signal(|| "/etc/wireguard/wg0.conf".to_string());
    rsx! {
        div { class: "w-5/6 mx-auto mt-4 bg-background shadow-md rounded-md overflow-hidden",
            div { class: "p-4 space-y-4",
                div { class: "flex items-center justify-between",
                    label { class: "font-medium text-foreground", "Active Interface:" }
                    select {
                        class: "border border-gray-300 hover:border-gray-500 rounded-md px-1 py-1 text-sm transition-all focus:outline-none",
                        value: "{selected_interface}",
                        onchange: move |evt| selected_interface.set(evt.value()),
                        option { value: "wg0", "wg0" }
                        option { value: "wg1", "wg1" }
                        option { value: "wg2", "wg2" }
                    }
                }
                div { class: "flex items-center justify-between",
                    label { class: "font-medium text-gray-700", "Start on boot:" }
                    label { class: "relative inline-flex items-center cursor-pointer",
                        input {
                            r#type: "checkbox",
                            class: "sr-only peer",
                            checked: start_on_boot(),
                            onchange: move |evt| start_on_boot.set(evt.checked()),
                        }
                        div {
                            class: format!(
                                "w-11 h-6 bg-gray-200 rounded-full peer transition-all {}",
                                if start_on_boot() { "peer-checked:bg-accent-2" } else { "" },
                            ),
                        }
                        div {
                            class: format!(
                                "absolute top-[2px] left-[2px] bg-white border border-gray-300 rounded-full h-5 w-5 transition-all {}",
                                if start_on_boot() { "translate-x-full border-white" } else { "" },
                            ),
                        }
                    }
                }
                div { class: "space-y-2",
                    div { class: "flex items-center gap-3",
                        label { class: "block font-medium text-gray-700 mr-5", "Config Path:" }
                        div { class: "flex-1 bg-background border border-gray-300 rounded-md px-3 py-2 text-sm text-gray-600 truncate",
                            "{config_path}"
                        }
                        button {
                            class: "btn-main focus:outline-none bg-accent hover:bg-accent-2 hover:outline-1 px-3 py-2 rounded-md text-sm font-medium transition-all inset-shadow-2xs",
                            onclick: move |_| {
                                println!("Opening file dialog...");
                            },
                            "..."
                        }
                    }
                }
            }
        }
    }
}
