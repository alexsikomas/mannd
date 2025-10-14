use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};
use tracing::info;

use crate::{
    app::{SelectableList, Selection},
    network::NetworkState,
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

        let network_area = network_block.inner(main_chunks[0]);
        let network_chunks = Layout::new(
            Direction::Vertical,
            self.network
                .aps
                .iter()
                .map(|_| Constraint::Length(1))
                .collect::<Vec<_>>(),
        )
        .margin(1)
        .split(network_area);

        network_block.render(main_chunks[0], buf);

        info!("{:?}", self.network.aps);
        for (i, network) in self.network.aps.iter().enumerate() {
            let mut fg_col = theme.foreground.color();
            if let Selection::Network(val) = self.list.items[self.list.selected] {
                if i == val[0] {
                    fg_col = theme.accent.color();
                }
            }
            Paragraph::new(network.ssid.clone())
                .style(Style::new().fg(fg_col).bold())
                .render(network_chunks[i], buf);
        }

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

        info!("{:?}", self.list.items);
        // skip first value as it's for knowing if we are in the left menu
        for (i, item) in self.list.items.iter().skip(1).enumerate() {
            if i >= selection_chunks.len() {
                break;
            }

            let (fg_col, bg_col) = if (i + 1) == self.list.selected {
                (theme.background.color(), theme.secondary.color())
            } else {
                (theme.foreground.color(), theme.background.color())
            };

            let paragraph = Paragraph::new(item.as_str())
                .centered()
                .style(Style::new().fg(fg_col).bold());

            if i + 1 == self.list.selected {
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
