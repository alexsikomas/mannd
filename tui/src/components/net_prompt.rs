use com::wireless::common::AccessPoint;
use ratatui::{
    style::Style,
    widgets::{Block, Borders, Widget},
};

use crate::ui::{THEME, Theme};

struct NetworkPrompt {
    network: AccessPoint,
}

impl Widget for NetworkPrompt {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => return,
        };

        // let main_block = Block::new().borders(Borders::ALL).title_top("Connect").style(Style::new().fg(color))
    }
}
