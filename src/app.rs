use std::{cell::RefCell, path::PathBuf};

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
}

impl Default for App {
    fn default() -> Self {
        let config = Config::new().unwrap();
        Self {
            menu: Menu::default(),
            sidebar: Sidebar::new(&config),
            central_panel: Panel::default(),
            config,
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Default::default()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.menu.top_panel(ctx);
        let mut messages = Vec::new();
        messages.extend(self.sidebar.render(ctx));
        messages.extend(self.central_panel.render(ctx));

        for message in messages {
            self.handle_message(message);
        }
    }
}

pub enum Message {
    Config(ConfigMessage),
}

pub enum PathOptions {
    Add,
    Remove,
    RemoveAll,
}

pub enum ConfigMessage {
    UpdateWgPath(PathBuf, PathOptions),
    UpdateNetworkPath(PathBuf),
    UpdateInterface(String),
    UpdateBoot(bool),
}

impl App {
    fn handle_message(&mut self, message: Message) {
        match message {
            Message::Config(conf) => self.config.handle_message(conf),
        };
    }
}
