use super::{traits::HoverableWidget, ui_callback::CallbackRegistry};
use ratatui::{prelude::*, widgets::Widget};

/// A ratatui Paragraph that can display hover text when the mouse hovers over it.
#[derive(Debug, Default, Clone)]
pub struct HoverTextSpan<'a> {
    /// Base span
    span: Span<'a>,
    /// Hover text
    /// If the hover text is not empty, the hover text will be displayed when the mouse hovers over the paragraph
    /// If the hover text is empty, the hover text will not be displayed
    hover_text: Text<'a>,
    layer: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Hash)]
pub struct Wrap {
    /// Should leading whitespace be trimmed
    pub trim: bool,
}

impl<'a> HoverTextSpan<'a> {
    pub fn new<T>(span: Span<'a>, hover_text: T) -> HoverTextSpan<'a>
    where
        T: Into<Text<'a>>,
    {
        HoverTextSpan {
            span,
            hover_text: hover_text.into(),
            layer: 0,
        }
    }

    pub fn width(&self) -> usize {
        self.span.width()
    }
}

impl Widget for HoverTextSpan<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.span.render(area, buf);
    }
}

impl HoverableWidget for HoverTextSpan<'_> {
    fn layer(&self) -> usize {
        self.layer
    }

    fn before_rendering(&mut self, _area: Rect, _callback_registry: &mut CallbackRegistry) {}

    fn hover_text(&self) -> Text<'_> {
        self.hover_text.clone().into()
    }
}

impl std::fmt::Display for HoverTextSpan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.span.content)
    }
}
