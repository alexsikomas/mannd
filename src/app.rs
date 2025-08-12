use crate::view::menu::Menu;

pub struct App {
    menu: Menu,
}

impl Default for App {
    fn default() -> Self {
        Self {
            menu: Menu::default(),
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
    }
}
