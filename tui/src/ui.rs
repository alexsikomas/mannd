use std::error::Error;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Styled, Stylize},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use serde::Deserialize;
use toml::Value;
use tracing::{info, instrument};

pub struct UiState {
    pub theme: Theme,
}

impl UiState {
    #[instrument]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = std::fs::read_to_string("tui/example_config.toml")?;
        let toml_value: Value = toml::from_str(&config)?;
        let selected_theme = toml_value["theme"]["selected"].as_str().unwrap();

        info!("Selected Theme: {selected_theme}");

        let theme_table = toml_value["theme"][selected_theme].as_table().unwrap();

        let theme: Theme = theme_table.clone().try_into()?;
        info!("Theme Table: {:?}", theme);

        Ok(Self { theme })
    }
}

pub fn ui<'a>(frame: &mut Frame<'a>, ui_state: &UiState) {
    let outer_area = frame.size();

    let title_block = Block::new()
        .borders(Borders::all())
        .style(
            Style::new()
                .fg((&ui_state.theme.foreground).into())
                .bg((&ui_state.theme.background).into()),
        )
        .title(
            Line::from(" mannd ")
                .style(
                    Style::new()
                        .fg((&ui_state.theme.background).into())
                        .add_modifier(Modifier::BOLD),
                )
                .centered(),
        );

    let inner_area = title_block.inner(outer_area);
    frame.render_widget(title_block, outer_area);
    frame.render_widget(
        Block::new().style(
            Style::new()
                .fg((&ui_state.theme.background).into())
                .bg((&ui_state.theme.background).into()),
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

#[derive(Deserialize, Debug)]
struct Config {
    selected: String,
}

#[derive(Deserialize, Debug)]
struct ThemeColor(String);

#[derive(Deserialize, Debug)]
struct Theme {
    background: ThemeColor,
    foreground: ThemeColor,
    muted: ThemeColor,
    error: ThemeColor,
    warning: ThemeColor,
    success: ThemeColor,
    info: ThemeColor,
    primary: ThemeColor,
    secondary: ThemeColor,
    tertiary: ThemeColor,
    accent: ThemeColor,
}

impl Into<Color> for &ThemeColor {
    fn into(self) -> Color {
        // ignore # in theme color
        Color::Rgb(
            u8::from_str_radix(&self.0[1..=2], 16).unwrap(),
            u8::from_str_radix(&self.0[3..=4], 16).unwrap(),
            u8::from_str_radix(&self.0[5..=6], 16).unwrap(),
        )
    }
}
