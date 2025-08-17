use egui::{AtomExt, Color32, Vec2};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct Menu {
    show_license: bool,
    connected_colour: Color32,
    disconnected_colour: Color32,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            show_license: false,
            connected_colour: Color32::from_rgb(0, 255, 0),
            disconnected_colour: Color32::from_rgb(255, 0, 0),
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
                        // TODO: Include at compile time for ease of distribution
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Image::new(egui::include_image!("../../assets/gui/circle.svg"))
                            .tint(self.connected_colour),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("Connected").color(self.connected_colour),
                        )
                        .selectable(false),
                    );
                });
            });
        });
    }
}
