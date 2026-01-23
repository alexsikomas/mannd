use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Offset, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

use crate::{
    state::PopupType,
    ui::{Theme, THEME},
};

// Style that should be displayed for each
// prompt type
struct PromptMeta {
    title: String,
    main_color: Color,
    secondary_color: Color,
    text_color: Color,
}

pub struct PopupPrompt<'a> {
    theme: &'a Theme,
    text: &'a String,
    prompt_type: &'a PopupType,
}

impl<'a> PopupPrompt<'a> {
    pub fn new(text: &'a String, prompt_type: &'a PopupType) -> Option<Self> {
        let theme = match THEME.get() {
            Some(t) => t,
            None => {
                return None;
            }
        };

        Some(Self {
            theme,
            text,
            prompt_type,
        })
    }
}

impl<'a> Widget for PopupPrompt<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let theme = &self.theme;

        let [main_area] = Layout::vertical([Constraint::Percentage(50)])
            .flex(Flex::Center)
            .areas(
                Layout::horizontal([Constraint::Percentage(50)])
                    .flex(Flex::Center)
                    .areas::<1>(area)[0],
            );
        Clear.render(main_area, buf);

        buf.set_style(
            main_area,
            Style::new()
                .fg(theme.background.color())
                .bg(theme.background.color()),
        );

        let prompt_meta = self.prompt_type.to_info(theme);

        let main_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(
                Style::new()
                    .bg(theme.background.color())
                    .fg(prompt_meta.main_color),
            )
            .title_top(
                Line::from(prompt_meta.title)
                    .centered()
                    .style(Style::new().fg(prompt_meta.text_color).bold()),
            );

        let main_inner = main_block.inner(main_area);
        main_block.render(main_area, buf);

        let span = Span::styled(
            self.text,
            Style::new()
                .bg(theme.background.color())
                .fg(theme.foreground.color())
                .bold(),
        )
        .into_centered_line();

        span.render(main_inner.offset(Offset { x: 0, y: 1 }), buf);

        let exit_block = Block::new()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(Style::new().fg(prompt_meta.secondary_color));

        let btn_layout = Layout::vertical([Constraint::Min(0), Constraint::Length(3)])
            .flex(Flex::Center)
            .split(
                Layout::horizontal([Constraint::Ratio(1, 3); 3])
                    .flex(Flex::Center)
                    .split(main_inner)[1],
            );

        let btn_layout = Layout::horizontal([Constraint::Percentage(50)])
            .flex(Flex::Center)
            .spacing(1)
            .split(btn_layout[1]);

        let btn_span = Span::styled("OK", Style::new().fg(prompt_meta.secondary_color).bold())
            .into_centered_line();
        let btn_area = exit_block.inner(btn_layout[0]);

        exit_block.render(btn_layout[0], buf);
        btn_span.render(btn_area, buf);
    }
}

impl PopupType {
    fn to_info(&self, theme: &Theme) -> PromptMeta {
        match self {
            PopupType::General => PromptMeta {
                title: " Info ".to_string(),
                main_color: theme.primary.color(),
                secondary_color: theme.secondary.color(),
                text_color: Color::White,
            },
            PopupType::Error => PromptMeta {
                title: " Error ".to_string(),
                main_color: theme.error.color(),
                secondary_color: theme.secondary.color(),
                text_color: Color::White,
            },
        }
    }
}
