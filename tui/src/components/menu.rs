use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Widget},
};

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
        let select = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(Style::new().fg(ratatui::style::Color::Red))
            .title_top("Select");

        select.render(area, buf);
    }
}
