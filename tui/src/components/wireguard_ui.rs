use com::wireguard::store::WgMeta;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Padding, Paragraph, Widget},
};

use crate::{
    state::{SelectableList, VpnSelection, VpnState},
    ui::{Theme, THEME},
};

// min num of cols, max num of cols, target line amount
const COLS: (usize, usize, u16) = (2, 6, 30);
// target line amount
const ROW_H: u16 = 6;

pub struct WireguardMenu<'a> {
    state: &'a VpnState,
    names: &'a Vec<String>,
    meta: &'a [WgMeta],
    theme: &'a Theme,
}

impl<'a> WireguardMenu<'a> {
    pub fn new(state: &'a VpnState, names: &'a Vec<String>, meta: &'a [WgMeta]) -> Option<Self> {
        let theme: &Theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self {
            state,
            names,
            meta,
            theme,
        })
    }
}

impl<'a> Widget for WireguardMenu<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let theme = &self.theme;

        Clear.render(area, buf);
        buf.set_style(
            area,
            Style::new()
                .fg(theme.background.color())
                .bg(theme.background.color()),
        );

        let mut main_area = Layout::horizontal([Constraint::Percentage(80)])
            .flex(Flex::Center)
            .split(
                Layout::vertical([Constraint::Percentage(90)])
                    .flex(Flex::Center)
                    .areas::<1>(area)[0],
            )[0];

        let main_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(theme.primary.color())
            .title_top(
                Line::from(" WireGuard ")
                    .centered()
                    .style(theme.accent.color())
                    .bold(),
            );
        main_block.render(main_area, buf);

        let item_count = match self.names.len().cmp(&self.meta.len()) {
            std::cmp::Ordering::Equal => self.names.len(),
            std::cmp::Ordering::Less => {
                tracing::warn!("Wireguard names & meta are not the same length");
                self.names.len()
            }
            std::cmp::Ordering::Greater => {
                tracing::warn!("Wireguard names & meta are not the same length");
                self.meta.len()
            }
        };
        if item_count <= 0 {
            return;
        }

        // selection options

        // without this it can extend beyond main border
        main_area.y += 1;
        main_area.height -= 1;
        main_area.x += 1;
        main_area.width -= 2;
        let [top, main] = Layout::vertical([Constraint::Percentage(7), Constraint::Fill(1)])
            .areas::<2>(main_area);

        let options_layout = Layout::horizontal([Constraint::Percentage(95)])
            .flex(Flex::Center)
            .split(top);

        let opt_block = Block::new()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(theme.accent.color()));

        opt_block.render(options_layout[0], buf);

        let main_area = Layout::horizontal([Constraint::Percentage(90)])
            .flex(Flex::Center)
            .split(main)[0];
        // rows displayable
        let rows = (main_area.height / ROW_H) as usize;
        tracing::info!("ROWS: {rows}, height: {}", main_area.height);
        if rows <= 0 {
            tracing::error!("Not enough room to display a single row...");
            return;
        }

        let cols = match calc_max_cols(main_area) {
            Some(c) => c,
            None => {
                tracing::error!("Not enough room to display a single column...");
                return;
            }
        };

        let items_per_page = rows * cols;
        let mut selected_item =
            self.state.file_cursor.x + (self.state.file_cursor.y * (cols as usize));
        if self.state.file_cursor.x_alt > 0 {
            let mult = self.state.file_cursor.x_alt / (cols + 1);
            tracing::info!(
                "X: {}, X_ALT: {}, Y: {}, MULT: {}",
                self.state.file_cursor.x,
                self.state.file_cursor.x_alt,
                self.state.file_cursor.y,
                mult
            );
            selected_item = (self.state.file_cursor.y * (cols as usize))
                .saturating_sub(self.state.file_cursor.x_alt);
        }
        let current_page = selected_item / items_per_page;

        let mut item_areas: Vec<Rect> = vec![];
        let rows_layout = Layout::vertical(vec![Constraint::Percentage(100 / (rows as u16)); rows])
            .split(main_area);

        for row in rows_layout.into_iter() {
            let cols = Layout::horizontal(vec![Constraint::Percentage(100 / (cols as u16)); cols])
                .flex(Flex::Center)
                .split(*row);

            item_areas.append(&mut cols.to_vec());
        }
        tracing::info!("ITEM AREAS: {:?}", item_areas);

        for (i, area) in item_areas.iter().enumerate() {
            let i = i + items_per_page * current_page;
            if i > item_count {
                break;
            }

            let style = if selected_item == i {
                Style::new().bg(theme.success.color())
            } else {
                Style::new().bg(theme.error.color())
            };

            match self.names.get(i) {
                Some(name) => {
                    let block = Block::new()
                        .style(style)
                        .title_top(Line::from(format!("{}", name)).left_aligned());
                    block.render(*area, buf);
                }
                None => {
                    return;
                }
            }
        }
    }
}

pub fn calc_max_cols(area: Rect) -> Option<usize> {
    let mut max_cols = 0;
    // from max col to min until
    // first which provide enough pixels
    for i in (COLS.0..=COLS.1).rev() {
        tracing::info!("{}", area.width / (i as u16));
        if area.width / (i as u16) > COLS.2 {
            max_cols = i;
            break;
        }
    }
    if max_cols > 0 {
        Some(max_cols)
    } else {
        None
    }
}

fn render_wg_item<'a>(name: &'a String, meta: &'a WgMeta) {}

struct Entry<'a> {
    name: &'a String,
    info: &'a WgMeta,
}

impl<'a> Widget for Entry<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
    }
}

impl<'a> Entry<'a> {
    fn new(name: &'a String, info: &'a WgMeta) -> Self {
        Self { name, info }
    }
}
