use std::path::PathBuf;

use egui::Ui;
use egui_file_dialog::FileDialog;

pub struct MainView {
    file_dialog: FileDialog,
    picked_folder: Option<PathBuf>,
}

impl Default for MainView {
    fn default() -> Self {
        Self {
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
        ui.horizontal(|ui| {
            let wg_label = ui.label("Choose WireGuard folder: ");
            if ui.button("Click here!").labelled_by(wg_label.id).clicked() {
                self.file_dialog.pick_directory();
            }
        });
        ui.label(format!("Picked folder: {:?}", self.picked_folder));
        self.file_dialog.update(ctx);
        if let Some(path) = self.file_dialog.take_picked() {
            self.picked_folder = Some(path.to_path_buf());
        }
    }
}
