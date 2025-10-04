use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{
    App, AppMessage, Query,
    ui::{THEME, Theme},
};

pub struct MainMenu {
    tx: UnboundedSender<AppMessage>,
}

impl MainMenu {
    pub fn new(tx: UnboundedSender<AppMessage>) -> Self {
        Self { tx }
    }
}

impl Widget for MainMenu {
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

        let (res, recv) = oneshot::channel();

        let _ = self
            .tx
            .send(AppMessage::Query(Query::MainMenu { res: res }));

        let main_menu = tokio::task::block_in_place(|| 
            // TODO: propagate error
            recv.blocking_recv().unwrap());

        for (i, &item) in main_menu.items.iter().enumerate() {
            let colour: Color;
            colour = if i == main_menu.selected {
                theme.secondary.shift(20)
            } else {
                theme.secondary.color()
            };

            let paragraph = Paragraph::new(item)
                .centered()
                .style(Style::new().fg(colour));

            paragraph.render(inner_chunks[i], buf);
        }
    }
}
