use com::wireless::common::AccessPoint;
use ratatui::{
    layout::{Constraint, Flex, Layout, Margin, Rect, Spacing},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, Paragraph, Widget, Wrap},
};

use crate::{
    state::WpaInterfacePrompt,
    ui::{Theme, THEME},
};

pub struct WpaInterfaceUi<'a> {
    info: &'a WpaInterfacePrompt,
    ifaces: &'a Vec<String>,
    theme: &'a Theme,
}

impl<'a> WpaInterfaceUi<'a> {
    pub fn new(info: &'a WpaInterfacePrompt, ifaces: &'a Vec<String>) -> Option<Self> {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self {
            info,
            ifaces,
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

        let info_text = "Selecting an interface here will add it to your wpa_supplicant service file and will work at startup.\nPress Enter to select and ESC to return";
        let info = Paragraph::new(info_text)
            .style(theme.accent.color())
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true });

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

        for (i, iface) in self.ifaces.iter().enumerate() {
            let mut iface_text = Line::from(iface.clone());
            if i == self.info.interface_cursor {
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

        info.render(areas.info, buf);
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

    let [info_area, list_area] =
        Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(70)])
            .flex(Flex::Center)
            .areas(inner_area);

    InterfaceAreas {
        outer: outer_area,
        inner: inner_area,
        info: info_area,
        list: list_area,
    }
}

pub struct InterfaceAreas {
    outer: Rect,
    inner: Rect,
    info: Rect,
    list: Rect,
}
