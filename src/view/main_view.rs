use std::path::PathBuf;

use egui::{Button, CornerRadius, Frame, Margin, Ui};
use egui_file_dialog::FileDialog;

#[derive(PartialEq)]
enum ConfigOptions {
    WireGuard,
    Network,
}

pub struct MainView {
    wg_config_open: bool,
    file_dialog: FileDialog,
    picked_folder: Option<PathBuf>,
    config_options: ConfigOptions,
    network_options: NetworkOptions,
}

impl Default for MainView {
    fn default() -> Self {
        Self {
            wg_config_open: false,
            file_dialog: FileDialog::new(),
            picked_folder: None,
            config_options: ConfigOptions::WireGuard,
            network_options: NetworkOptions::default(),
        }
    }
}

impl MainView {
    pub fn central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.wg_path_button(ctx, ui);
        });
    }

    pub fn wg_path_button(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        if ui.button("Configuration").clicked() {
            self.wg_config_open = !self.wg_config_open;
        }

        egui::Window::new("Configuration")
            .open(&mut self.wg_config_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.config_options,
                        ConfigOptions::WireGuard,
                        "WireGuard",
                    );
                    ui.selectable_value(
                        &mut self.config_options,
                        ConfigOptions::Network,
                        "Network",
                    );
                });
                ui.separator();

                match self.config_options {
                    ConfigOptions::WireGuard => {
                        Self::config_wg_view(ui, &mut self.file_dialog);
                    }
                    ConfigOptions::Network => {
                        Self::config_network_view(ui, &mut self.network_options);
                    }
                }
            });

        self.wg_folder_selection(ctx);
    }

    fn wg_folder_selection(&mut self, ctx: &egui::Context) {
        self.file_dialog.update(ctx);
        if let Some(path) = self.file_dialog.take_picked() {
            self.picked_folder = Some(path.to_path_buf());
        }
    }

    fn config_wg_view(ui: &mut Ui, fd: &mut FileDialog) {
        let wg_button = ui.button("Add WireGuard Folder");
        if wg_button.clicked() {
            fd.pick_directory();
        };

        wg_button.on_hover_ui_at_pointer(|ui| {
            ui.label("Adds a WireGuard Folder to the list of tracked folders");
        });
    }

    fn config_network_view(ui: &mut Ui, options: &mut NetworkOptions) {
        egui::Grid::new("network_grid")
            .num_columns(2)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                ui.label("Systemd Network Folder: ");
                Frame::NONE
                    .fill(egui::Color32::from_rgb(50, 50, 50))
                    .inner_margin(Margin::same(4))
                    .corner_radius(CornerRadius::same(5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("/etc/systemd/network/");
                            if ui
                                .add(
                                    egui::Button::new("...")
                                        .fill(egui::Color32::from_rgb(90, 90, 90)),
                                )
                                .clicked()
                            {
                                println!("CLICKED");
                            }
                        })
                    });

                ui.end_row();
                ui.label("Inferface: ");
                egui::ComboBox::from_label("")
                    .selected_text(&options.selected)
                    .show_ui(ui, |ui| {
                        // TODO: read and write from toml
                        ui.selectable_value(&mut options.selected, "wlan0".to_string(), "wlan0");
                    });
            });
    }
}

#[derive(Default)]
struct NetworkOptions {
    path: PathBuf,
    selected: String,
    start_on_boot: bool,
}

struct WireguardOptions {}
