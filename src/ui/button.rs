use super::{
    constants::{PrintableKeyCode, UiStyle},
    ui_callback::{CallbackRegistry, UiCallbackPreset},
    widgets::default_block,
};
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Margin, Rect},
    style::{Style, Styled, Stylize},
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Widget},
};
use std::{sync::Arc, sync::Mutex};

#[derive(Debug, Clone)]
pub struct Button<'a> {
    text: Text<'a>,
    hotkey: Option<KeyCode>,
    on_click: UiCallbackPreset,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    disabled: bool,
    disabled_text: Option<String>,
    text_alignemnt: ratatui::layout::Alignment,
    style: Style,
    hover_style: Style,
    box_style: Option<Style>,
    box_hover_style: Option<Style>,
    hover_text: Option<String>,
    hover_text_target: Option<Rect>,
    layer: u8,
}

impl<'a> From<Button<'a>> for Text<'a> {
    fn from(button: Button<'a>) -> Text<'a> {
        button.text
    }
}

impl<'a> Button<'a> {
    fn is_hovered(&self, rect: Rect) -> bool {
        self.callback_registry.lock().unwrap().is_hovering(rect)
            && self.layer == self.callback_registry.lock().unwrap().get_max_layer()
    }
    pub fn new(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
    ) -> Self {
        Self {
            text: text.into(),
            hotkey: None,
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Center,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: Some(Style::default()),
            box_hover_style: Some(Style::default()),
            hover_text: None,
            hover_text_target: None,
            layer: 0,
        }
    }

    pub fn box_on_hover(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
    ) -> Self {
        Self {
            text: text.into(),
            hotkey: None,
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Center,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::UNSELECTED,
            box_style: None,
            box_hover_style: Some(Style::default()),
            hover_text: None,
            hover_text_target: None,
            layer: 0,
        }
    }

    pub fn no_box(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
    ) -> Self {
        Self {
            text: text.into(),
            hotkey: None,
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Center,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: None,
            box_hover_style: None,
            hover_text: None,
            hover_text_target: None,
            layer: 0,
        }
    }

    pub fn paragraph(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
    ) -> Self {
        Self {
            text: text.into(),
            hotkey: None,
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Left,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: None,
            box_hover_style: None,
            hover_text: None,
            hover_text_target: None,
            layer: 0,
        }
    }

    pub fn text(
        text: Text<'a>,
        on_click: UiCallbackPreset,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
    ) -> Self {
        Self {
            text,
            hotkey: None,
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Left,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: None,
            box_hover_style: None,
            hover_text: None,
            hover_text_target: None,
            layer: 0,
        }
    }

    pub fn disable(&mut self, text: Option<String>) {
        self.disabled = true;
        self.disabled_text = text;
    }

    pub fn enable(&mut self) {
        self.disabled = false;
    }

    pub fn set_hover_style(mut self, style: Style) -> Self {
        self.hover_style = style;
        self
    }

    pub fn set_box_style(mut self, style: Style) -> Self {
        self.box_style = Some(style);
        self
    }

    pub fn set_hover_text(mut self, text: String, target: Rect) -> Self {
        self.hover_text = Some(text);
        self.hover_text_target = Some(target);
        self
    }

    pub fn set_hotkey(mut self, k: KeyCode) -> Self {
        self.hotkey = Some(k);
        self
    }

    pub fn set_layer(mut self, layer: u8) -> Self {
        self.layer = layer;
        self
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

        if self.disabled == false {
            self.callback_registry
                .lock()
                .unwrap()
                .register_mouse_callback(
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                    Some(inner),
                    self.on_click.clone(),
                );

            if let Some(key) = self.hotkey {
                self.callback_registry
                    .lock()
                    .unwrap()
                    .register_keyboard_callback(key, self.on_click.clone());
            }
        }

        let paragraph = if self.disabled {
            let text = if self.disabled_text.is_some() {
                self.disabled_text.clone().unwrap().into()
            } else {
                self.text.clone()
            };
            Paragraph::new(text).alignment(self.text_alignemnt)
        } else {
            if let Some(u) = self.hotkey {
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
            }
        };

        if area.height < 3 {
            if self.disabled {
                paragraph.set_style(UiStyle::UNSELECTABLE).render(area, buf);
            } else if self.is_hovered(inner) {
                paragraph.set_style(self.hover_style).render(area, buf);
            } else {
                paragraph.set_style(self.style).render(area, buf);
            }
        } else if self.disabled {
            if let Some(box_style) = self.box_style {
                paragraph
                    .set_style(UiStyle::UNSELECTABLE)
                    .block(default_block().border_style(box_style))
                    .render(area, buf);
            } else {
                paragraph
                    .set_style(UiStyle::UNSELECTABLE)
                    .render(inner, buf);
            }
        } else if self.is_hovered(inner) {
            if let Some(box_hover_style) = self.box_hover_style {
                paragraph
                    .set_style(self.hover_style)
                    .block(default_block().border_style(box_hover_style))
                    .render(area, buf);
            } else {
                paragraph.set_style(self.hover_style).render(inner, buf);
            }
        } else {
            if let Some(box_style) = self.box_style {
                paragraph
                    .set_style(self.style)
                    .block(default_block().border_style(box_style))
                    .render(area, buf);
            } else {
                paragraph.set_style(self.style).render(inner, buf);
            }
        }

        if self.hover_text.is_some() && self.hover_text_target.is_some() && self.is_hovered(area) {
            let hover_text = Paragraph::new(self.hover_text.unwrap()).centered();
            Clear.render(self.hover_text_target.unwrap(), buf);
            hover_text.render(self.hover_text_target.unwrap(), buf);
        }
    }
}

