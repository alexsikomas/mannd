use dioxus::prelude::*;
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaChevronDown, FaFolderPlus, FaGear, FaXmark},
    Icon,
};
#[component]
pub fn MenuHeader() -> Element {
    let mut settings_open = use_signal(|| false);
    rsx! {
        header { class: "sticky top-0 z-50 w-full border-b border-gray-200/50 bg-gradient-to-r from-[#fffbf4] to-[#fbf8f1] backdrop-blur-sm shadow-sm",
            div { class: "container mx-auto flex items-center justify-between px-6 py-5",
                div { class: "flex items-center gap-3",
                    div {
                        h1 { class: "text-3xl font-bold bg-gradient-to-r from-gray-800 to-gray-600 bg-clip-text text-transparent",
                            "WireGuard Manager"
                        }
                        p { class: "text-sm text-gray-500 font-medium tracking-wide",
                            "For Networkd"
                        }
                    }
                }
                button {
                    onclick: move |_| {
                        settings_open.toggle();
                    },
                    class: "p-3 bg-[#f9c647] hover:bg-[#f7b731] rounded-xl shadow-lg hover:shadow-xl transition-all duration-200 transform hover:scale-105 active:scale-95",
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
    let opts = use_signal(|| vec!["WireGuard", "Network"]);
    let mut current = use_signal(|| "WireGuard");
    if open() {
        rsx! {
            div { class: "fixed inset-0 z-50 transition-all ease-in-out duration-300 bg-black/60 backdrop-blur-sm",
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
                    class: "absolute top-4 right-4 p-2 bg-[#FF5F15] hover:bg-[] rounded-xl shadow-md hover:shadow-lg transition-all duration-200 transform hover:scale-105",
                    Icon {
                        width: 20,
                        height: 20,
                        fill: "black",
                        icon: FaXmark,
                    }
                }
                div { class: "flex gap-3 p-6 pb-4 border-b border-gray-200/50",
                    for & item in opts.read().iter() {
                        if *current.read() == item {
                            button { class: "px-6 py-3 bg-[#f9c647] text-gray-800 font-semibold rounded-xl shadow-lg transform scale-105 transition-all duration-200",
                                "{item}"
                            }
                        } else {
                            button {
                                onclick: move |_| { current.set(item) },
                                class: "px-6 py-3 bg-white/80 hover:bg-[#f9c647]/20 text-gray-600 hover:text-gray-800 font-medium rounded-xl shadow-md hover:shadow-lg transition-all duration-200 transform hover:scale-105",
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
fn WireguardMenu() -> Element {
    let mut is_open = use_signal(|| false);
    rsx! {
        div {
            class: format!(
                "transition-all ease-out duration-300 w-full mx-auto bg-white/80 shadow-lg hover:shadow-xl rounded-2xl overflow-hidden border border-gray-200/50 {}",
                if *is_open.read() {
                    "ring-2 ring-[#f9c647]/50 shadow-[#f9c647]/20"
                } else {
                    "hover:border-[#f9c647]/30"
                },
            ),
            div { class: "rounded-2xl",
                h2 { class: "mb-0",
                    button {
                        class: format!(
                            "flex items-center justify-between w-full p-6 font-semibold text-left transition-all duration-300 rounded-2xl {}",
                            if *is_open.read() {
                                "bg-gradient-to-r from-[#f9c647] to-[#f7b731] text-gray-800 shadow-lg"
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
                                "transform transition-transform duration-300 text-xl {}",
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
                        "transition-all duration-300 overflow-hidden {}",
                        (if *is_open.read() {
                            "ease-out max-h-96 opacity-100"
                        } else {
                            "ease-in max-h-0 opacity-0"
                        }),
                    ),
                    div { class: "p-6 bg-gradient-to-b from-white/50 to-[#fbf8f1]/30",
                        button { class: "flex items-center gap-3 px-6 py-4 bg-gradient-to-r from-[#f9c647] to-[#f7b731] hover:from-[#f7b731] to-[#f5a623] text-gray-800 font-semibold rounded-xl shadow-lg hover:shadow-xl transition-all duration-200 transform hover:scale-105 active:scale-95",
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
#[component]
fn NetworkMenu() -> Element {
    let mut start_on_boot = use_signal(|| false);
    let mut selected_interface = use_signal(|| "wg0".to_string());
    let mut config_path = use_signal(|| "/etc/wireguard/wg0.conf".to_string());
    rsx! {
        div { class: "w-5/6 mx-auto mt-4 bg-[#fbf8f1] shadow-md rounded-md overflow-hidden",
            div { class: "p-4 space-y-4",
                div { class: "flex items-center justify-between",
                    label { class: "font-medium text-gray-700", "Active Interface:" }
                    select {
                        class: "bg-[#fbf8f1] border border-gray-300 rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-[#f9c647] focus:border-[#f9c647] transition-all",
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
                                "w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-[#f9c647]/25 rounded-full peer transition-all {}",
                                if start_on_boot() { "peer-checked:bg-[#f9c647]" } else { "" },
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
                    label { class: "block font-medium text-gray-700", "Config Path:" }
                    div { class: "flex items-center gap-2",
                        div { class: "flex-1 bg-[#fbf8f1] border border-gray-300 rounded-md px-3 py-2 text-sm text-gray-600 truncate",
                            "{config_path}"
                        }
                        button {
                            class: "btn-main bg-[#f9c647] hover:bg-[#f7b731] px-3 py-2 rounded-md text-sm font-medium transition-all inset-shadow-2xs",
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
