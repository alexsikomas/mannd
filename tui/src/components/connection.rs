use std::sync::Arc;

use ratatui::{
    buffer::Buffer,
    layout::{self, Constraint, Direction, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use tokio::sync::RwLock;

use crate::{
    app::{NetworkState, SelectableList, Selection},
    ui::{THEME, Theme},
};

pub struct Connection<'a> {
    list: &'a SelectableList<Selection>,
    network: &'a NetworkState,
}

impl<'a> Connection<'a> {
    pub fn new(list: &'a SelectableList<Selection>, network: &'a NetworkState) -> Self {
        Self { list, network }
    }
}

impl<'a> Widget for Connection<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => return,
        };

        let main_chunks =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(area);

        let network_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(Style::new().fg(theme.primary.color()))
            .title_top(
                Line::from(" Network Status ")
                    .centered()
                    .style(Style::new().fg(theme.accent.color())),
            );

        let network_details =
            Text::from(vec![Line::from(""), Line::from("      Available APs: ...")])
                .style(Style::new().fg(theme.info.color()));

        let network_paragraph = Paragraph::new(network_details);

        let network_area = network_block.inner(main_chunks[0]);
        network_block.render(main_chunks[0], buf);
        network_paragraph.render(network_area, buf);

        let selection_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(Style::new().fg(theme.primary.color()))
            .title_top(
                Line::from(" Options ")
                    .centered()
                    .style(Style::new().fg(theme.accent.color())),
            );

        let selection_area = selection_block.inner(main_chunks[1]);
        selection_block.render(main_chunks[1], buf);

        let selection_chunks = Layout::vertical(
            self.list
                .items
                .iter()
                .map(|_| Constraint::Length(1))
                .collect::<Vec<_>>(),
        )
        .flex(Flex::Center)
        .split(selection_area);

        for (i, item) in self.list.items.iter().enumerate() {
            if i >= selection_chunks.len() {
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
                    .split(selection_chunks[i])[0];

                Block::default()
                    .style(Style::new().bg(bg_col))
                    .render(highlight_area, buf);
            }

            paragraph.render(selection_chunks[i], buf);
        }
    }
}

