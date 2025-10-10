use std::sync::Arc;

use ratatui::{
    layout::{self, Constraint, Flex, Layout},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget, canvas::Label},
};
use tokio::sync::RwLock;
use tracing::info;

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
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
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

        // 80% horizontal, 70% vertical and centered
        let outer_area = Layout::horizontal([Constraint::Percentage(80)])
            .flex(Flex::Center)
            .areas::<1>(
                Layout::vertical([Constraint::Percentage(70)])
                    .flex(Flex::Center)
                    .areas::<1>(area)[0],
            )[0];

        let outer_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::all())
            .style(Style::new().fg(theme.secondary.shift(30)))
            .title_top(
                Line::from(" Networks ")
                    .centered()
                    .style(Style::new().fg(theme.secondary.shift(10))),
            );
        outer_block.render(outer_area, buf);

        let label_area = Layout::horizontal([Constraint::Percentage(30)])
            .flex(Flex::End)
            .margin(2)
            .areas::<1>(
                Layout::vertical([Constraint::Percentage(100)])
                    .flex(Flex::Center)
                    .areas::<1>(outer_area)[0],
            )[0];

        let label_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::all())
            .style(Style::new().fg(theme.secondary.shift(50)));

        // labels

        let constraints: Vec<Constraint> = std::iter::repeat(Constraint::Length(1))
            .take(self.list.items.len())
            .collect();

        let label_chunks = Layout::default()
            .direction(layout::Direction::Vertical)
            .margin(2)
            .constraints(constraints)
            .split(label_area);

        for (index, item) in self.list.items.iter().enumerate() {
            if index == self.list.selected {}
            let label = Paragraph::new(item.as_str()).style(theme.info.color());
            label.render(label_chunks[index], buf);
        }

        label_block.render(label_area, buf);
        info!("{:?}", self.network.aps);
    }
}
