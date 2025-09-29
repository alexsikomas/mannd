use std::{error::Error, sync::OnceLock};

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

use crate::components::menu::Menu;

pub static THEME: OnceLock<Theme> = OnceLock::new();

impl Theme {
    #[instrument]
    pub fn new() -> Result<(), Box<dyn std::error::Error>> {
        let config = std::fs::read_to_string("tui/example_config.toml")?;
        let toml_value: Value = toml::from_str(&config)?;
        let selected_theme = toml_value["theme"]["selected"].as_str().unwrap();

        info!("Selected Theme: {selected_theme}");

        let theme_table = toml_value["theme"][selected_theme].as_table().unwrap();

        let theme: Theme = theme_table.clone().try_into()?;
        match THEME.set(theme) {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::info!("Theme has already been initalised.");
                Ok(())
            }
        }
    }
}

pub fn ui<'a>(frame: &mut Frame<'a>) {
    let outer_area = frame.size();
    let theme: &Theme;
    match THEME.get() {
        Some(t) => {
            theme = t;
        }
        None => {
            return;
        }
    }

    let title_block = Block::new()
        .borders(Borders::all())
        .style(
            Style::new()
                .fg((&theme.foreground).into())
                .bg((&theme.background).into()),
        )
        .title(
            Line::from(" mannd ")
                .style(
                    Style::new()
                        .fg((&theme.primary).into())
                        .add_modifier(Modifier::BOLD),
                )
                .centered(),
        );

    let inner_area = title_block.inner(outer_area);
    frame.render_widget(title_block, outer_area);
    frame.render_widget(
        Block::new().style(
            Style::new()
                .fg((&theme.background).into())
                .bg((&theme.background).into()),
        ),
        inner_area,
    );

    let menu = Menu::default();
    frame.render_widget(menu, inner_area);
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
pub struct Theme {
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

impl<'a> From<&'a ThemeColor> for Color {
    fn from(theme_color: &'a ThemeColor) -> Self {
        // ignore # in theme color
        Color::Rgb(
            u8::from_str_radix(&theme_color.0[1..=2], 16).unwrap(),
            u8::from_str_radix(&theme_color.0[3..=4], 16).unwrap(),
            u8::from_str_radix(&theme_color.0[5..=6], 16).unwrap(),
        )
    }
}
