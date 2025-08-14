use std::path::PathBuf;

use egui::Ui;
use egui_file_dialog::FileDialog;

pub struct MainView {
    wg_config_open: bool,
    file_dialog: FileDialog,
    picked_folder: Option<PathBuf>,
}

impl Default for MainView {
    fn default() -> Self {
        Self {
            wg_config_open: false,
            file_dialog: FileDialog::new(),
            picked_folder: None,
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
        if ui.button("WireGuard Config").clicked() {
            self.wg_config_open = !self.wg_config_open;
        }

        if self.wg_config_open {
            egui::Window::new("WireGuard Configuration")
                .open(&mut self.wg_config_open)
                .show(ctx, |ui| {
                    if ui.button("Add Folder").clicked() {
                        self.file_dialog.pick_directory();
                    };
                });
        }

        ui.label(format!("Picked folder: {:?}", self.picked_folder));
        self.file_dialog.update(ctx);
        if let Some(path) = self.file_dialog.take_picked() {
            self.picked_folder = Some(path.to_path_buf());
        }
    }
}
