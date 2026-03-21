use std::path::PathBuf;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Widget},
};

use crate::{
    state::NetdState,
    ui::{THEME, Theme},
};

pub struct NetdMenu<'a> {
    state: &'a NetdState,
    configs: &'a [PathBuf],
    theme: &'a Theme,
}

impl<'a> NetdMenu<'a> {
    pub fn new(state: &'a NetdState, configs: &'a [PathBuf]) -> Option<Self> {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self {
            state,
            configs,
            theme,
        })
    }
}

impl Widget for NetdMenu<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let areas = Self::build_layout_no_render(area);
        self.render_main_block(areas.outer, buf);
    }
}

impl NetdMenu<'_> {
    fn render_main_block(&self, area: Rect, buf: &mut Buffer) {
        let main = Block::default()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(self.theme.primary.color())
            .title_top(
                Line::from(" Networkd ")
                    .centered()
                    .style(self.theme.accent.color())
                    .bold(),
            );
        main.render(area, buf);
    }

    fn build_layout_no_render(area: Rect) -> NetdAreas {
        let outer_area = Layout::horizontal([Constraint::Percentage(80)])
            .flex(Flex::Center)
            .split(
                Layout::vertical([Constraint::Percentage(90)])
                    .flex(Flex::Center)
                    .areas::<1>(area)[0],
            )[0];

        let [select_area, config_area_vert] =
            Layout::vertical([Constraint::Max(3), Constraint::Fill(1)]).areas::<2>(outer_area);

        let config_area = Layout::horizontal([Constraint::Percentage(90)])
            .flex(Flex::Center)
            .split(config_area_vert)[0];

        NetdAreas {
            outer: outer_area,
            config: config_area,
            select: select_area,
        }
    }

    fn get_style(&self, is_selected: bool) -> (Style, Style) {
        if is_selected {
            let border = Style::new().fg(self.theme.accent.color());
            let text = Style::new().fg(self.theme.info.color());
            (border, text)
        } else {
            let border = Style::new().fg(self.theme.muted.color());
            let text = Style::new().fg(self.theme.muted.color());
            (border, text)
        }
    }
}

pub struct NetdAreas {
    outer: Rect,
    config: Rect,
    select: Rect,
}
