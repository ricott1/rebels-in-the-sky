use super::{
    constants::UiStyle,
    traits::InteractiveWidget,
    ui_callback::{CallbackRegistry, UiCallback},
    widgets::default_block,
};
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Margin, Rect},
    style::{Style, Styled, Stylize},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Widget},
};

#[derive(Debug, Default, Clone)]
pub struct Button<'a> {
    text: Text<'a>,
    hotkey: Option<KeyCode>,
    on_click: UiCallback,
    disabled: bool,
    selected: bool,
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

impl<'a> From<Button<'a>> for Text<'a> {
    fn from(button: Button<'a>) -> Text<'a> {
        button.text
    }
}

impl<'a> Button<'a> {
    pub fn new(text: impl Into<Text<'a>>, on_click: UiCallback) -> Self {
        Self::no_box(text, on_click)
            .hover_block(default_block())
            .block(default_block())
    }

    pub fn box_on_hover(text: impl Into<Text<'a>>, on_click: UiCallback) -> Self {
        Self::no_box(text, on_click).hover_block(default_block())
    }

    pub fn no_box(text: impl Into<Text<'a>>, on_click: UiCallback) -> Self {
        Self {
            text: text.into(),
            on_click,
            text_alignemnt: ratatui::layout::Alignment::Center,
            hover_style: UiStyle::HIGHLIGHT,
            ..Default::default()
        }
    }

    pub fn set_text(&mut self, text: impl Into<Text<'a>>) {
        self.text = text.into();
    }

    pub fn disable(&mut self, text: Option<impl Into<Text<'a>>>) {
        self.disabled = true;
        self.disabled_text = text.map(|t| t.into());
    }

    pub fn disabled(mut self, text: Option<impl Into<Text<'a>>>) -> Self {
        self.disable(text);
        self
    }

    pub fn enable(&mut self) {
        self.disabled = false;
    }

    pub fn select(&mut self) {
        self.selected = true;
    }

    pub fn selected(mut self) -> Self {
        self.select();
        self
    }

    pub const fn set_hover_style(mut self, style: Style) -> Self {
        self.hover_style = style;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn no_block(mut self) -> Self {
        self.block = None;
        self
    }

    pub fn hover_block(mut self, block: Block<'a>) -> Self {
        self.hover_block = Some(block);
        self
    }

    pub fn no_hover_block(mut self) -> Self {
        self.hover_block = None;
        self
    }

    pub fn set_hover_text(mut self, text: impl Into<Text<'a>>) -> Self {
        self.hover_text = Some(text.into());
        self
    }

    pub fn set_hotkey(mut self, k: KeyCode) -> Self {
        self.hotkey = Some(k);
        self
    }

    pub fn set_layer(mut self, layer: usize) -> Self {
        self.layer = layer;
        self
    }

    pub fn text_width(&self) -> usize {
        self.text.width()
    }
}

impl<'a> Styled for Button<'a> {
    type Item = Button<'a>;

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

impl<'a> Widget for Button<'a> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let inner = if area.height >= 3 {
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            })
        } else {
            area
        };

        let paragraph = if let Some(u) = self.hotkey {
            let split = self
                .text
                .to_string()
                .splitn(2, &u.to_string())
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            if split.len() > 1 {
                Paragraph::new(Line::from(vec![
                    Span::raw(split[0].clone()),
                    Span::styled(u.to_string(), UiStyle::DEFAULT.underlined()),
                    Span::raw(split[1].clone()),
                ]))
                .alignment(self.text_alignemnt)
            } else {
                Paragraph::new(self.text.clone()).alignment(self.text_alignemnt)
            }
        } else {
            Paragraph::new(self.text.clone()).alignment(self.text_alignemnt)
        };

        let paragraph_style = if self.selected {
            UiStyle::SELECTED_BUTTON
        } else if self.disabled {
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
            block = if self.selected {
                block.border_set(border::THICK)
            } else if self.disabled {
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

impl InteractiveWidget for Button<'_> {
    fn layer(&self) -> usize {
        self.layer
    }

    fn before_rendering(&mut self, area: Rect, callback_registry: &mut CallbackRegistry) {
        let inner = if area.height >= 3 {
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            })
        } else {
            area
        };
        self.is_hovered = callback_registry.is_hovering(inner)
            && callback_registry.get_active_layer() == self.layer();

        if !self.disabled {
            if self.is_hovered {
                callback_registry.register_mouse_callback(
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                    Some(inner),
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
