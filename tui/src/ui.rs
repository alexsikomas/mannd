use std::sync::OnceLock;

use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders},
};
use serde::Deserialize;
use toml::Value;
use tracing::info;

use crate::{
    components::{connection::Connection, main_menu::MainMenu, password_prompt::PasswordPrompt},
    state::{AppContext, PromptState, UiState, View},
};

/// Theme global state, used to bypass needing to
/// send theme data to functions that require instead
/// if a function needs it they can read it
pub static THEME: OnceLock<Theme> = OnceLock::new();

impl Theme {
    /// Reads config toml from a predefined location and sets the
    /// global value of `THEME`
    pub fn new() -> Result<(), Box<dyn std::error::Error>> {
        // temporary, will be changed to more standard .config location
        // as well as command-line option
        let config = std::fs::read_to_string("tui/example_config.toml")?;
        let toml_value: Value = toml::from_str(&config)?;
        let selected_theme = toml_value["theme"]["selected"].as_str().ok_or_else(|| {
            return std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Selected value for theme not found.",
            );
        })?;

        info!("Selected Theme: {selected_theme}");

        let theme_table = toml_value["theme"][selected_theme]
            .as_table()
            .ok_or_else(|| {
                return std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Could not find/parse the selected theme table",
                );
            })?;

        let theme: Theme = theme_table.clone().try_into()?;
        match THEME.set(theme) {
            Ok(_) => Ok(()),
            Err(_) => {
                tracing::info!("Theme has already been initalised.");
                Ok(())
            }
        }
    }
}

/// Renders title, border and conditionally renders main content depending on
/// state
pub fn render<'a>(frame: &mut Frame<'a>, state: &UiState, ctx: &AppContext) {
    let outer_area = frame.area();
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

    // will do this instead when rust stablises it
    // let widget: impl Widget;
    // frame.render_widget(widget, inner_area);

    // we give the widget only the necessary selections to
    // render
    match &state.current_view {
        View::MainMenu(list) => {
            let menu = MainMenu::new(&list);
            frame.render_widget(menu, inner_area);
        }
        View::Connection(connection_state) => {
            if let Some(con) = Connection::new(ctx.networks, &connection_state) {
                frame.render_widget(con, inner_area);
            }

            for prompt in &state.prompt_stack {
                match prompt {
                    PromptState::PskConnect(psk_prompt) => {
                        let Some(selected) = ctx.networks.get(connection_state.network_cursor)
                        else {
                            return;
                        };

                        if let Some(prompt_instance) = PasswordPrompt::new(selected, &psk_prompt) {
                            frame.render_widget(prompt_instance, inner_area);
                        }
                    }
                }
            }
        }
        _ => {
            return;
        }
    }
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
    /// Turns `&ThemeColor` into a vector of `u8`, if any induvidual
    /// part of the conversion encounters an error it will return 0
    /// for that part.
    fn into(self) -> Vec<u8> {
        // ignore # in theme color
        vec![
            u8::from_str_radix(&self.0[1..=2], 16).unwrap_or(0),
            u8::from_str_radix(&self.0[3..=4], 16).unwrap_or(0),
            u8::from_str_radix(&self.0[5..=6], 16).unwrap_or(0),
        ]
    }
}

impl ThemeColor {
    /// Tinting and shading alogrithm
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
