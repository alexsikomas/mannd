use com::wireless::common::AccessPoint;
use ratatui::{
    layout::{Constraint, Flex, Layout, Margin, Rect, Spacing},
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::{
    state::PskConnectionPrompt,
    ui::{Theme, THEME},
};

pub struct PasswordPrompt<'a> {
    network: &'a AccessPoint,
    info: &'a PskConnectionPrompt,
    theme: &'a Theme,
}

impl<'a> PasswordPrompt<'a> {
    pub fn new(ap: &'a AccessPoint, info: &'a PskConnectionPrompt) -> Option<Self> {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self {
            network: ap,
            info,
            theme,
        })
    }
}

impl<'a> Widget for PasswordPrompt<'a> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let theme = &self.theme;

        let areas = build_areas(area);
        Clear.render(areas.outer, buf);
        buf.set_style(
            areas.outer,
            Style::new()
                .fg(theme.background.color())
                .bg(theme.background.color()),
        );

        let border_block = Block::new()
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

        border_block.render(areas.outer, buf);

        let ssid_area = areas.chunks[0];
        let password_area = areas.chunks[2];

        let ssid_line = Line::from(vec![
            Span::styled("  SSID: ", Style::new().fg(theme.tertiary.color())),
            Span::styled(
                self.network.ssid.clone(),
                Style::new().fg(theme.accent.color()),
            ),
        ]);

        Paragraph::new(ssid_line).render(ssid_area, buf);

        // BUTTON STYLES
        // for order refer to PskPromptSelect::to_vec()
        let mut styles: Vec<Style> = vec![];
        for i in 0..self.info.select.items.len() {
            let style = if self.info.select.selected_index == i {
                Style::new().fg(theme.accent.color())
            } else {
                Style::new().fg(theme.muted.color())
            };
            styles.push(style);
        }

        let password_box = Block::new()
            .title_top(Line::from(" Password ").style(Style::new().fg(theme.tertiary.color())))
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(styles[0]);

        let show_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(styles[1])
            .title_top(Line::from(" Show ").centered());

        let password_areas = Layout::horizontal([
            Constraint::Percentage(100), // password box
            Constraint::Length(10),      // show password box
        ])
        .flex(Flex::Center)
        .spacing(Spacing::Space(1))
        .split(password_area);

        let password_text_area = Layout::horizontal([Constraint::Percentage(98)])
            .flex(Flex::Center)
            .split(password_box.inner(password_areas[0]))[0];

        password_box.render(password_areas[0], buf);
        show_block.render(password_areas[1], buf);
        let mut password_text = Paragraph::new("*".repeat(self.info.password.len()));
        let mut select_text = Line::from(" ");

        if self.info.show_password {
            let width = password_areas[0].width as usize;
            if self.info.password.len() > width {
                password_text = Paragraph::new(&self.info.password[width..]);
            } else {
                password_text = Paragraph::new(self.info.password.clone());
            }
            select_text = Line::from("X").centered();
        }

        password_text.render(password_text_area, buf);
        select_text.render(password_areas[1].inner(Margin::new(1, 1)), buf);

        let connect_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(styles[2]);

        let back_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(styles[3]);

        let button_layouts = Layout::vertical([Constraint::Min(0), Constraint::Length(3)])
            .flex(Flex::Center)
            .split(
                Layout::horizontal([Constraint::Min(0), Constraint::Percentage(60)])
                    .flex(Flex::Center)
                    .split(areas.inner)[1],
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

fn build_areas(area: Rect) -> PasswordAreas {
    let [outer_area] = Layout::vertical([Constraint::Percentage(75)])
        .flex(Flex::Center)
        .areas(
            Layout::horizontal([Constraint::Percentage(75)])
                .flex(Flex::Center)
                .areas::<1>(area)[0],
        );
    let border_block = Block::new()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded);
    let inner_area = border_block.inner(outer_area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .margin(1)
    .split(inner_area);

    PasswordAreas {
        outer: outer_area,
        inner: inner_area,
        chunks: chunks.to_vec(),
    }
}

pub struct PasswordAreas {
    outer: Rect,
    inner: Rect,
    chunks: Vec<Rect>,
}
