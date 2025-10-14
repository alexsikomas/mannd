use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

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
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => return,
        };

        let max_item_width = self
            .list
            .items
            .iter()
            .map(|item| item.as_str().len() as u16)
            .max()
            .unwrap_or(10);
        let box_width = max_item_width + 16;

        let box_height = self.list.items.len() as u16 + 4;

        let centered_area = Layout::vertical([Constraint::Length(box_height)])
            .flex(Flex::Center)
            .split(
                Layout::horizontal([Constraint::Length(box_width)])
                    .flex(Flex::Center)
                    .split(area)[0],
            )[0];

        let select_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(Style::new().fg(theme.primary.color()))
            .title_top(
                Line::from(" Select ")
                    .centered()
                    .style(Style::new().fg(theme.accent.color())),
            );

        let inner_area = select_block.inner(centered_area);
        select_block.render(centered_area, buf);

        let inner_chunks = Layout::vertical(
            self.list
                .items
                .iter()
                .map(|_| Constraint::Length(1))
                .collect::<Vec<_>>(),
        )
        .flex(Flex::Center)
        .split(inner_area);

        for (i, item) in self.list.items.iter().enumerate() {
            if i >= inner_chunks.len() {
                break;
            }

            let (fg_col, bg_col) = if i == self.list.selected {
                (theme.background.color(), theme.secondary.color())
            } else {
                (theme.foreground.color(), theme.background.color())
            };

            let paragraph = Paragraph::new(item.as_str())
                .centered()
                .style(Style::new().fg(fg_col).bold());

            if i == self.list.selected {
                let highlight_area = Layout::horizontal([Constraint::Percentage(95)])
                    .flex(Flex::Center)
                    .split(inner_chunks[i])[0];

                Block::default()
                    .style(Style::new().bg(bg_col))
                    .render(highlight_area, buf);
            }

            paragraph.render(inner_chunks[i], buf);
        }
    }
}
