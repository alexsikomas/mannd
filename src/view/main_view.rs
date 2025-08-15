use std::path::PathBuf;

use egui::Ui;
use egui_file_dialog::FileDialog;

#[derive(PartialEq)]
enum ConfigOptions {
    WireGuard,
    Other,
}

pub struct MainView {
    wg_config_open: bool,
    file_dialog: FileDialog,
    picked_folder: Option<PathBuf>,
    config_options: ConfigOptions,
}

impl Default for MainView {
    fn default() -> Self {
        Self {
            wg_config_open: false,
            file_dialog: FileDialog::new(),
            picked_folder: None,
            config_options: ConfigOptions::WireGuard,
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
                    ui.selectable_value(&mut self.config_options, ConfigOptions::Other, "Other");
                });
                ui.separator();

                match self.config_options {
                    ConfigOptions::WireGuard => {
                        Self::config_wg_view(ui, &mut self.file_dialog);
                    }
                    ConfigOptions::Other => {
                        Self::config_other_view(ui);
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
        if ui.button("Add Folder").clicked() {
            fd.pick_directory();
        };
    }

    fn config_other_view(ui: &mut Ui) {}
}
