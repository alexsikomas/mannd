use mannd::{
    CONFIG_HOME, SETTINGS, error::ManndError, ini_parse::IniConfig, state::network::InterfaceTypes,
};
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders},
};
use serde::{Deserialize, de::IntoDeserializer};
use std::{borrow::Cow, path::PathBuf, sync::OnceLock};
use tracing::{info, instrument};

use crate::{
    components::{
        main_menu::MainMenu, networkd_ui::NetdMenu, password_prompt::PasswordPrompt,
        popup_prompt::PopupPrompt, wifi_menu::Connection, wireguard_ui::WireguardMenu,
        wpa_interface_ui::WpaInterfaceUi,
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
    #[instrument(err)]
    pub fn new() -> Result<(), ManndError> {
        let conf = &SETTINGS;

        let theme = conf
            .sections
            .get("theme")
            .ok_or_else(|| ManndError::SectionNotFound("theme".to_string()))?;

        let selected_theme = theme
            .get("selected")
            .ok_or_else(|| ManndError::PropertyNotFound("selected".to_string()))?;

        let theme_name = format!("theme.{}", selected_theme);

        let selected_theme_section = conf
            .sections
            .get(theme_name.as_str())
            .ok_or_else(|| ManndError::SectionNotFound(theme_name))?;

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

    /// Renders title, border, and conditionally
    /// renders main content depending on state
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

        let net_ctx = ctx.net_ctx;

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

        // Prefer this when stabilised:
        // let widget: impl Widget;
        // frame.render_widget(widget, inner_area);

        match &state.current_view {
            View::MainMenu(list) => {
                if let Some(menu) = MainMenu::new(&list) {
                    frame.render_widget(menu, inner_area);
                } else {
                    return;
                }
            }
            View::Wifi(connection_state) => {
                if let Some(con) = Connection::new(&net_ctx.networks, &connection_state) {
                    frame.render_widget(con, inner_area);
                }
                for prompt in &state.prompt_stack {
                    match prompt {
                        PromptState::PskConnect(psk_prompt) => {
                            let Some(selected) =
                                net_ctx.networks.get(connection_state.network_cursor)
                            else {
                                return;
                            };

                            if let Some(prompt_instance) =
                                PasswordPrompt::new(selected, &psk_prompt)
                            {
                                frame.render_widget(prompt_instance, inner_area);
                            }
                        }
                        _ => {}
                    };
                }
            }
            View::Vpn(vpn_state) => {
                let mut cols: usize = 0;
                let vpn_areas = WireguardMenu::build_layout_no_render(inner_area, &mut cols);
                let wg_meta = if ctx.net_ctx.wg_ctx.is_on {
                    Some(&net_ctx.wg_ctx.meta)
                } else {
                    None
                };

                if let Some(vpn) = WireguardMenu::new(
                    &vpn_state,
                    &net_ctx.wg_ctx.names,
                    wg_meta,
                    ctx.net_ctx.wg_ctx.is_on,
                    vpn_areas,
                ) {
                    if cols != state.vpn_cols {
                        self.message = Some(UiMessage::SetVpnCols(cols));
                    }
                    frame.render_widget(vpn, inner_area);
                }
            }
            View::Networkd(netd_state) => {
                let tmp: Vec<PathBuf> = vec![];
                NetdMenu::new(&netd_state, &tmp);
            }
            _ => {
                return;
            }
        }

        for prompt in &state.prompt_stack {
            match prompt {
                PromptState::Info(info_prompt) => {
                    if let Some(prompt_instance) =
                        PopupPrompt::new(&info_prompt.reason, &info_prompt.kind)
                    {
                        frame.render_widget(prompt_instance, inner_area);
                    }
                }
                PromptState::WpaInterface(wpa_prompt) => {
                    if let Some(InterfaceTypes::Wpa(wpa_ifaces)) = &net_ctx.interfaces {
                        if let Some(prompt_instance) = WpaInterfaceUi::new(
                            wpa_prompt,
                            ctx.net_ctx.persist_wpa_changes,
                            &wpa_ifaces,
                        ) {
                            frame.render_widget(prompt_instance, inner_area);
                        }
                    }
                }
                _ => {}
            };
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
