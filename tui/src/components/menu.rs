use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Widget},
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

        info!("Color before shift: {:?}", theme.primary);
        info!("Color after shift: {}", theme.primary.shift(10));
        let select = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(Style::new().fg(theme.primary.shift(100)))
            .title_top("Select");

        select.render(area, buf);
    }
}
