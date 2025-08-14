use crate::view::{main_view::MainView, menu::Menu};

pub struct App {
    menu: Menu,
    main_view: MainView,
}

impl Default for App {
    fn default() -> Self {
        Self {
            menu: Menu::default(),
            main_view: MainView::default(),
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
