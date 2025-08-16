use std::path::PathBuf;

use crate::{
    cli::config::Config,
    view::{main_view::MainView, menu::Menu},
};

pub struct App {
    menu: Menu,
    main_view: MainView,
    config: Config,
    messages: Vec<Message>,
}

impl Default for App {
    fn default() -> Self {
        let config = Config::new().unwrap();
        Self {
            menu: Menu::default(),
            main_view: MainView::new(&config),
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
        let messages = self.main_view.render(ctx);
        for message in messages {
            Self::handle_messages(self, message);
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
    fn handle_messages(&mut self, message: Message) {
        match message {
            Message::Config(conf) => self.config.handle_message(conf),
            _ => {}
        };
    }
}
