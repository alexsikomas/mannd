use std::path::PathBuf;

use egui::{
    epaint::text::{FontInsert, InsertFontFamily},
    Color32,
};
use serde::{Deserialize, Serialize};

use crate::{
    cli::config::Config,
    gui::{main_panel::Panel, menu::Menu, sidebar::Sidebar},
};

#[derive(Serialize, Deserialize)]
pub struct App {
    menu: Menu,
    sidebar: Sidebar,
    central_panel: Panel,
    config: Config,
    #[serde(skip)]
    style: AppStyle,
}

impl Default for App {
    fn default() -> Self {
        // TODO: Add prompt for network location on fail
        let config = match Config::new() {
            Ok(c) => c,
            Err(e) => {
                panic!("Failure during initialisation of config file, check log!")
            }
        };
        Self {
            menu: Menu::default(),
            sidebar: Sidebar::new(&config),
            central_panel: Panel::default(),
            config,
            style: AppStyle::default(),
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.add_font(FontInsert::new(
            "Poppins",
            egui::FontData::from_static(include_bytes!(
                "../assets/poppins-font/Poppins-Regular.ttf"
            )),
            vec![InsertFontFamily {
                family: egui::FontFamily::Proportional,
                priority: egui::epaint::text::FontPriority::Highest,
            }],
        ));
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Default::default()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.menu.render(ctx);
        let mut messages = Vec::new();
        messages.extend(self.sidebar.render(ctx));
        messages.extend(self.central_panel.render(ctx));

        for message in messages {
            self.handle_message(message);
        }
    }
}

/// This is the communication format that functions
/// return for processing in update loop
pub enum Message {
    Config(ConfigMessage),
}

/// Operations that may be performed on a path,
/// used by `UpdateWgPath`
pub enum PathOptions {
    Add,
    Remove,
    RemoveAll,
}

/// Handled by `config`
pub enum ConfigMessage {
    UpdateWgPath(PathBuf, PathOptions),
    UpdateNetworkPath(PathBuf),
    UpdateInterface(String),
    UpdateBoot(bool),
}

impl App {
    /// Sends `Message` to relavent `handle_message` function
    fn handle_message(&mut self, message: Message) {
        match message {
            Message::Config(conf) => self.config.handle_message(conf),
        };
    }
}

pub struct AppColours {
    connected: Color32,
    disconnected: Color32,
    text: Color32,
    frame: Color32,
}

impl Default for AppColours {
    fn default() -> Self {
        Self {
            connected: Color32::from_rgb(0, 255, 0),
            disconnected: Color32::from_rgb(255, 0, 0),
            text: Color32::from_rgb(234, 219, 180),
            frame: Color32::from_rgb(28, 32, 33),
        }
    }
}

pub struct AppStyle {
    colours: AppColours,
}

impl Default for AppStyle {
    fn default() -> Self {
        Self {
            colours: AppColours::default(),
        }
    }
}
