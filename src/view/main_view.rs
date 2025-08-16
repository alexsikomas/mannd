use std::{
    fs::{self, DirEntry},
    io,
    path::PathBuf,
};

use egui::{Button, CornerRadius, Frame, Margin, Ui};
use egui_file_dialog::FileDialog;

use crate::{
    app::{ConfigMessage, Message, PathOptions},
    cli::config::Config,
};

#[derive(PartialEq)]
enum ConfigOptions {
    WireGuard,
    Network,
}

pub struct MainView {
    wg_config_open: bool,
    file_dialog: FileDialog,
    config_options: ConfigOptions,
    network_options: NetworkOptions,
}

impl Default for MainView {
    fn default() -> Self {
        Self {
            wg_config_open: false,
            file_dialog: FileDialog::new(),
            config_options: ConfigOptions::WireGuard,
            network_options: NetworkOptions::default(),
        }
    }
}

impl MainView {
    pub fn new(config: &Config) -> Self {
        let network_options = NetworkOptions {
            path: config.network.path.clone(),
            interfaces: vec![],
            selected: config
                .network
                .active_interface
                .clone()
                .unwrap_or("".to_string()),
            start_on_boot: config.network.start_on_boot,
        };

        let mut view = Self {
            network_options: network_options,
            ..Default::default()
        };

        view.network_options.interfaces =
            NetworkOptions::get_network_files(&view.network_options.path).unwrap();
        view
    }

    pub fn render(&mut self, ctx: &egui::Context) -> Vec<Message> {
        let mut messages = vec![];
        egui::CentralPanel::default().show(ctx, |ui| {
            self.config_button(ctx, ui, &mut messages);
        });
        messages
    }

    pub fn config_button(&mut self, ctx: &egui::Context, ui: &mut Ui, messages: &mut Vec<Message>) {
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
                        Self::config_network_view(ui, &mut self.network_options, messages);
                    }
                }
            });

        self.wg_folder_selection(ctx, messages);
    }

    fn wg_folder_selection(&mut self, ctx: &egui::Context, messages: &mut Vec<Message>) {
        self.file_dialog.update(ctx);
        if let Some(path) = self.file_dialog.take_picked() {
            messages.push(Message::Config(ConfigMessage::UpdateWgPath(
                path,
                PathOptions::Add,
            )));
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

    fn config_network_view(ui: &mut Ui, options: &mut NetworkOptions, messages: &mut Vec<Message>) {
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
                            {}
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

struct NetworkOptions {
    path: PathBuf,
    interfaces: Vec<DirEntry>,
    selected: String,
    start_on_boot: bool,
}

impl Default for NetworkOptions {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            interfaces: vec![],
            selected: "".to_string(),
            start_on_boot: false,
        }
    }
}

impl NetworkOptions {
    fn new() {}

    /// Reads the network files in `network_folder` and returns a vector of
    /// directory entries
    ///
    /// Entries which return `io::Error` are ignored
    fn get_network_files(path: &PathBuf) -> io::Result<Vec<DirEntry>> {
        Ok(fs::read_dir(path)?.filter_map(Result::ok).collect())
    }
}

struct WireguardOptions {}
