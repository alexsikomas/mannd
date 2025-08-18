use std::{
    fs::{self, DirEntry},
    io,
    path::{Path, PathBuf},
};

use egui::{Color32, CornerRadius, Frame, Margin, Ui, Widget};
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
pub struct Sidebar {
    config_open: bool,
    #[serde(skip)]
    file_dialog: FileDialog,
    config_selected: ConfigSelected,
    wireguard_options: WireguardOptions,
    network_options: NetworkOptions,
}

impl Default for Sidebar {
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

impl Sidebar {
    /// Creates a new sidebar instance
    ///
    /// # Panics
    /// If the network path cannot be found this will panic
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

    /// Render loop to be sent back to the controller in the update loop,
    /// returns a vector of messages to be processed in the update loop.
    pub fn render(&mut self, ctx: &egui::Context) -> Vec<Message> {
        let mut messages = vec![];
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            self.render_config_buttons(ui);
        });
        self.render_config_ui(ctx, &mut messages);
        messages
    }

    /// Renders the buttons in the sidebar, if images cannot be loaded
    /// they are replaced by a red triangle
    pub fn render_config_buttons(&mut self, ui: &mut Ui) {
        ui.add_space(10.);
        egui::Grid::new("sidebar")
            .num_columns(1)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                if ui
                    .button((
                        egui::Image::new(egui::include_image!("../../assets/gui/gears-solid.svg")),
                        "Configuration",
                    ))
                    .clicked()
                {
                    self.config_open = !self.config_open;
                }
                ui.end_row();
                if ui
                    .button((
                        egui::Image::new(egui::include_image!("../../assets/gui/link.svg")),
                        egui::RichText::new("Connect").color(Color32::from_rgb(0, 255, 0)),
                    ))
                    .clicked()
                {}
            });
    }

    /// Renders the config window when `config_open` is true
    fn render_config_ui(&mut self, ctx: &egui::Context, messages: &mut Vec<Message>) {
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
                        self.file_dialog.update(ctx);
                        if let Some(path) = self.file_dialog.take_picked() {
                            messages.push(Message::Config(ConfigMessage::UpdateWgPath(
                                path.clone(),
                                PathOptions::Add,
                            )));
                            self.wireguard_options.folders.push(path);
                        }
                    }
                    ConfigSelected::Network => {
                        self.network_options
                            .render(ui, &mut self.file_dialog, messages);
                        self.file_dialog.update(ctx);
                        if let Some(path) = self.file_dialog.take_picked() {
                            messages.push(Message::Config(ConfigMessage::UpdateNetworkPath(
                                path.clone(),
                            )));
                            // TODO: update possible interfaces
                            self.network_options.path = path;
                        }
                    }
                }
            });
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
    fn get_network_files(path: impl AsRef<Path>) -> io::Result<Vec<DirEntry>> {
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

    /// Renders the network side of the config window
    pub fn render(&mut self, ui: &mut Ui, fd: &mut FileDialog, messages: &mut Vec<Message>) {
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
                        ui.horizontal_centered(|ui| {
                            ui.label(self.path.to_str().unwrap());
                            if ui
                                .button(
                                    egui::RichText::new("...")
                                        .color(Color32::from_rgb(150, 150, 150)),
                                )
                                .clicked()
                            {
                                fd.pick_directory();
                            }
                        });
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
                            if ui
                                .selectable_value(&mut self.selected, file_name.clone(), file_name)
                                .clicked()
                            {
                                messages.push(Message::Config(ConfigMessage::UpdateInterface(
                                    self.selected.clone(),
                                )));
                            }
                        }
                    });

                ui.end_row();
                ui.label("Start on boot:");
                if egui::Checkbox::without_text(&mut self.start_on_boot)
                    .ui(ui)
                    .changed()
                {
                    messages.push(Message::Config(ConfigMessage::UpdateBoot(
                        self.start_on_boot.clone(),
                    )));
                }
            });
    }
}

#[derive(Serialize, Deserialize, Default)]
struct WireguardOptions {
    folders: Vec<PathBuf>,
}

impl WireguardOptions {
    /// Renders the wireguard side of the config menu
    ///
    /// # Panics
    /// If any wireguard folder path causes an error
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
}
