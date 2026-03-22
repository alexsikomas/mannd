use std::path::PathBuf;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, Widget},
};

use crate::{components::layout::panel_with_toolbar, state::networkd::NetworkdState, ui::theme};

pub struct NetworkdMenu<'a> {
    state: &'a NetworkdState,
    configs: &'a [PathBuf],
}

impl<'a> NetworkdMenu<'a> {
    pub fn new(state: &'a NetworkdState, configs: &'a [PathBuf]) -> Option<Self> {
        Some(Self { state, configs })
    }
}

impl Widget for NetworkdMenu<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let areas = Self::build_layout_no_render(area);
        self.render_main_block(areas.outer, buf);
    }
}

impl NetworkdMenu<'_> {
    fn render_main_block(&self, area: Rect, buf: &mut Buffer) {
        let theme = theme();
        let main = Block::default()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(theme.primary.color())
            .title_top(
                Line::from(" Networkd ")
                    .centered()
                    .style(theme.accent.color())
                    .bold(),
            );
        main.render(area, buf);
    }

    fn build_layout_no_render(area: Rect) -> NetdAreas {
        let (outer, select_area, config_area) = panel_with_toolbar(area, 80, 90);
        NetdAreas {
            outer,
            config: config_area,
            select: select_area,
        }
    }
}

pub struct NetdAreas {
    outer: Rect,
    config: Rect,
    select: Rect,
}
