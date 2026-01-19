use com::wireguard::store::WgMeta;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::{
    state::{SelectableList, VpnSelection},
    ui::{Theme, THEME},
};

pub struct WireguardMenu<'a> {
    list: &'a SelectableList<VpnSelection>,
    files: &'a Vec<WgMeta>,
    theme: &'a Theme,
}

impl<'a> WireguardMenu<'a> {
    pub fn new(list: &'a SelectableList<VpnSelection>, files: &'a Vec<WgMeta>) -> Option<Self> {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self { list, files, theme })
    }
}

impl<'a> Widget for WireguardMenu<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let theme = &self.theme;
        let main_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(theme.primary.color())
            .title_top(
                Line::from(" WireGuard ")
                    .centered()
                    .style(theme.accent.color()),
            );
        main_block.render(area, buf);
    }
}
