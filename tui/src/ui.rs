use com::{error::ManndError, ini_parse::IniConfig};
use futures::executor::block_on;
use ratatui::{
    CompletedFrame, Frame, Terminal,
    prelude::{Backend, CrosstermBackend},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Widget},
};
use serde::{
    Deserialize,
    de::{IntoDeserializer, value::MapDeserializer},
};
use std::{
    borrow::Cow,
    collections::HashMap,
    env,
    fmt::format,
    io::{self, Stdout, Write},
    path::PathBuf,
    sync::OnceLock,
};
use tokio::sync::mpsc;
use tracing::info;

use crate::{
    components::{
        connection::Connection, main_menu::MainMenu, password_prompt::PasswordPrompt,
        popup_prompt::PopupPrompt, wireguard_ui::WireguardMenu,
    },
    state::{AppContext, PromptState, UiState, View},
};

/// Theme global state, used to bypass needing to
/// send theme data to functions that require instead
/// if a function needs it they can read it
pub static THEME: OnceLock<Theme> = OnceLock::new();

impl Theme {
    /// Reads config toml from a predefined location and sets the
    /// global value of `THEME`
    #[inline(never)]
    pub fn new() -> Result<(), ManndError> {
        let mut path = match env::var("XDG_CONFIG_HOME") {
            Ok(val) => PathBuf::from(val),
            Err(_) => {
                let home = env::var_os("HOME");
                match home {
                    Some(val) => {
                        let mut path = PathBuf::from(val);
                        path.push(".config");
                        path
                    }
                    None => {
                        panic!("Cannot find $HOME directory!");
                    }
                }
            }
        };

        path.push("mannd/settings.conf");

        let file_str = std::fs::read_to_string(path)?;
        let lines = file_str.lines();
        let mut config = IniConfig::new();
        config.parse_file(lines);

        let theme = config
            .sections
            .get("theme")
            .ok_or_else(|| ManndError::ConfigSectionNotFound("theme".to_string()))?;

        info!("THEME: {:?}", theme);
        let selected_theme = theme
            .get("selected")
            .ok_or_else(|| ManndError::ConfigPropertyNotFound("selected".to_string()))?;

        info!("THEME: {selected_theme}");
        let theme_name = format!("theme.{}", selected_theme);
        info!("THEME: {theme_name}");

        let selected_theme_section = config
            .sections
            .get(theme_name.as_str())
            .ok_or_else(|| ManndError::ConfigSectionNotFound(theme_name))?;

        let hash = IntoDeserializer::<serde::de::value::Error>::into_deserializer(
            selected_theme_section.clone(),
        );
        let theme = Theme::deserialize(hash)?;

        match THEME.set(theme) {
            Ok(_) => Ok(()),
            Err(_) => {
                tracing::info!("Theme has already been initalised.");
                Ok(())
            }
        }
    }
}

pub struct UiContext {
    pub message: Option<UiMessage>,
}

impl UiContext {
    pub fn new() -> Self {
        Self { message: None }
    }

    /// Renders title, border and conditionally renders main content depending on
    /// state
    pub fn render<'a>(&mut self, frame: &mut Frame<'a>, state: &UiState, ctx: &AppContext) {
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
                if let Some(menu) = MainMenu::new(&list) {
                    frame.render_widget(menu, inner_area);
                } else {
                    return;
                }
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

                            if let Some(prompt_instance) =
                                PasswordPrompt::new(selected, &psk_prompt)
                            {
                                frame.render_widget(prompt_instance, inner_area);
                            }
                        }
                        PromptState::Info(info_prompt) => {
                            if let Some(prompt_instance) =
                                PopupPrompt::new(&info_prompt.reason, &info_prompt.kind)
                            {
                                frame.render_widget(prompt_instance, inner_area);
                            }
                        }
                    }
                }
            }
            View::Vpn(vpn_state) => {
                let mut cols: usize = 0;
                let vpn_areas = WireguardMenu::build_layout_no_render(inner_area, &mut cols);
                if let Some(vpn) =
                    WireguardMenu::new(&vpn_state, &ctx.wg_files.0, ctx.wg_files.1, vpn_areas)
                {
                    if cols != state.vpn_cols {
                        self.message = Some(UiMessage::SetVpnCols(cols));
                    }
                    frame.render_widget(vpn, inner_area);
                }
            }
            _ => {
                return;
            }
        }
    }
}

pub enum UiMessage {
    SetVpnCols(usize),
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "Cow<'_, str>")]
pub struct ThemeRgb {
    red: u8,
    green: u8,
    blue: u8,
}

#[derive(Debug, Deserialize)]
pub struct Theme {
    pub background: ThemeRgb,
    pub foreground: ThemeRgb,
    pub muted: ThemeRgb,
    pub error: ThemeRgb,
    pub warning: ThemeRgb,
    pub success: ThemeRgb,
    pub info: ThemeRgb,
    pub primary: ThemeRgb,
    pub secondary: ThemeRgb,
    pub tertiary: ThemeRgb,
    pub accent: ThemeRgb,
}

impl<'a> From<&'a ThemeRgb> for Color {
    fn from(col: &'a ThemeRgb) -> Self {
        Color::Rgb(col.red, col.green, col.blue)
    }
}

/// Turns `&ThemeColor` into a vector of `u8`, if any induvidual
/// part of the conversion encounters an error it will return 0
/// for that part.
// ignore # in theme color
impl<'a> TryFrom<Cow<'_, str>> for ThemeRgb {
    type Error = ManndError;

    fn try_from(value: Cow<'_, str>) -> Result<Self, Self::Error> {
        Ok(ThemeRgb {
            red: u8::from_str_radix(&value[1..=2], 16).map_err(|e| {
                return ManndError::ParseInt(e);
            })?,
            green: u8::from_str_radix(&value[3..=4], 16).map_err(|e| {
                return ManndError::ParseInt(e);
            })?,
            blue: u8::from_str_radix(&value[5..=6], 16).map_err(|e| {
                return ManndError::ParseInt(e);
            })?,
        })
    }
}

impl ThemeRgb {
    /// Tinting and shading alogrithm
    pub fn shift(&self, percent: i8) -> Color {
        let percent = (percent as f32).clamp(-100.0, 100.0) / 100.0;

        let col: [u8; 3] = [self.red, self.green, self.blue];
        let mut new_col = [0u8; 3];

        for (i, c) in col.iter().enumerate() {
            let res: u8;
            if percent > 0.0 {
                res = (*c as f32 + (255.0 - *c as f32) * percent).round() as u8;
            } else {
                res = (*c as f32 * (1.0 + percent)).round() as u8;
            }
            new_col[i] = res;
        }

        Color::Rgb(new_col[0], new_col[1], new_col[2])
    }

    pub fn color(&self) -> Color {
        let col: Color = self.into();
        return col;
    }
}