#[derive(Debug)]
pub struct RadioButton<'a> {
    pub text: Text<'a>,
    on_click: UiCallbackPreset,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    disabled: bool,
    linked_index: &'a mut usize,
    index: usize,
    style: Style,
    hover_style: Style,
    box_style: Option<Style>,
    box_hover_style: Option<Style>,
    box_hover_title: Option<String>,
    layer: u8,
}

impl<'a> From<RadioButton<'a>> for Text<'a> {
    fn from(button: RadioButton<'a>) -> Text<'a> {
        button.text
    }
}

impl<'a> Styled for RadioButton<'a> {
    type Item = RadioButton<'a>;

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

impl<'a> RadioButton<'a> {
    fn is_hovered(&self, rect: Rect) -> bool {
        self.callback_registry.lock().unwrap().is_hovering(rect)
            && self.layer == self.callback_registry.lock().unwrap().get_max_layer()
    }

    pub fn new(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        linked_index: &'a mut usize,
        index: usize,
    ) -> Self {
        Self {
            text: text.into(),
            on_click,
            callback_registry,
            disabled: false,
            linked_index,
            index,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: Some(Style::default()),
            box_hover_style: Some(Style::default()),
            box_hover_title: None,
            layer: 0,
        }
    }
    pub fn box_on_hover(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        linked_index: &'a mut usize,
        index: usize,
    ) -> Self {
        Self {
            text: text.into(),
            on_click,
            callback_registry,
            disabled: false,
            linked_index,
            index,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::UNSELECTED,
            box_style: None,
            box_hover_style: Some(Style::default()),
            box_hover_title: None,
            layer: 0,
        }
    }

    pub fn no_box(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        linked_index: &'a mut usize,
        index: usize,
    ) -> Self {
        Self {
            text: text.into(),
            on_click,
            callback_registry,
            disabled: false,
            linked_index,
            index,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: None,
            box_hover_style: None,
            box_hover_title: None,
            layer: 0,
        }
    }

    pub fn disable(&mut self) {
        self.disabled = true;
    }

    pub fn enable(&mut self) {
        self.disabled = false;
    }

    pub fn set_hover_style(mut self, style: Style) -> Self {
        self.hover_style = style;
        self
    }

    pub fn set_box_hover_style(mut self, style: Style) -> Self {
        self.box_hover_style = Some(style);
        self
    }

    pub fn set_box_hover_title(mut self, title: String) -> Self {
        self.box_hover_title = Some(title);
        self
    }
}

impl<'a> Widget for RadioButton<'a> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let inner = if area.height >= 3 {
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            })
        } else {
            area
        };

        if self.disabled == false {
            if self.is_hovered(inner) {
                *self.linked_index = self.index;
            }
            self.callback_registry
                .lock()
                .unwrap()
                .register_mouse_callback(
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                    Some(inner),
                    self.on_click,
                );
        }

        let paragraph = if self.disabled {
            Paragraph::new(self.text)
                .centered()
                .style(UiStyle::UNSELECTABLE)
        } else {
            if *self.linked_index == self.index {
                Paragraph::new(self.text).centered().style(self.hover_style)
            } else {
                Paragraph::new(self.text).centered().style(self.style)
            }
        };

        if area.height < 3 {
            paragraph.render(area, buf);
        } else {
            if *self.linked_index == self.index {
                if let Some(box_hover_style) = self.box_hover_style {
                    let block = if let Some(box_hover_title) = self.box_hover_title {
                        default_block()
                            .border_style(box_hover_style)
                            .title(box_hover_title)
                    } else {
                        default_block().border_style(box_hover_style)
                    };
                    paragraph.block(block).render(area, buf);
                } else {
                    paragraph.render(inner, buf);
                }
            } else {
                if let Some(box_style) = self.box_style {
                    paragraph
                        .block(default_block().style(box_style))
                        .render(area, buf);
                } else {
                    paragraph.render(inner, buf);
                }
            }
        }
    }
}
