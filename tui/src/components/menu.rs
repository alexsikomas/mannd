use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{
    app::{SelectableList, Selection},
    ui::{THEME, Theme},
};

pub struct MainMenu<'a> {
    list: &'a SelectableList<Selection>,
}

impl<'a> MainMenu<'a> {
    pub fn new(list: &'a SelectableList<Selection>) -> Self {
        Self { list }
    }
}

impl<'a> Widget for MainMenu<'a> {
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

        let outer_area = Layout::vertical([Constraint::Percentage(30)])
            .flex(Flex::Center)
            .split(area)[0];

        let main_area = Layout::horizontal([Constraint::Percentage(25)])
            .flex(Flex::Center)
            .split(outer_area)[0];

        let select = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::all())
            .style(Style::new().fg(theme.secondary.shift(30)))
            .title_top(
                Line::from(" Select ")
                    .centered()
                    .style(Style::new().fg(theme.secondary.shift(10))),
            );

        // TODO: add dynamic constraints based on res

        select.render(main_area, buf);

        let inner_chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .flex(Flex::Center)
        .margin(2)
        .split(main_area);

        for (i, item) in self.list.items.iter().enumerate() {
            let colour: Color;
            colour = if i == self.list.selected {
                theme.secondary.shift(20)
            } else {
                theme.secondary.color()
            };

            let paragraph = Paragraph::new(item.as_str())
                .centered()
                .style(Style::new().fg(colour));

            paragraph.render(inner_chunks[i], buf);
        }
    }
}
