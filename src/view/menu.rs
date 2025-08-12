use log::warn;
use std::{
    fs,
    os::unix::process::CommandExt,
    process::{Command, Output},
};

pub struct Menu {
    show_license: bool,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            show_license: false,
        }
    }
}

impl Menu {
    pub fn top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                if ui.button("License").clicked() {
                    self.show_license = !self.show_license;
                };
                if self.show_license {
                    self.license_viewport(ctx);
                }
            });
        });
    }

    fn license_viewport(&mut self, ctx: &egui::Context) {
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("License"),
            egui::ViewportBuilder::default()
                .with_title("License")
                .with_inner_size([200.0, 200.0]),
            |ctx, class| {
                assert!(
                    class == egui::ViewportClass::Immediate,
                    "This egui backend doesn't support multiple viewports"
                );

                if ctx.input(|i| i.viewport().close_requested()) {
                    self.show_license = false;
                }

                match fs::read_to_string("LICENSE") {
                    Ok(text) => {
                        egui::CentralPanel::default().show(ctx, |ui| ui.label(text));
                    }
                    Err(e) => {
                        warn!("Could not locate LICENSE file!")
                    }
                };
            },
        );
    }
}
