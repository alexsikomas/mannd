use color_eyre::owo_colors::OwoColorize;
use ratatui::{
    buffer::Buffer,
    layout::{self, Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Padding, Paragraph, Widget},
};
use tracing::info;

use crate::ui::{THEME, Theme};

pub struct Menu {
    current: MenuType,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            current: MenuType::Connection,
        }
    }
}

enum MenuType {
    Connection,
    Vpn,
    Config,
}

impl Into<String> for MenuType {
    fn into(self) -> String {
        match self {
            Self::Connection => "Connection".to_string(),
            Self::Vpn => "VPN".to_string(),
            Self::Config => "Config".to_string(),
        }
    }
}

impl<'a> Into<Line<'a>> for MenuType {
    fn into(self) -> Line<'a> {
        let s: String = self.into();
        Line::from(s)
    }
}

impl Widget for Menu {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let theme: &Theme;
        match THEME.get() {
            Some(t) => {
                theme = t;
            }
            None => {
                return;
            }
        }

        let select = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::all())
            .style(Style::new().fg(theme.secondary.shift(-50)))
            .padding(Padding::new(0, 0, 1, 0))
            .title_top(
                Line::from(" Select ")
                    .centered()
                    .style(Style::new().fg(theme.secondary.color())),
            );

        let paragraph = Paragraph::new("Connection")
            .centered()
            .style(Style::new().fg(theme.secondary.color()));

        // TODO: add dynamic constraints based on res
        let layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Percentage(30)])
            .flex(layout::Flex::Center)
            .split(area);

        let layout = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(25)])
            .flex(layout::Flex::Center)
            .split(layout[0]);

        info!("{:?}", layout);
        paragraph.block(select).render(layout[0], buf);
    }
}
