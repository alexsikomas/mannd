use std::path::PathBuf;

use crate::{
    cli::config::Config,
    view::{main_view::MainView, menu::Menu},
};

pub struct App {
    menu: Menu,
    main_view: MainView,
    config: Config,
}

impl Default for App {
    fn default() -> Self {
        Self {
            menu: Menu::default(),
            main_view: MainView::default(),
            config: Config::new().unwrap(),
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
        self.main_view.central_panel(ctx);
    }
}

enum Message {
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
    fn handle_messages(&mut self, message: Message) {
        match message {
            Message::Config(conf) => self.config.handle_message(conf),
            _ => {}
        };
    }
}
