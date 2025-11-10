use com::wireless::common::AccessPoint;
use ratatui::{
    layout::{Constraint, Flex, Layout},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::{
    state::{ConnectionPrompt, ConnectionPromptSelect},
    ui::{Theme, THEME},
};

pub struct NetworkPrompt<'a> {
    network: &'a AccessPoint,
    selected: &'a ConnectionPrompt,
}

impl<'a> NetworkPrompt<'a> {
    pub fn new(ap: &'a AccessPoint, selected: &'a ConnectionPrompt) -> Self {
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

        Clear.render(area, buf);

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

        let inner_area = main_block.inner(area);
        main_block.render(area, buf);

        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .margin(1)
        .split(inner_area);

        let ssid_area = chunks[0];
        let password_area = chunks[2];

        let ssid_line = Line::from(vec![
            Span::styled("  SSID: ", Style::new().fg(theme.tertiary.color())),
            Span::styled(
                self.network.ssid.clone(),
                Style::new().fg(theme.accent.color()),
            ),
        ]);

        Paragraph::new(ssid_line).render(ssid_area, buf);

        let password_box_style = if matches!(self.selected.select, ConnectionPromptSelect::Password)
        {
            Style::new().fg(theme.accent.color())
        } else {
            Style::new().fg(theme.muted.color())
        };

        let password_box = Block::new()
            .title_top(Line::from(" Password ").style(Style::new().fg(theme.tertiary.color())))
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(password_box_style);

        let password_text_area = Layout::horizontal([Constraint::Percentage(98)])
            .flex(Flex::Center)
            .split(password_box.inner(password_area))[0];

        password_box.render(password_area, buf);
        let password_text = Paragraph::new("*".repeat(self.selected.password.len()));
        password_text.render(password_text_area, buf);

        let connect_box_style = if matches!(self.selected.select, ConnectionPromptSelect::Connect) {
            Style::new().fg(theme.accent.color())
        } else {
            Style::new().fg(theme.muted.color())
        };

        let connect_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(connect_box_style);

        let back_block_style = if matches!(self.selected.select, ConnectionPromptSelect::Back) {
            Style::new().fg(theme.accent.color())
        } else {
            Style::new().fg(theme.muted.color())
        };

        let back_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(back_block_style);

        let button_layouts = Layout::vertical([Constraint::Min(0), Constraint::Length(3)])
            .flex(Flex::Center)
            .split(
                Layout::horizontal([Constraint::Min(0), Constraint::Percentage(60)])
                    .flex(Flex::Center)
                    .split(inner_area)[1],
            );

        let button_layouts = Layout::horizontal([
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(1),
        ])
        .flex(Flex::End)
        .spacing(1)
        .split(button_layouts[1]);

        Line::from("Connect")
            .style(Style::new().fg(theme.muted.color()))
            .centered()
            .render(connect_block.inner(button_layouts[1]), buf);

        connect_block.render(button_layouts[1], buf);

        Line::from("Back")
            .style(Style::new().fg(theme.muted.color()))
            .centered()
            .render(back_block.inner(button_layouts[0]), buf);

        back_block.render(button_layouts[0], buf);
    }
}
