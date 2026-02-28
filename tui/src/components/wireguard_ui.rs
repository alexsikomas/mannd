use com::wireguard::store::WgMeta;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Margin, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{self, Block, Borders, Widget},
};

use crate::{
    state::{VpnSelection, VpnState},
    ui::{Theme, THEME},
};

// min num of cols, max num of cols, target line amount
const COLS: (usize, usize, u16) = (2, 6, 30);
// target line amount
const ROW_H: u16 = 6;

pub struct WireguardMenu<'a> {
    state: &'a VpnState,
    names: &'a Vec<String>,
    meta: Option<&'a Vec<WgMeta>>,
    wg_on: bool,
    theme: &'a Theme,
    areas: VpnAreas,
}

impl<'a> WireguardMenu<'a> {
    pub fn new(
        state: &'a VpnState,
        names: &'a Vec<String>,
        meta: Option<&'a Vec<WgMeta>>,
        wg_on: bool,
        areas: VpnAreas,
    ) -> Option<Self> {
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
            wg_on,
            theme,
            areas,
        })
    }
}

impl<'a> Widget for WireguardMenu<'a> {
    fn render(self, _area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        self.render_main_block(self.areas.outer, buf);
        self.render_option_menu(self.areas.select, buf);

        match self.meta {
            Some(meta) => {
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

                let cols = match calc_max_cols(self.areas.vpn) {
                    Some(c) => c,
                    None => {
                        tracing::error!("Not enough room to display a single column...");
                        return;
                    }
                };

                let items_per_page = rows * cols;
                let selected_item = self.state.file_cursor;
                let current_page = selected_item / items_per_page;

                let mut item_areas: Vec<Rect> = vec![];
                let rows_layout =
                    Layout::vertical(vec![Constraint::Percentage(97 / (rows as u16)); rows])
                        .split(self.areas.vpn);

                for row in rows_layout.into_iter() {
                    let cols =
                        Layout::horizontal(vec![Constraint::Percentage(100 / (cols as u16)); cols])
                            .flex(Flex::Center)
                            .split(*row);

                    item_areas.append(&mut cols.to_vec());
                }

                for (mut i, area) in item_areas.iter().enumerate() {
                    i += items_per_page * current_page;
                    if i > item_count {
                        break;
                    }

                    let is_selected = selected_item == i
                        && self.state.selection.selected() == Some(&VpnSelection::Files);
                    self.render_wg_item(&meta, *area, buf, is_selected, i);
                }
            }
            None => {}
        }
    }
}

impl<'a> WireguardMenu<'a> {
    fn render_main_block(&self, area: Rect, buf: &mut Buffer) {
        let main_block = Block::new()
            .border_type(ratatui::widgets::BorderType::Rounded)
            .borders(Borders::ALL)
            .style(self.theme.primary.color())
            .title_top(
                Line::from(" WireGuard ")
                    .centered()
                    .style(self.theme.accent.color())
                    .bold(),
            );
        main_block.render(area, buf);
    }

    fn render_option_menu(&self, area: Rect, buf: &mut Buffer) {
        let options_layout = Layout::horizontal([Constraint::Percentage(95)])
            .flex(Flex::Center)
            .split(area);

        let is_block_selected = self.state.selection.selected() != Some(&VpnSelection::Files);

        let opt_block = Block::new()
            .borders(Borders::ALL)
            .border_style(if is_block_selected {
                Style::new().fg(self.theme.accent.color())
            } else {
                Style::new().fg(self.theme.muted.color())
            });

        let opt_inner = opt_block.inner(options_layout[0]);

        opt_block.render(options_layout[0], buf);

        // TODO: find better way to do this
        let layouts = if !self.wg_on {
            vec![
                Constraint::Min(0),
                Constraint::Length(5),
                Constraint::Length(4),
                Constraint::Length(13),
                Constraint::Length(6),
                Constraint::Min(0),
            ]
        } else {
            vec![
                Constraint::Min(0),
                Constraint::Length(10),
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

        let mut btn_styles = [self.theme.muted.color(); 4];

        // order disconnect/start -> scan -> countries -> filter
        if self.state.selection.selected() != Some(&VpnSelection::Files) {
            btn_styles[self.state.selection.selected_index] = self.theme.info.color();
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

    fn alter_area_bounds(area: &mut Rect) {
        area.y += 1;
        area.height -= 1;
        area.x += 1;
        area.width -= 2;
    }

    // To reduce the amount of times this is performed this needs to be run
    // before the ui element is initialised
    pub fn build_layout_no_render(area: Rect, cols: &mut usize) -> VpnAreas {
        let mut outer_area = Layout::horizontal([Constraint::Percentage(80)])
            .flex(Flex::Center)
            .split(
                Layout::vertical([Constraint::Percentage(90)])
                    .flex(Flex::Center)
                    .areas::<1>(area)[0],
            )[0];

        let original_outer_area = outer_area.clone();
        Self::alter_area_bounds(&mut outer_area);

        let [select_area, vpn_vert_area] =
            Layout::vertical([Constraint::Max(3), Constraint::Fill(1)]).areas::<2>(outer_area);

        let vpn_area = Layout::horizontal([Constraint::Percentage(90)])
            .flex(Flex::Center)
            .split(vpn_vert_area)[0];

        let max_cols = match calc_max_cols(vpn_area) {
            Some(c) => c,
            None => {
                tracing::error!("Not enough room to display a single column...");
                0
            }
        };

        *cols = max_cols;
        VpnAreas {
            outer: original_outer_area,
            vpn: vpn_area,
            select: select_area,
        }
    }

    fn render_wg_item(
        &self,
        meta: &Vec<WgMeta>,
        area: Rect,
        buf: &mut Buffer,
        is_selected: bool,
        i: usize,
    ) {
        let (border_style, text_style) = self.get_style(is_selected);
        match self.names.get(i) {
            Some(name) => {
                let block = Block::new()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .style(text_style)
                    .title_top(Line::from(format!(" {} ", name)).left_aligned());

                let meta = &meta[i];
                let mod_area = block.inner(area);

                if meta.country != [0, 0] {
                    let block = block.title_bottom(
                        Line::from(format!(
                            " [{}{}] ",
                            char::from(meta.country[0]),
                            char::from(meta.country[1])
                        ))
                        .right_aligned(),
                    );
                    block.render(area, buf);
                } else {
                    block.render(area, buf);
                }

                let mod_line = Line::from(format!(" Modified: {}", meta.last_modified));
                let access_area = mod_area.inner(Margin::new(mod_line.width() as u16, 2));
                mod_line.render(mod_area, buf);

                if meta.last_used != 0 {
                    let access_line = Line::from(format!(" Used: {}", meta.last_used));
                    access_line.render(access_area, buf);
                }
            }
            None => {
                return;
            }
        }
    }

    // border, text
    fn get_style(&self, is_selected: bool) -> (Style, Style) {
        if is_selected {
            let border = Style::new().fg(self.theme.accent.color());
            let text = Style::new().fg(self.theme.info.color());
            (border, text)
        } else {
            let border = Style::new().fg(self.theme.muted.color());
            let text = Style::new().fg(self.theme.muted.color());
            (border, text)
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
    if max_cols > 0 {
        Some(max_cols)
    } else {
        None
    }
}

pub struct VpnAreas {
    outer: Rect,
    vpn: Rect,
    select: Rect,
}
