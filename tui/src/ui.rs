use std::{error::Error, sync::OnceLock};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Styled, Stylize},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};
use serde::Deserialize;
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use toml::Value;
use tracing::{info, instrument};

use crate::{
    App, AppMessage, Query,
    components::menu::{self, MainMenu},
};

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

pub fn render<'a>(frame: &mut Frame<'a>, tx: UnboundedSender<AppMessage>) {
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
        .border_type(BorderType::Rounded)
        .borders(Borders::all())
        .style(
            Style::new()
                .fg(theme.foreground.color())
                .bg(theme.background.color()),
        )
        .title(
            Line::from(" mannd ")
                .style(
                    Style::new()
                        .fg(theme.primary.color())
                        .add_modifier(Modifier::BOLD),
                )
                .centered(),
        );

    let inner_area = title_block.inner(outer_area);
    frame.render_widget(title_block, outer_area);
    frame.render_widget(
        Block::new().style(
            Style::new()
                .fg(theme.background.color())
                .bg(theme.background.color()),
        ),
        inner_area,
    );

    // conditional

    // will do this instead when rust stablises it
    // let widget: impl Widget;
    // frame.render_widget(widget, inner_area);
    let (res, recv) = oneshot::channel();

    let _ = tx.send(AppMessage::Query(Query::View { res: res }));
    let view = tokio::task::block_in_place(|| recv.blocking_recv().unwrap());

    match view.selected {
        0 => {
            let menu = MainMenu::new(tx.clone());
            frame.render_widget(menu, inner_area);
        }
        _ => {
            return;
        }
    }
}

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
pub struct ThemeColor(String);

#[derive(Deserialize, Debug)]
pub struct Theme {
    pub background: ThemeColor,
    pub foreground: ThemeColor,
    pub muted: ThemeColor,
    pub error: ThemeColor,
    pub warning: ThemeColor,
    pub success: ThemeColor,
    pub info: ThemeColor,
    pub primary: ThemeColor,
    pub secondary: ThemeColor,
    pub tertiary: ThemeColor,
    pub accent: ThemeColor,
}

impl<'a> From<&'a ThemeColor> for Color {
    fn from(theme_color: &'a ThemeColor) -> Self {
        let col: Vec<u8> = theme_color.into();
        Color::Rgb(col[0], col[1], col[2])
    }
}

impl Into<Vec<u8>> for &ThemeColor {
    fn into(self) -> Vec<u8> {
        // ignore # in theme color
        vec![
            u8::from_str_radix(&self.0[1..=2], 16).unwrap(),
            u8::from_str_radix(&self.0[3..=4], 16).unwrap(),
            u8::from_str_radix(&self.0[5..=6], 16).unwrap(),
        ]
    }
}

impl ThemeColor {
    pub fn shift(&self, percent: i8) -> Color {
        let percent = (percent as f32).clamp(-100.0, 100.0) / 100.0;
        let col: Vec<u8> = self.into();
        let mut new_col: Vec<u8> = vec![];

        for c in col {
            let res: u8;
            if percent > 0.0 {
                res = (c as f32 + (255.0 - c as f32) * percent).round() as u8;
            } else {
                res = (c as f32 * (1.0 + percent)).round() as u8;
            }
            new_col.push(res);
        }

        Color::Rgb(new_col[0], new_col[1], new_col[2])
    }

    pub fn color(&self) -> Color {
        let col: Color = self.into();
        return col;
    }
}
