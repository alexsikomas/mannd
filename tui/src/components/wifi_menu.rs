use core::{
    controller::DaemonType,
    wireless::common::{AccessPoint, NetworkFlags, Security},
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{
        self, Block, Borders, Clear, List, ListDirection, ListItem, ListState, Paragraph, Widget,
    },
};

use crate::{
    state::{ConnectionAction, ConnectionFocus, WifiState},
    ui::{THEME, Theme},
};

pub struct Connection<'a> {
    networks: &'a [AccessPoint],
    conn_state: &'a WifiState,
    theme: &'a Theme,
}

impl<'a> Connection<'a> {
    pub fn new(networks: &'a [AccessPoint], conn_state: &'a WifiState) -> Option<Self> {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self {
            networks,
            conn_state,
            theme,
        })
    }
}

impl<'a> Widget for Connection<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let theme = &self.theme;
        let heading_styles = self.heading_styles(&self.conn_state.focused_area);

        let main_chunks =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(area);

        let network_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(Style::new().fg(
                if self.conn_state.focused_area == ConnectionFocus::Networks {
                    theme.primary.color()
                } else {
                    theme.muted.color()
                },
            ))
            .title_top(
                Line::from(" Network Status ")
                    .centered()
                    .style(heading_styles.1),
            );

        // Networks (left)
        let mut network_ssids: Vec<ListItem> = vec![];
        for (_i, network) in self.networks.iter().enumerate() {
            // precedence: selected > connected > known > default

            // let is_selected = i == self.conn_state.network_cursor;
            let is_focused = self.conn_state.focused_area == ConnectionFocus::Networks;
            let network_style = self.network_style(network, is_focused);

            let mut spans = vec![Span::styled(network.ssid.clone(), network_style)];

            let sec_span = Self::security_span(&network.security, network_style);
            spans.push(sec_span);

            let line = Line::from(spans);
            network_ssids.push(ListItem::new(line));
        }

        let network_list = List::new(network_ssids)
            .block(network_block)
            .direction(ListDirection::TopToBottom)
            .scroll_padding(1)
            .highlight_symbol("> ")
            .highlight_style(Style::new().fg(theme.accent.color()).bold());

        let mut network_list_state = ListState::default();
        network_list_state.select(Some(self.conn_state.network_cursor));

        widgets::StatefulWidget::render(network_list, main_chunks[0], buf, &mut network_list_state);

        let selection_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(Style::new().fg(theme.muted.color()))
            .title_top(Line::from(" Options ").centered().style(heading_styles.0));

        // list not used here because highlight symbol can't be centered
        let selection_area = selection_block.inner(main_chunks[1]);
        selection_block.render(main_chunks[1], buf);

        let selection_chunks = Layout::vertical(
            self.conn_state
                .actions
                .items
                .iter()
                .map(|_| Constraint::Length(1))
                .collect::<Vec<_>>(),
        )
        .flex(Flex::Center)
        .split(selection_area);

        // Action labels (right)
        for (i, item) in self.conn_state.actions.items.iter().enumerate() {
            if i >= selection_chunks.len() {
                break;
            }

            let is_selected = i == self.conn_state.actions.selected_index;
            let (fg_col, bg_col) =
                self.action_item_colors(&self.conn_state.focused_area, is_selected);

            let paragraph = Paragraph::new(item.as_str())
                .centered()
                .style(Style::new().fg(fg_col).bold());

            if is_selected && self.conn_state.focused_area == ConnectionFocus::Actions {
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

impl<'a> Connection<'a> {
    fn security_span(security: &Security, network_style: Style) -> Span<'a> {
        match security {
            Security::Psk => Span::styled("  ".to_string(), network_style.bold()),
            Security::Open => Span::styled(" (Open)".to_string(), network_style),
            Security::Ieee8021x => Span::styled(" (EAP)".to_string(), network_style),
            Security::Unknown => Span::styled("".to_string(), network_style),
        }
    }

    fn heading_styles(&self, focus: &ConnectionFocus) -> (Style, Style) {
        match focus {
            ConnectionFocus::Actions => (
                Style::new().fg(self.theme.accent.color()).bold(),
                Style::new().fg(self.theme.accent.color()),
            ),
            ConnectionFocus::Networks => (
                Style::new().fg(self.theme.accent.color()),
                Style::new().fg(self.theme.accent.color()).bold(),
            ),
        }
    }

    fn action_item_colors(&self, focus: &ConnectionFocus, is_selected: bool) -> (Color, Color) {
        if *focus == ConnectionFocus::Actions && is_selected {
            return (self.theme.background.color(), self.theme.secondary.color());
        } else {
            return (self.theme.foreground.color(), self.theme.background.color());
        }
    }

    fn network_style(&self, ap: &AccessPoint, is_focused: bool) -> Style {
        let mut style = Style::new();
        if ap.flags.contains(NetworkFlags::CONNECTED) && is_focused {
            let fg_col = self.theme.success.color();
            style = style.fg(fg_col).bold();
        } else if ap.flags.contains(NetworkFlags::KNOWN) && is_focused {
            let fg_col = self.theme.tertiary.color();
            style = style.fg(fg_col).italic();
        }
        style
    }
}
