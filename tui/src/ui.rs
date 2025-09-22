use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Styled, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

struct UiState {
    pub theme: Box<dyn ColorScheme>,
}

pub fn ui(frame: &mut Frame) {
    let state = UiState {
        theme: Box::new(Theme::Dark),
    };

    let outer_area = frame.size();

    let title_block = Block::new()
        .borders(Borders::all())
        .style(state.theme.foreground())
        .title(
            Line::from(" mannd ")
                .style(
                    Style::new()
                        .fg(state.theme.focused())
                        .add_modifier(Modifier::BOLD),
                )
                .centered(),
        );

    let inner_area = title_block.inner(outer_area);
    frame.render_widget(title_block, outer_area);
    frame.render_widget(
        Block::new().style(
            Style::new()
                .fg(state.theme.active())
                .bg(state.theme.active()),
        ),
        inner_area,
    );
}

fn main_menu(frame: &mut Frame) {}

fn calculate_layout(area: Rect) -> (Rect, Rect) {
    let outer_area = area;

    let inner_layout = Layout::default()
        .constraints([Constraint::Percentage(100)])
        .margin(1);
    let inner_area = inner_layout.split(outer_area)[0];

    (outer_area, inner_area)
}

enum Theme {
    Light,
    Dark,
}

pub trait ColorScheme {
    fn background(&self) -> Color;
    fn foreground(&self) -> Color;

    fn primary_accent(&self) -> Color;
    fn secondary_accent(&self) -> Color;

    fn active(&self) -> Color;
    fn inactive(&self) -> Color;
    fn focused(&self) -> Color;
    fn unfocused(&self) -> Color;

    fn success(&self) -> Color;
    fn warning(&self) -> Color;
    fn error(&self) -> Color;
}

impl ColorScheme for Theme {
    fn background(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(40, 40, 40),
        }
    }

    fn foreground(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(235, 219, 178),
        }
    }

    fn primary_accent(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(60, 56, 54),
        }
    }

    fn secondary_accent(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(80, 73, 69),
        }
    }

    fn active(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(131, 165, 152),
        }
    }

    fn inactive(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(146, 131, 116),
        }
    }

    fn focused(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(142, 192, 124),
        }
    }

    fn unfocused(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(124, 111, 100),
        }
    }

    fn success(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(184, 187, 38),
        }
    }

    fn warning(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(250, 189, 47),
        }
    }

    fn error(&self) -> Color {
        match self {
            Theme::Light => Color::Rgb(0, 0, 0),
            Theme::Dark => Color::Rgb(251, 73, 52),
        }
    }
}
