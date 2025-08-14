use log::warn;
use std::fs;

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

                ui.menu_button("Help", |ui| {
                    if ui.button("License").clicked() {
                        self.show_license = !self.show_license;
                    };
                });

                if self.show_license {
                    let window = egui::Window::new("License");
                    window.open(&mut self.show_license).show(ctx, |ui| {
                        match fs::read_to_string("LICENSE") {
                            Ok(text) => {
                                ui.label(text);
                            }
                            Err(e) => {
                                warn!("Could not load LICENSE file!, Error: {e}");
                            }
                        }
                    });
                }
            });
        });
    }
}
