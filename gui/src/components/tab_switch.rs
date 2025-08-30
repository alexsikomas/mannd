use dioxus::prelude::*;

#[component]
pub fn TabSwitch() -> Element {
    let opts = use_signal(|| vec!["Connection", "VPN"]);
    let mut current = use_signal(|| opts.read()[0]);

    rsx! {
        div { class: "flex bg-gradient-to-r from-[#fbf8f1] to-[#f8f4e6] rounded-lg p-1 shadow-sm border border-[#e8dcc0] max-w-xs mx-auto",

            for (index , & item) in opts.read().iter().enumerate() {
                button {
                    key: "{item}",
                    onclick: move |_| {
                        current.set(item);
                    },
                    class: format!(
                        "flex-1 px-4 py-2 text-sm font-medium transition-all duration-200 ease-in-out {} {}",
                        if index == 0 { "rounded-l-md" } else { "rounded-r-md" },
                        if *current.read() == item {
                            "bg-accent outline-1 text-gray-800 shadow-md transform scale-[0.98]"
                        } else {
                            "text-gray-600 hover:text-gray-800 hover:bg-accent/10 active:scale-[0.96]"
                        },
                    ),
                    "{item}"
                }
            }
        }
    }
}
