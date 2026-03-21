use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::Style,
    widgets::{Block, BorderType, Borders},
};

use crate::ui::Theme;

pub fn centered_rect(area: Rect, h_ptc: u16, v_pct: u16) -> Rect {
    Layout::horizontal([Constraint::Percentage(h_ptc)])
        .flex(Flex::Center)
        .split(
            Layout::vertical([Constraint::Percentage(v_pct)])
                .flex(Flex::Center)
                .areas::<1>(area)[0],
        )[0]
}

pub fn centered_overlay(area: Rect, h_ptc: u16, v_pct: u16) -> (Rect, Rect) {
    let outer = centered_rect(area, h_ptc, v_pct);
    let border_block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = border_block.inner(outer);
    (outer, inner)
}

pub fn panel_with_toolbar(area: Rect, h_ptc: u16, v_pct: u16) -> (Rect, Rect, Rect) {
    let outer = centered_rect(area, h_ptc, v_pct);

    let inner = Rect {
        x: outer.x + 1,
        y: outer.y + 1,
        width: outer.width.saturating_sub(2),
        height: outer.height.saturating_sub(1),
    };

    let [select_area, content_vert] =
        Layout::vertical([Constraint::Max(3), Constraint::Fill(1)]).areas::<2>(inner);

    let content_area = Layout::horizontal([Constraint::Percentage(90)])
        .flex(Flex::Center)
        .split(content_vert)[0];

    (outer, select_area, content_area)
}

pub fn selection_style(theme: &Theme, is_selected: bool) -> (Style, Style) {
    if is_selected {
        (
            Style::new().fg(theme.accent.color()),
            Style::new().fg(theme.info.color()),
        )
    } else {
        (
            Style::new().fg(theme.muted.color()),
            Style::new().fg(theme.muted.color()),
        )
    }
}

pub fn get_inner_area(area: Rect) -> Rect {
    Block::new()
        .border_type(BorderType::Rounded)
        .borders(Borders::all())
        .inner(area)
}
