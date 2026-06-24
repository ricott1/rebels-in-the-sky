use super::{
    constants::UiStyle,
    traits::InteractiveWidget,
    ui_callback::{CallbackRegistry, UiCallback},
    widgets::default_block,
};
use ratatui::crossterm;
use ratatui::crossterm::event::KeyCode;
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Style, Styled},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Widget},
};

#[derive(Debug, Default, Clone)]
pub struct Checkbox<'a> {
    text: Text<'a>,
    state: bool,
    hotkey: Option<KeyCode>,
    on_click: UiCallback,
    disabled: bool,
    is_hovered: bool,
    disabled_text: Option<Text<'a>>,
    text_alignemnt: ratatui::layout::Alignment,
    style: Style,
    hover_style: Style,
    block: Option<Block<'a>>,
    hover_block: Option<Block<'a>>,
    hover_text: Option<Text<'a>>,
    layer: usize,
}

impl<'a> Checkbox<'a> {
    pub fn new(text: impl Into<Text<'a>>, on_click: UiCallback, initial_state: bool) -> Self {
        Self {
            text: text.into(),
            state: initial_state,
            on_click,
            text_alignemnt: ratatui::layout::Alignment::Center,
            hover_style: UiStyle::HIGHLIGHT,
            block: Some(default_block()),
            hover_block: Some(default_block()),
            ..Default::default()
        }
    }

    pub fn set_hover_text(mut self, text: impl Into<Text<'a>>) -> Self {
        self.hover_text = Some(text.into());
        self
    }

    pub const fn set_hotkey(mut self, k: KeyCode) -> Self {
        self.hotkey = Some(k);
        self
    }
}

impl<'a> Styled for Checkbox<'a> {
    type Item = Checkbox<'a>;

    fn style(&self) -> Style {
        self.style
    }
    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        Self {
            style: style.into(),
            ..self
        }
    }
}

impl<'a> Widget for Checkbox<'a> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let inner = if area.height >= 3 {
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            })
        } else {
            area
        };

        // Label is the first text line, with the hotkey character underlined.
        let mut spans: Vec<Span> = match self.text.lines.first() {
            Some(first_line) => {
                if let Some(u) = self.hotkey {
                    let first_line_string = first_line.to_string();
                    if let Some((before, after)) = first_line_string.split_once(&u.to_string()) {
                        vec![
                            Span::raw(before.to_owned()),
                            Span::styled(u.to_string(), UiStyle::DEFAULT.underlined()),
                            Span::raw(after.to_owned()),
                        ]
                    } else {
                        first_line.spans.clone()
                    }
                } else {
                    first_line.spans.clone()
                }
            }
            None => vec![],
        };

        // Switch glyph: a coloured track with a knob on the right when on,
        // on the left when off.
        let track_color = if self.disabled {
            Color::DarkGray
        } else if self.state {
            Color::Green
        } else {
            Color::Rgb(70, 70, 86)
        };
        let knob_color = if self.disabled {
            Color::Gray
        } else {
            Color::White
        };

        let switch = if self.state {
            vec![
                Span::styled("■■", Style::default().fg(track_color)),
                Span::styled("■", Style::default().fg(knob_color)),
            ]
        } else {
            vec![
                Span::styled("■", Style::default().fg(knob_color)),
                Span::styled("■■", Style::default().fg(track_color)),
            ]
        };

        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.extend(switch);

        let paragraph = Paragraph::new(Line::from(spans)).alignment(self.text_alignemnt);

        let paragraph_style = if self.disabled {
            UiStyle::UNSELECTABLE
        } else if self.is_hovered {
            self.hover_style
        } else {
            self.style
        };

        let maybe_block = if self.is_hovered {
            self.hover_block
        } else {
            self.block
        };

        if area.height < 3 {
            paragraph.set_style(paragraph_style).render(area, buf);
        } else if let Some(mut block) = maybe_block {
            block = if self.disabled {
                block
                    .border_style(UiStyle::UNSELECTABLE)
                    .border_set(border::Set::default())
            } else {
                block
            };

            paragraph
                .set_style(paragraph_style)
                .block(block)
                .render(area, buf);
        } else {
            paragraph.set_style(paragraph_style).render(inner, buf);
        }
    }
}

impl InteractiveWidget for Checkbox<'_> {
    fn layer(&self) -> usize {
        self.layer
    }

    fn before_rendering(&mut self, area: Rect, callback_registry: &mut CallbackRegistry) {
        self.is_hovered = callback_registry.is_hovering(area)
            && callback_registry.get_active_layer() == self.layer();

        if !self.disabled {
            if self.is_hovered {
                callback_registry.register_mouse_callback(
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                    Some(area),
                    self.on_click.clone(),
                );
            }

            if let Some(key) = self.hotkey {
                callback_registry.register_keyboard_callback(key, self.on_click.clone());
            }
        }
    }

    fn hover_text(&'_ self) -> Text<'_> {
        let mut spans = vec![];
        if let Some(hover_text) = self.hover_text.as_ref() {
            spans.push(Span::raw(hover_text.to_string()));

            if self.disabled {
                if let Some(disabled_text) = self.disabled_text.as_ref() {
                    spans.push(Span::styled(
                        format!("  Disabled: {disabled_text}"),
                        UiStyle::ERROR,
                    ));
                }
            }
        }
        Line::from(spans).into()
    }
}
