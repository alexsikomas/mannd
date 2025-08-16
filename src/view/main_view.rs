use std::{
    fs::{self, DirEntry},
    io,
    path::PathBuf,
};

use egui::{CornerRadius, Frame, Margin, Ui};
use egui_file_dialog::FileDialog;
use serde::{Deserialize, Serialize};

use crate::{
    app::{ConfigMessage, Message, PathOptions},
    cli::config::Config,
};

#[derive(PartialEq, Serialize, Deserialize)]
enum ConfigSelected {
    Wireguard,
    Network,
}

#[derive(Serialize, Deserialize)]
pub struct MainView {
    config_open: bool,
    #[serde(skip)]
    file_dialog: FileDialog,
    config_selected: ConfigSelected,
    wireguard_options: WireguardOptions,
    network_options: NetworkOptions,
}

impl Default for MainView {
    fn default() -> Self {
        Self {
            config_open: false,
            file_dialog: FileDialog::new(),
            config_selected: ConfigSelected::Wireguard,
            wireguard_options: WireguardOptions::default(),
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

        let wireguard_options = WireguardOptions {
            folders: config.wireguard.folders.clone(),
        };

        let mut view = Self {
            wireguard_options: wireguard_options,
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
            self.config_open = !self.config_open;
        }

        egui::Window::new("Configuration")
            .open(&mut self.config_open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.config_selected,
                        ConfigSelected::Wireguard,
                        "WireGuard",
                    );
                    ui.selectable_value(
                        &mut self.config_selected,
                        ConfigSelected::Network,
                        "Network",
                    );
                });
                ui.separator();

                match self.config_selected {
                    ConfigSelected::Wireguard => {
                        self.wireguard_options.render(ui, &mut self.file_dialog);
                    }
                    ConfigSelected::Network => {
                        self.network_options.render(ui, &mut self.file_dialog);
                    }
                }
            });

        self.wireguard_options
            .wg_folder_selection(&mut self.file_dialog, ctx, messages);
    }
}

#[derive(Serialize, Deserialize)]
struct NetworkOptions {
    path: PathBuf,
    #[serde(skip)]
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
    /// Reads the network files in `network_folder` and returns a vector of
    /// directory entries
    ///
    /// Entries which return `io::Error` are ignored
    fn get_network_files(path: &PathBuf) -> io::Result<Vec<DirEntry>> {
        Ok(fs::read_dir(path)?
            .filter_map(Result::ok)
            .filter(|s| {
                s.path().is_file()
                    && s.path()
                        .extension()
                        .map(|s| s == "network")
                        .unwrap_or(false)
            })
            .collect())
    }

    pub fn render(&mut self, ui: &mut Ui, fd: &mut FileDialog) {
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
                                fd.pick_directory();
                            }
                        })
                    });

                ui.end_row();
                ui.label("Inferface: ");
                egui::ComboBox::from_label("")
                    .selected_text(&self.selected)
                    .show_ui(ui, |ui| {
                        for interface in &self.interfaces {
                            let file_name = interface
                                .file_name()
                                .into_string()
                                .unwrap_or("None".to_string());
                            ui.selectable_value(&mut self.selected, file_name.clone(), file_name);
                        }
                    });
            });
    }
}

#[derive(Serialize, Deserialize, Default)]
struct WireguardOptions {
    folders: Vec<PathBuf>,
}

impl WireguardOptions {
    pub fn render(&mut self, ui: &mut Ui, fd: &mut FileDialog) {
        let wg_button = ui.button("Add WireGuard Folder");
        if wg_button.clicked() {
            fd.pick_directory();
        };

        wg_button.on_hover_ui_at_pointer(|ui| {
            ui.label("Adds a WireGuard Folder to the list of tracked folders");
        });

        ui.add_space(10.0);
        egui::Grid::new("wireguard_grid")
            .num_columns(1)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for folder in &self.folders {
                    Frame::NONE
                        .fill(egui::Color32::from_rgb(50, 50, 50))
                        .inner_margin(Margin::same(4))
                        .corner_radius(CornerRadius::same(5))
                        .show(ui, |ui| {
                            // BUG: since min window size enforced this is problematic for long
                            // paths
                            ui.label(folder.as_path().to_str().unwrap());
                        });
                    ui.end_row();
                }
            });
    }

    fn wg_folder_selection(
        &mut self,
        fd: &mut FileDialog,
        ctx: &egui::Context,
        messages: &mut Vec<Message>,
    ) {
        fd.update(ctx);
        if let Some(path) = fd.take_picked() {
            messages.push(Message::Config(ConfigMessage::UpdateWgPath(
                path.clone(),
                PathOptions::Add,
            )));
            self.folders.push(path);
        }
    }
}
