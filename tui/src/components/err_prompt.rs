use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Offset, Rect},
    style::{palette::material::WHITE, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::{
    state::{MainMenuSelection, SelectableList},
    ui::{Theme, THEME},
};

pub struct ErrorPrompt<'a> {
    theme: &'a Theme,
    reason: &'a String,
}

impl<'a> ErrorPrompt<'a> {
    pub fn new(reason: &'a String) -> Option<Self> {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self { theme, reason })
    }
}

impl<'a> Widget for ErrorPrompt<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let theme = &self.theme;

        let [main_area] = Layout::vertical([Constraint::Percentage(50)])
            .flex(Flex::Center)
            .areas(
                Layout::horizontal([Constraint::Percentage(50)])
                    .flex(Flex::Center)
                    .areas::<1>(area)[0],
            );
        Clear.render(main_area, buf);

        buf.set_style(
            main_area,
            Style::new()
                .fg(theme.background.color())
                .bg(theme.background.color()),
        );

        let main_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(
                Style::new()
                    .bg(theme.background.color())
                    .fg(theme.error.color()),
            )
            .title_top(
                Line::from(" ERROR ")
                    .centered()
                    .style(Style::new().fg(theme.error.color()).bold()),
            );

        let main_inner = main_block.inner(main_area);
        main_block.render(main_area, buf);

        let span = Span::styled(
            self.reason,
            Style::new()
                .bg(theme.background.color())
                .fg(theme.foreground.color())
                .bold(),
        )
        .into_centered_line();

        span.render(main_inner.offset(Offset { x: 0, y: 1 }), buf);

        let exit_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(Style::new().fg(theme.accent.color()));

        let btn_layout = Layout::vertical([Constraint::Min(0), Constraint::Length(3)])
            .flex(Flex::Center)
            .split(
                Layout::horizontal([Constraint::Ratio(1, 3); 3])
                    .flex(Flex::Center)
                    .split(main_inner)[1],
            );

        let btn_layout = Layout::horizontal([Constraint::Percentage(50)])
            .flex(Flex::Center)
            .spacing(1)
            .split(btn_layout[1]);

        let btn_span =
            Span::styled("OK", Style::new().fg(theme.accent.color()).bold()).into_centered_line();
        let btn_area = exit_block.inner(btn_layout[0]);

        exit_block.render(btn_layout[0], buf);
        btn_span.render(btn_area, buf);
    }
}
