use mannd::store::WgMeta;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Margin, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Widget},
};

use crate::{
    components::layout::{panel_with_toolbar, selection_style},
    state::{VpnState, vpn::VpnSelection},
    ui::{Theme, theme},
};

// min num of cols, max num of cols, target line amount
const COLS: (usize, usize, u16) = (2, 6, 30);
// target line amount
const ROW_H: u16 = 6;

pub struct VpnMenu<'a> {
    state: &'a VpnState,
    names: &'a [String],
    meta: Option<&'a [WgMeta]>,
    wg_on: bool,
    areas: VpnAreas,
}

impl<'a> VpnMenu<'a> {
    pub fn new(
        state: &'a VpnState,
        names: &'a [String],
        meta: Option<&'a [WgMeta]>,
        wg_on: bool,
        areas: VpnAreas,
    ) -> Self {
        Self {
            state,
            names,
            meta,
            wg_on,
            areas,
        }
    }
}

impl Widget for VpnMenu<'_> {
    fn render(self, _area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let theme = theme();
        self.render_main_block(self.areas.outer, buf, theme);
        self.render_option_menu(self.areas.select, buf, theme);

        if let Some(meta) = self.meta {
            let item_count = match self.names.len().cmp(&meta.len()) {
                std::cmp::Ordering::Equal => self.names.len(),
                std::cmp::Ordering::Less => {
                    tracing::warn!("Wireguard names & meta are not the same length");
                    self.names.len()
                }
                std::cmp::Ordering::Greater => {
                    tracing::warn!("Wireguard names & meta are not the same length");
                    meta.len()
                }
            };
            if item_count <= 0 {
                return;
            }

            // rows displayable
            let rows = (self.areas.vpn.height / ROW_H) as usize;
            if rows <= 0 {
                tracing::error!("Not enough room to display a single row...");
                return;
            }

            let cols = if let Some(c) = calc_max_cols(self.areas.vpn) {
                c
            } else {
                tracing::error!("Not enough room to display a single column...");
                return;
            };

            let items_per_page = rows * cols;
            let selected_item = self.state.file_cursor.index;
            let current_page = selected_item / items_per_page;

            let mut item_areas: Vec<Rect> = vec![];
            let rows_layout =
                Layout::vertical(vec![Constraint::Percentage(97 / (rows as u16)); rows])
                    .split(self.areas.vpn);

            for row in rows_layout.iter() {
                let cols =
                    Layout::horizontal(vec![Constraint::Percentage(100 / (cols as u16)); cols])
                        .flex(Flex::Center)
                        .split(*row);

                item_areas.append(&mut cols.to_vec());
            }

            for (mut i, area) in item_areas.iter().enumerate() {
                i += items_per_page * current_page;
                if i >= item_count {
                    break;
                }

                let is_selected = selected_item == i
                    && self.state.selection.selected() == Some(&VpnSelection::Files);
                self.render_wg_item(meta, *area, buf, is_selected, i, theme);
            }
        }
    }
}

impl VpnMenu<'_> {
    fn render_main_block(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
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
        main_block.render(area, buf);
    }

    fn render_option_menu(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let options_layout = Layout::horizontal([Constraint::Percentage(95)])
            .flex(Flex::Center)
            .split(area);

        let is_block_selected = self.state.selection.selected() != Some(&VpnSelection::Files);

        let opt_block = Block::new()
            .borders(Borders::ALL)
            .border_style(if is_block_selected {
                Style::new().fg(theme.accent.color())
            } else {
                Style::new().fg(theme.muted.color())
            });

        let opt_inner = opt_block.inner(options_layout[0]);

        opt_block.render(options_layout[0], buf);

        // TODO: find better way to do this
        let layouts = if self.wg_on {
            vec![
                Constraint::Min(0),
                Constraint::Length(10),
                Constraint::Length(4),
                Constraint::Length(13),
                Constraint::Length(6),
                Constraint::Min(0),
            ]
        } else {
            vec![
                Constraint::Min(0),
                Constraint::Length(5),
                Constraint::Length(4),
                Constraint::Length(13),
                Constraint::Length(6),
                Constraint::Min(0),
            ]
        };

        let btn_areas = Layout::horizontal(layouts)
            .flex(Flex::Center)
            .spacing(4)
            .split(opt_inner);

        let mut btn_styles = [theme.muted.color(); 4];

        // order disconnect/start -> scan -> countries -> filter
        if self.state.selection.selected() != Some(&VpnSelection::Files) {
            btn_styles[self.state.selection.selected_index] = theme.info.color();
        }

        if self.wg_on {
            Line::from("Disconnect")
                .style(btn_styles[0])
                .render(btn_areas[1], buf);
        } else {
            Line::from("Start")
                .style(btn_styles[0])
                .render(btn_areas[1], buf);
        }
        Line::from("Scan")
            .style(btn_styles[1])
            .render(btn_areas[2], buf);
        Line::from("Get Countries")
            .style(btn_styles[2])
            .render(btn_areas[3], buf);
        Line::from("Filter")
            .style(btn_styles[3])
            .render(btn_areas[4], buf);
    }

    // To reduce the amount of times this is performed this needs to be run
    // before the ui element is initialised
    pub fn build_layout_no_render(area: Rect, cols: &mut usize) -> VpnAreas {
        let (outer, select_area, vpn_area) = panel_with_toolbar(area, 80, 90);

        let max_cols = calc_max_cols(vpn_area).unwrap_or(0);
        *cols = max_cols;

        VpnAreas {
            outer,
            vpn: vpn_area,
            select: select_area,
        }
    }

    fn render_wg_item(
        &self,
        meta: &[WgMeta],
        area: Rect,
        buf: &mut Buffer,
        is_selected: bool,
        i: usize,
        theme: &Theme,
    ) {
        let (border_style, text_style) = selection_style(theme, is_selected);
        if let Some(name) = self.names.get(i) {
            let block = Block::new()
                .borders(Borders::ALL)
                .border_style(border_style)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .style(text_style)
                .title_top(Line::from(format!(" {name} ")).left_aligned());

            let meta = &meta[i];
            // let mod_area = block.inner(area);

            if meta.country == [0, 0] {
                block.render(area, buf);
            } else {
                let block = block.title_bottom(
                    Line::from(format!(
                        " [{}{}] ",
                        char::from(meta.country[0]),
                        char::from(meta.country[1])
                    ))
                    .right_aligned(),
                );
                block.render(area, buf);
            }
        }
    }
}

pub fn calc_max_cols(area: Rect) -> Option<usize> {
    let mut max_cols = 0;
    // from max col to min until
    // first which provide enough pixels
    for i in (COLS.0..=COLS.1).rev() {
        if area.width / (i as u16) > COLS.2 {
            max_cols = i;
            break;
        }
    }
    if max_cols > 0 { Some(max_cols) } else { None }
}

pub struct VpnAreas {
    outer: Rect,
    vpn: Rect,
    select: Rect,
}
