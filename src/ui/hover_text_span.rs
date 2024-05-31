use super::ui_callback::CallbackRegistry;
use ratatui::{
    prelude::*,
    widgets::{Clear, Paragraph, Widget},
};
use std::sync::{Arc, Mutex};

/// A ratatui Paragraph that can display hover text when the mouse hovers over it.
#[derive(Debug, Default, Clone)]
pub struct HoverTextSpan<'a> {
    /// Base span
    span: Span<'a>,
    /// Hover text
    /// If the hover text is not empty, the hover text will be displayed when the mouse hovers over the paragraph
    /// If the hover text is empty, the hover text will not be displayed
    hover_text: Text<'a>,
    /// Hover text render target
    hover_text_target: Rect,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
}

#[derive(Debug, Default, Clone, Copy,  PartialEq, Hash)]
pub struct Wrap {
    /// Should leading whitespace be trimmed
    pub trim: bool,
}

impl<'a> HoverTextSpan<'a> {
    pub fn new<T>(
        span: Span<'a>,
        hover_text: T,
        hover_text_target: Rect,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
    ) -> HoverTextSpan<'a>
    where
        T: Into<Text<'a>>,
    {
        HoverTextSpan {
            span,
            hover_text: hover_text.into(),
            hover_text_target,
            callback_registry,
        }
    }

    pub fn width(&self) -> usize {
        self.span.width()
    }
}

impl Widget for HoverTextSpan<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.span.render(area, buf);

        // Render hover text if the mouse is hovering over the paragraph
        // and the hover text is not empty.
        if self.callback_registry.lock().unwrap().is_hovering(area)
            && self.hover_text.to_string().len() > 0
        {
            Clear.render(self.hover_text_target, buf);
            let hover_text = Paragraph::new(self.hover_text).centered();
            hover_text.render(self.hover_text_target, buf);
        }
    }
}

impl std::fmt::Display for HoverTextSpan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.span.content)
    }
}
