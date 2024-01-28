use std::{cell::RefCell, rc::Rc};

use super::{
    constants::UiStyle,
    ui_callback::{CallbackRegistry, UiCallbackPreset},
    widgets::default_block,
};
use ratatui::{
    layout::Margin,
    style::{Style, Styled},
    text::Text,
    widgets::{Paragraph, Widget},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Button<'a> {
    text: Text<'a>,
    on_click: UiCallbackPreset,
    callback_registry: Rc<RefCell<CallbackRegistry>>,
    disabled: bool,
    disabled_text: Option<String>,
    text_alignemnt: ratatui::layout::Alignment,
    style: Style,
    hover_style: Style,
    box_style: Option<Style>,
    box_hover_style: Option<Style>,
}

impl<'a> From<Button<'a>> for Text<'a> {
    fn from(button: Button<'a>) -> Text<'a> {
        button.text
    }
}

impl<'a> Button<'a> {
    pub fn new(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Rc<RefCell<CallbackRegistry>>,
    ) -> Self {
        Self {
            text: text.into(),
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Center,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: Some(Style::default()),
            box_hover_style: Some(Style::default()),
        }
    }

    pub fn box_on_hover(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Rc<RefCell<CallbackRegistry>>,
    ) -> Self {
        Self {
            text: text.into(),
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Center,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::UNSELECTED,
            box_style: None,
            box_hover_style: Some(Style::default()),
        }
    }

    pub fn no_box(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Rc<RefCell<CallbackRegistry>>,
    ) -> Self {
        Self {
            text: text.into(),
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Center,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: None,
            box_hover_style: None,
        }
    }

    pub fn paragraph(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Rc<RefCell<CallbackRegistry>>,
    ) -> Self {
        Self {
            text: text.into(),
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Left,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: None,
            box_hover_style: None,
        }
    }

    pub fn text(
        text: Text<'a>,
        on_click: UiCallbackPreset,
        callback_registry: Rc<RefCell<CallbackRegistry>>,
    ) -> Self {
        Self {
            text,
            on_click,
            callback_registry,
            disabled: false,
            disabled_text: None,
            text_alignemnt: ratatui::layout::Alignment::Left,
            style: UiStyle::UNSELECTED,
            hover_style: UiStyle::SELECTED,
            box_style: None,
            box_hover_style: None,
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
}

impl<'a> Styled for Button<'a> {
    type Item = Button<'a>;

    fn style(&self) -> Style {
        self.style
    }
    fn set_style(self, style: Style) -> Self::Item {
        Self { style, ..self }
    }
}

impl<'a> Widget for Button<'a> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let inner = if area.height >= 3 {
            area.inner(&Margin {
                horizontal: 1,
                vertical: 1,
            })
        } else {
            area
        };
        if self.disabled == false {
            self.callback_registry.borrow_mut().register_callback(
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                Some(inner),
                self.on_click,
            );
        }

        let paragraph = if self.disabled {
            let text = if self.disabled_text.is_some() {
                self.disabled_text.unwrap().into()
            } else {
                self.text
            };
            Paragraph::new(text)
                .alignment(self.text_alignemnt)
                .style(UiStyle::UNSELECTABLE)
        } else {
            if self.callback_registry.borrow().is_hovering(inner) {
                Paragraph::new(self.text)
                    .alignment(self.text_alignemnt)
                    .style(self.hover_style)
            } else {
                Paragraph::new(self.text)
                    .alignment(self.text_alignemnt)
                    .style(self.style)
            }
        };

        if area.height < 3 {
            paragraph.render(area, buf);
        } else {
            if self.callback_registry.borrow().is_hovering(inner) {
                if let Some(box_hover_style) = self.box_hover_style {
                    paragraph
                        .block(default_block().border_style(box_hover_style))
                        .render(area, buf);
                } else {
                    paragraph.render(inner, buf);
                }
            } else {
                if let Some(box_style) = self.box_style {
                    paragraph
                        .block(default_block().border_style(box_style))
                        .render(area, buf);
                } else {
                    paragraph.render(inner, buf);
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RadioButton<'a> {
    pub text: Text<'a>,
    on_click: UiCallbackPreset,
    callback_registry: Rc<RefCell<CallbackRegistry>>,
    disabled: bool,
    linked_index: &'a mut usize,
    index: usize,
    style: Style,
    hover_style: Style,
    box_style: Option<Style>,
    box_hover_style: Option<Style>,
}

impl<'a> From<RadioButton<'a>> for Text<'a> {
    fn from(button: RadioButton<'a>) -> Text<'a> {
        button.text
    }
}

// impl<'a> From<RadioButton<'a>> for Span<'a> {
//     fn from(button: RadioButton) -> Span<'a> {
//         Span::raw(button.text)
//     }
// }

// impl<'a> From<RadioButton<'a>> for Line<'a> {
//     fn from(button: RadioButton<'a>) -> Self {
//         Self {
//             spans: vec![Span::from(button)],
//             ..Default::default()
//         }
//     }
// }

impl<'a> Styled for RadioButton<'a> {
    type Item = RadioButton<'a>;

    fn style(&self) -> Style {
        self.style
    }
    fn set_style(self, style: Style) -> Self::Item {
        Self { style, ..self }
    }
}

impl<'a> RadioButton<'a> {
    pub fn new(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Rc<RefCell<CallbackRegistry>>,
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
        }
    }
    pub fn box_on_hover(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Rc<RefCell<CallbackRegistry>>,
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
        }
    }

    pub fn no_box(
        text: String,
        on_click: UiCallbackPreset,
        callback_registry: Rc<RefCell<CallbackRegistry>>,
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
}

impl<'a> Widget for RadioButton<'a> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let inner = if area.height >= 3 {
            area.inner(&Margin {
                horizontal: 1,
                vertical: 1,
            })
        } else {
            area
        };

        if self.disabled == false {
            if self.callback_registry.borrow().is_hovering(inner) {
                *self.linked_index = self.index;
            }
            self.callback_registry.borrow_mut().register_callback(
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                Some(inner),
                self.on_click,
            );
        }

        let paragraph = if self.disabled {
            Paragraph::new(self.text)
                .alignment(ratatui::layout::Alignment::Center)
                .style(UiStyle::UNSELECTABLE)
        } else {
            if *self.linked_index == self.index {
                Paragraph::new(self.text)
                    .alignment(ratatui::layout::Alignment::Center)
                    .style(self.hover_style)
            } else {
                Paragraph::new(self.text)
                    .alignment(ratatui::layout::Alignment::Center)
                    .style(self.style)
            }
        };

        if area.height < 3 {
            paragraph.render(area, buf);
        } else {
            if *self.linked_index == self.index {
                if let Some(box_hover_style) = self.box_hover_style {
                    paragraph
                        .block(default_block().border_style(box_hover_style))
                        .render(area, buf);
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
