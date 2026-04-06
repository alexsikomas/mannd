use mannd::{config::ThemePalette, context, error::ManndError};
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders},
};
use std::{path::PathBuf, sync::OnceLock};
use tracing::instrument;

use crate::{
    components::{
        main_menu::MainMenu, networkd_ui::NetworkdMenu, password_prompt::PasswordPrompt,
        popup_prompt::PopupPrompt, wifi_menu::Connection, wireguard_ui::VpnMenu,
        wpa_interface_ui::WpaInterfaceUi,
    },
    state::{AppContext, PromptState, UiState, View},
};

/// Theme global state, used to bypass needing to
/// send theme data to functions that require instead
/// if a function needs it they can read it
pub static THEME: OnceLock<Theme> = OnceLock::new();

pub fn theme() -> &'static Theme {
    THEME
        .get()
        .expect("Theme must be initialised before rendering")
}

impl Theme {
    /// Reads config toml from a predefined location and sets the
    /// global value of `THEME`
    #[instrument(err)]
    pub fn new() -> Result<(), ManndError> {
        let config = &context().settings;
        let selected = &config.theme.selected;

        let palette = config
            .theme
            .palettes
            .get(selected.as_str())
            .ok_or_else(|| ManndError::SectionNotFound(selected.clone()))?;

        let theme = Theme::try_from(palette)?;

        if THEME.set(theme).is_err() {
            tracing::info!("Theme has already been initialised");
        }
        Ok(())
    }
}

impl TryFrom<&ThemePalette> for Theme {
    type Error = ManndError;
    fn try_from(value: &ThemePalette) -> Result<Self, Self::Error> {
        Ok(Theme {
            background: ThemeRgb::try_from(value.background.as_str())?,
            foreground: ThemeRgb::try_from(value.foreground.as_str())?,
            muted: ThemeRgb::try_from(value.muted.as_str())?,
            error: ThemeRgb::try_from(value.error.as_str())?,
            warning: ThemeRgb::try_from(value.warning.as_str())?,
            success: ThemeRgb::try_from(value.success.as_str())?,
            info: ThemeRgb::try_from(value.info.as_str())?,
            primary: ThemeRgb::try_from(value.primary.as_str())?,
            secondary: ThemeRgb::try_from(value.secondary.as_str())?,
            tertiary: ThemeRgb::try_from(value.tertiary.as_str())?,
            accent: ThemeRgb::try_from(value.accent.as_str())?,
        })
    }
}

pub struct UiContext {}

impl UiContext {
    pub fn render(frame: &mut Frame<'_>, state: &UiState, ctx: &AppContext) {
        let outer_area = frame.area();
        let theme = theme();

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

        match &state.current_view {
            View::MainMenu(list) => {
                if let Some(menu) = MainMenu::new(list) {
                    frame.render_widget(menu, inner_area);
                } else {
                    return;
                }
            }
            View::Wifi(connection_state) => {
                if let Some(con) = Connection::new(&net_ctx.networks, connection_state) {
                    frame.render_widget(con, inner_area);
                }
            }
            View::Vpn(vpn_state) => {
                // unnecessary
                let mut cols: usize = 0;
                let vpn_areas = VpnMenu::build_layout_no_render(inner_area, &mut cols);

                let wg_meta = if net_ctx.wg_ctx.active {
                    Some(net_ctx.wg_ctx.meta.as_slice())
                } else {
                    None
                };

                let vpn_menu = VpnMenu::new(
                    vpn_state,
                    &net_ctx.wg_ctx.names,
                    wg_meta,
                    ctx.net_ctx.wg_ctx.active,
                    vpn_areas,
                );
                frame.render_widget(vpn_menu, inner_area);
            }
            View::Networkd(state) => {
                // TODO: Unfinished
                let tmp: Vec<PathBuf> = vec![];
                frame.render_widget(NetworkdMenu::new(state, &tmp), inner_area);
            }
            View::Config => {
                // TODO: Config UI not implemented
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
                    if let Some(wpa_ifaces) = &net_ctx.wpa_interfaces
                        && let Some(prompt_instance) = WpaInterfaceUi::new(wpa_prompt, wpa_ifaces)
                    {
                        frame.render_widget(prompt_instance, inner_area);
                    }
                }
                PromptState::PskConnect(psk_prompt) => {
                    if let Some(prompt_instance) =
                        PasswordPrompt::new(&psk_prompt.network, psk_prompt)
                    {
                        frame.render_widget(prompt_instance, inner_area);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ThemeRgb {
    red: u8,
    green: u8,
    blue: u8,
}

#[derive(Debug)]
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
impl TryFrom<&str> for ThemeRgb {
    type Error = ManndError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(ThemeRgb {
            red: u8::from_str_radix(&value[1..=2], 16)?,
            green: u8::from_str_radix(&value[3..=4], 16)?,
            blue: u8::from_str_radix(&value[5..=6], 16)?,
        })
    }
}

impl ThemeRgb {
    /// Tinting and shading alogrithm
    pub fn shift(&self, percent: i8) -> Color {
        let percent = f32::from(percent).clamp(-100.0, 100.0) / 100.0;

        let col: [u8; 3] = [self.red, self.green, self.blue];
        let mut new_col = [0u8; 3];

        for (i, c) in col.iter().enumerate() {
            let res: u8;
            if percent > 0.0 {
                res = (f32::from(*c) + (255.0 - f32::from(*c)) * percent).round() as u8;
            } else {
                res = (f32::from(*c) * (1.0 + percent)).round() as u8;
            }
            new_col[i] = res;
        }

        Color::Rgb(new_col[0], new_col[1], new_col[2])
    }

    pub fn color(&self) -> Color {
        let col: Color = self.into();
        col
    }
}
