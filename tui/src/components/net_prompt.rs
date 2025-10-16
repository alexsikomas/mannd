use com::wireless::common::AccessPoint;
use ratatui::{
    layout::Position,
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Widget},
};
use tracing::info;

use crate::{
    state::ConnectionPromptSelect,
    ui::{THEME, Theme},
};

pub struct NetworkPrompt<'a> {
    network: &'a AccessPoint,
    selected: &'a ConnectionPromptSelect,
}

impl<'a> NetworkPrompt<'a> {
    pub fn new(ap: &'a AccessPoint, selected: &'a ConnectionPromptSelect) -> Self {
        Self {
            network: ap,
            selected,
        }
    }
}

impl<'a> Widget for NetworkPrompt<'a> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => return,
        };

        // clear characters beneath
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                match buf.cell_mut(Position::new(x, y)) {
                    Some(cell) => {
                        cell.reset();
                    }
                    None => {}
                }
            }
        }

        let main_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title_top(
                Line::from(" Connect ")
                    .centered()
                    .style(Style::new().fg(theme.accent.color()).bold()),
            )
            .style(
                Style::new()
                    .fg(theme.info.color())
                    .bg(theme.background.color()),
            );

        main_block.render(area, buf);
    }
}
