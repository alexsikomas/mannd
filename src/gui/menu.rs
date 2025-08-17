use egui::{text::LayoutJob, AtomExt, Color32, Layout, RichText, Ui, Vec2};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct Menu {
    show_license: bool,
    show_credits: bool,
    connected_colour: Color32,
    disconnected_colour: Color32,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            show_license: false,
            show_credits: false,
            connected_colour: Color32::from_rgb(0, 255, 0),
            disconnected_colour: Color32::from_rgb(255, 0, 0),
        }
    }
}

impl Menu {
    pub fn render(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("Credits").clicked() {
                        self.show_credits = !self.show_credits;
                    }
                    if ui.button("License").clicked() {
                        self.show_license = !self.show_license;
                    };
                });

                self.credits_window(ctx, ui);
                self.license_window(ctx, ui);
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

    fn credits_window(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        // TODO: This looks messy, clean up structure or seperate into functions
        egui::Window::new("Credits")
            .open(&mut self.show_credits)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(
                    "Acknowledgements to various resources which made this application possible.",
                ));
                ui.add_space(10.);
                egui::Grid::new("credits_grid")
                    .num_columns(2)
                    .spacing([10.0, 1.0])
                    .show(ui, |ui| {
                        ui.label("GUI Library:");
                        ui.hyperlink_to("egui", "https://github.com/emilk/egui/");
                        ui.end_row();
                        ui.label("Font:");
                        ui.hyperlink_to(
                            "Poppins (Regular)",
                            "https://github.com/itfoundry/Poppins",
                        );
                        ui.end_row();
                        let mut job = LayoutJob::default();

                        
                        RichText::new("Icons: ").append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
                        RichText::new("[1]").small().raised().append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
                        ui.label(job);

                        ui.hyperlink_to("Font Awesome", "https://fontawesome.com/");
                    });

                ui.add_space(20.);
                ui.label(egui::RichText::new(
                    "There are also numerous other Rust crates which I'm thankful for, please check the Cargo.toml for more details.",
                ));

                let mut job = LayoutJob::default();
                RichText::new("[1] ").small().raised().append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
                RichText::new(
                    "This application uses icons from Font Awesome. Some icons have been modified. The original licenses apply to all modified resources.",
                ).italics().append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
                ui.label(job);
            });
    }

    fn license_window(&mut self, ctx: &egui::Context, ui: &mut Ui) {
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
}
