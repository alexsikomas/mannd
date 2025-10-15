use com::wireless::common::{AccessPoint, Security};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};
use tracing::info;

use crate::{
    network::NetworkState,
    state::{ConnectionAction, FocusedConnection, SelectableList},
    ui::{THEME, Theme, ThemeColor},
};

pub struct Connection<'a> {
    networks: &'a SelectableList<AccessPoint>,
    actions: &'a SelectableList<ConnectionAction>,
    focused: &'a FocusedConnection,
}

impl<'a> Connection<'a> {
    pub fn new(
        networks: &'a SelectableList<AccessPoint>,
        actions: &'a SelectableList<ConnectionAction>,
        focused: &'a FocusedConnection,
    ) -> Self {
        Self {
            networks,
            actions,
            focused,
        }
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

        let mut select_heading_style = Style::new().fg(theme.accent.color());
        if *self.focused == FocusedConnection::Networks {
            select_heading_style = Style::new().fg(theme.accent.color()).bold();
        }

        let main_chunks =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(area);

        let network_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(
                Style::new().fg(if *self.focused == FocusedConnection::Networks {
                    theme.primary.color()
                } else {
                    theme.muted.color()
                }),
            )
            .title_top(
                Line::from(" Network Status ")
                    .centered()
                    .style(select_heading_style),
            );

        let network_area = network_block.inner(main_chunks[0]);
        let network_chunks = Layout::new(
            Direction::Vertical,
            self.networks
                .items
                .iter()
                .map(|_| Constraint::Length(1))
                .collect::<Vec<_>>(),
        )
        .margin(1)
        .split(network_area);

        network_block.render(main_chunks[0], buf);

        info!("{:?}", self.networks);
        for (i, network) in self.networks.items.iter().enumerate() {
            let mut text = network.ssid.clone();
            // precedence: selected > connected > known > default
            let (mut fg_col, bg_col) = (theme.foreground.color(), theme.background.color());
            if self.networks.selected == i {
                if *self.focused == FocusedConnection::Networks {
                    fg_col = theme.accent.color();
                } else {
                    fg_col = theme.info.color();
                }
            } else if self.networks.items[i].connected {
                fg_col = theme.success.color();
            } else if self.networks.items[i].known {
                fg_col = theme.tertiary.color();
            }

            match network.security {
                Security::Psk => {
                    text.push_str("  ");
                }
                Security::Open => {
                    text.push_str(" (Open)");
                }
                Security::Ieee8021x => {
                    text.push_str(" (802.1x)");
                }
            }
            // select hover for options like connect
            Paragraph::new(text)
                .style(Style::new().fg(fg_col).bold())
                .render(network_chunks[i], buf);
        }

        select_heading_style = Style::new().fg(theme.accent.color());
        if *self.focused == FocusedConnection::Actions {
            select_heading_style = Style::new().fg(theme.accent.color()).bold();
        }

        let selection_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(
                Style::new().fg(if *self.focused == FocusedConnection::Actions {
                    theme.primary.color()
                } else {
                    theme.muted.color()
                }),
            )
            .title_top(
                Line::from(" Options ")
                    .centered()
                    .style(select_heading_style),
            );

        let selection_area = selection_block.inner(main_chunks[1]);
        selection_block.render(main_chunks[1], buf);

        let selection_chunks = Layout::vertical(
            self.actions
                .items
                .iter()
                .map(|_| Constraint::Length(1))
                .collect::<Vec<_>>(),
        )
        .flex(Flex::Center)
        .split(selection_area);

        for (i, item) in self.actions.items.iter().enumerate() {
            if i >= selection_chunks.len() {
                break;
            }

            let (mut fg_col, mut bg_col) = (theme.foreground.color(), theme.background.color());

            if i == self.actions.selected && *self.focused == FocusedConnection::Actions {
                fg_col = theme.background.color();
                bg_col = theme.secondary.color();
            }

            let paragraph = Paragraph::new(item.as_str())
                .centered()
                .style(Style::new().fg(fg_col).bold());

            if i == self.actions.selected && *self.focused == FocusedConnection::Actions {
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
