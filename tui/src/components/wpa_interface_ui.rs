use std::fmt::format;

use mannd::wireless::wpa_supplicant::WpaInterface;
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

use crate::{components::layout::centered_overlay, state::prompts::WpaInterfacePrompt, ui::theme};

pub struct WpaInterfaceUi<'a> {
    info: &'a WpaInterfacePrompt,
    ifaces: &'a [WpaInterface],
}

impl<'a> WpaInterfaceUi<'a> {
    pub fn new(info: &'a WpaInterfacePrompt, ifaces: &'a [WpaInterface]) -> Option<Self> {
        Some(Self { info, ifaces })
    }
}

impl Widget for WpaInterfaceUi<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let theme = theme();

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
        let info_text = if let Some(iface) = &self.info.pending_remove {
            format!(
                "Warning: press Enter again to remove managed interface '{iface}'. Move selection to cancel."
            )
        } else {
            "Select unmanaged interfaces to add them. Select managed interfaces and press Enter twice to remove them. Toggle persistant state above.".to_string()
        };

        Paragraph::new(info_text)
            .style(Style::new().fg(if self.info.pending_remove.is_some() {
                theme.warning.color()
            } else {
                theme.accent.color()
            }))
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true })
            .render(areas.info, buf);

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

        Paragraph::new(if self.info.persist { "Yes" } else { "No" })
            .style(Style::new().fg(if self.info.on_choice {
                theme.accent.color()
            } else {
                theme.muted.color()
            }))
            .render(cols[2], buf);

        let mut unmanaged: Vec<(usize, &str)> = vec![];
        let mut managed: Vec<(usize, &str)> = vec![];

        for (idx, iface) in self.ifaces.iter().enumerate() {
            if iface.is_managed() {
                managed.push((idx, iface.name()));
            } else {
                unmanaged.push((idx, iface.name()));
            }
        }

        let ordered = WpaInterfacePrompt::ordered_iface_indicies(self.ifaces);
        let selected_idx = if self.info.on_choice {
            None
        } else {
            ordered.get(self.info.interface_cursor.index).copied()
        };

        let mut lines: Vec<Line> = vec![];

        lines.push(
            Line::from(" Unmanaged ").style(
                Style::new()
                    .fg(theme.info.color())
                    .add_modifier(Modifier::BOLD),
            ),
        );

        if unmanaged.is_empty() {
            lines.push(Line::from(" (none) ").style(Style::new().fg(theme.muted.color())));
        } else {
            for (idx, name) in unmanaged {
                let mut line = Line::from(format!(" {name} "));
                if selected_idx == Some(idx) {
                    line.style = Style::new()
                        .bg(theme.secondary.color())
                        .fg(theme.background.color())
                        .add_modifier(Modifier::BOLD);
                } else {
                    line.style = Style::new().fg(theme.foreground.color());
                }
                lines.push(line);
            }
        }

        lines.push(Line::from(""));

        lines.push(
            Line::from(" Managed ").style(
                Style::new()
                    .fg(theme.info.color())
                    .add_modifier(Modifier::BOLD),
            ),
        );

        if managed.is_empty() {
            lines.push(Line::from(" (none) ").style(Style::new().fg(theme.muted.color())));
        } else {
            for (idx, name) in managed {
                let mut label = format!(" {name} ");
                if self.info.pending_remove.as_deref() == Some(name) {
                    label.push_str(" [press Enter again to remove]");
                }

                let mut line = Line::from(label);
                if selected_idx == Some(idx) {
                    line.style = Style::new()
                        .bg(theme.secondary.color())
                        .fg(theme.background.color())
                        .add_modifier(Modifier::BOLD);
                } else if self.info.pending_remove.as_deref() == Some(name) {
                    line.style = Style::new().fg(theme.warning.color());
                } else {
                    line.style = Style::new().fg(theme.foreground.color());
                }

                lines.push(line);
            }
        }

        Paragraph::new(lines).wrap(Wrap { trim: false }).render(
            Layout::horizontal([Constraint::Percentage(80)])
                .flex(Flex::Center)
                .split(areas.list)[0],
            buf,
        );
    }
}

fn build_areas(area: Rect) -> InterfaceAreas {
    let (outer, inner) = centered_overlay(area, 75, 75);

    let [info_area, choice_area, list_area] = Layout::vertical([
        Constraint::Percentage(20),
        Constraint::Length(3),
        Constraint::Percentage(60),
    ])
    .flex(Flex::Center)
    .areas(inner);

    InterfaceAreas {
        outer,
        inner,
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
