use com::wireless::{common::AccessPoint, wpa_supplicant::WpaInterface};
use ratatui::{
    layout::{Constraint, Flex, Layout, Margin, Rect, Spacing},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, Paragraph, Widget, Wrap},
};

use crate::{
    state::WpaInterfacePrompt,
    ui::{THEME, Theme},
};

pub struct WpaInterfaceUi<'a> {
    info: &'a WpaInterfacePrompt,
    ifaces: &'a Vec<WpaInterface>,
    persist: bool,
    theme: &'a Theme,
}

impl<'a> WpaInterfaceUi<'a> {
    pub fn new(
        info: &'a WpaInterfacePrompt,
        persist: bool,
        ifaces: &'a Vec<WpaInterface>,
    ) -> Option<Self> {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self {
            info,
            ifaces,
            persist,
            theme,
        })
    }
}

impl<'a> Widget for WpaInterfaceUi<'a> {
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
                Line::from(" Interfaces ")
                    .centered()
                    .style(Style::new().fg(theme.accent.color()).bold()),
            )
            .style(
                Style::new()
                    .fg(theme.info.color())
                    .bg(theme.background.color()),
            );

        border_block.render(areas.outer, buf);

        // info
        let info_text = "Selecting an interface either adds it to your wpa_supplicant configuration temporarily or permanently (will work after a reboot). For permanent changes enable persisting changes.";
        let info = Paragraph::new(info_text)
            .style(theme.accent.color())
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true });

        info.render(areas.info, buf);

        // choice
        let choice_text = "Apply and persist changes?";
        let cols = Layout::horizontal([
            Constraint::Length(choice_text.len() as u16),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .flex(Flex::Center)
        .split(areas.choice);

        let (choice_text_col, choice_text_bold) = if self.info.on_choice {
            (theme.primary.color(), Modifier::BOLD)
        } else {
            (theme.muted.color(), Modifier::empty())
        };

        Paragraph::new(choice_text)
            .style(
                Style::new()
                    .fg(choice_text_col)
                    .add_modifier(choice_text_bold),
            )
            .render(cols[0], buf);

        Paragraph::new(if self.persist { "Yes" } else { "No" })
            .style(Style::new().fg(if self.info.on_choice {
                theme.accent.color()
            } else {
                theme.muted.color()
            }))
            .render(cols[2], buf);

        let layouts = Layout::vertical(
            self.ifaces
                .iter()
                .map(|_| Constraint::Length(1))
                .collect::<Vec<_>>(),
        )
        .split(
            Layout::horizontal([Constraint::Percentage(70)])
                .flex(Flex::Center)
                .split(areas.list)[0],
        );

        // interfaces
        for (i, iface) in self.ifaces.iter().enumerate() {
            // for now skip later be able to manage it
            if let WpaInterface::Managed(_) = iface {
                continue;
            }

            let iface_string: String = iface.into();
            let mut iface_text = Line::from(iface_string);

            if i == self.info.interface_cursor && !self.info.on_choice {
                iface_text.style = Style::new()
                    .bg(theme.secondary.color())
                    .fg(theme.background.color())
                    .bold();
            } else {
                iface_text.style = Style::new()
                    .bg(theme.background.color())
                    .fg(theme.foreground.color());
            }

            iface_text.render(layouts[i], buf);
        }
    }
}

fn build_areas(area: Rect) -> InterfaceAreas {
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

    let [info_area, choice_area, list_area] = Layout::vertical([
        Constraint::Percentage(20),
        Constraint::Length(3),
        Constraint::Percentage(60),
    ])
    .flex(Flex::Center)
    .areas(inner_area);

    InterfaceAreas {
        outer: outer_area,
        inner: inner_area,
        choice: choice_area,
        info: info_area,
        list: list_area,
    }
}

pub struct InterfaceAreas {
    outer: Rect,
    inner: Rect,
    choice: Rect,
    info: Rect,
    list: Rect,
}
