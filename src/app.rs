use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    cli::config::Config,
    gui::{main_panel::Panel, menu::Menu},
};

#[derive(Serialize, Deserialize)]
pub struct App {
    menu: Menu,
    central_panel: Panel,
    config: Config,
    #[serde(skip)]
    messages: Vec<Message>,
}

impl Default for App {
    fn default() -> Self {
        let config = Config::new().unwrap();
        Self {
            menu: Menu::default(),
            central_panel: Panel::new(&config),
            config,
            messages: vec![],
        }
    }
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.menu.top_panel(ctx);
        let messages = self.central_panel.render(ctx);
        for message in messages {
            Self::handle_message(self, message);
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
