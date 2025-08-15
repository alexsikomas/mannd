#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_close_button(true),
        ..Default::default()
    };
    eframe::run_native(
        "Networkd Wireguard Manager",
        native_options,
        Box::new(|cc| Ok(Box::new(networkd_wireguard_manager::App::new(cc)))),
    )
}
