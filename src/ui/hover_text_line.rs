use super::{hover_text_span::HoverTextSpan, traits::InteractiveWidget};
use itertools::Itertools;
use ratatui::{prelude::*, widgets::Widget};

#[derive(Debug, Default, Clone)]
pub struct HoverTextLine<'a> {
    pub spans: Vec<HoverTextSpan<'a>>,
    pub style: Style,
    pub alignment: Option<Alignment>,
    hovered_span_index: usize,
}

impl<'a> HoverTextLine<'a> {
    pub fn spans<I>(mut self, spans: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<HoverTextSpan<'a>>,
    {
        self.spans = spans.into_iter().map(Into::into).collect();
        self
    }

    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    pub fn alignment(self, alignment: Alignment) -> Self {
        Self {
            alignment: Some(alignment),
            ..self
        }
    }

    pub fn left_aligned(self) -> Self {
        self.alignment(Alignment::Left)
    }

    pub fn centered(self) -> Self {
        self.alignment(Alignment::Center)
    }

    pub fn right_aligned(self) -> Self {
        self.alignment(Alignment::Right)
    }

    pub fn width(&self) -> usize {
        self.spans.iter().map(HoverTextSpan::width).sum()
    }

    pub fn patch_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = self.style.patch(style);
        self
    }

    pub fn reset_style(self) -> Self {
        self.patch_style(Style::reset())
    }

    /// Returns an iterator over the spans of this line.
    pub fn iter(&'_ self) -> std::slice::Iter<'_, HoverTextSpan<'a>> {
        self.spans.iter()
    }

    /// Returns a mutable iterator over the spans of this line.
    pub fn iter_mut(&'_ mut self) -> std::slice::IterMut<'_, HoverTextSpan<'a>> {
        self.spans.iter_mut()
    }
}

impl<'a> IntoIterator for HoverTextLine<'a> {
    type Item = HoverTextSpan<'a>;
    type IntoIter = std::vec::IntoIter<HoverTextSpan<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.spans.into_iter()
    }
}

impl<'a> IntoIterator for &'a HoverTextLine<'a> {
    type Item = &'a HoverTextSpan<'a>;
    type IntoIter = std::slice::Iter<'a, HoverTextSpan<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut HoverTextLine<'a> {
    type Item = &'a mut HoverTextSpan<'a>;
    type IntoIter = std::slice::IterMut<'a, HoverTextSpan<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'a> From<Vec<HoverTextSpan<'a>>> for HoverTextLine<'a> {
    fn from(spans: Vec<HoverTextSpan<'a>>) -> Self {
        Self {
            spans,
            ..Default::default()
        }
    }
}

impl<'a> From<HoverTextSpan<'a>> for HoverTextLine<'a> {
    fn from(span: HoverTextSpan<'a>) -> Self {
        Self::from(vec![span])
    }
}

impl<'a> From<Line<'a>> for HoverTextLine<'a> {
    fn from(value: Line<'a>) -> Self {
        Self {
            spans: value
                .spans
                .iter()
                .map(|s| HoverTextSpan::new(s.clone(), ""))
                .collect_vec(),
            ..Default::default()
        }
    }
}

impl<'a> From<Vec<Span<'a>>> for HoverTextLine<'a> {
    fn from(spans: Vec<Span<'a>>) -> Self {
        Self {
            spans: spans
                .iter()
                .map(|s| HoverTextSpan::new(s.clone(), ""))
                .collect_vec(),
            ..Default::default()
        }
    }
}

impl<'a> From<Span<'a>> for HoverTextLine<'a> {
    fn from(span: Span<'a>) -> Self {
        Self::from(vec![span])
    }
}

impl<'a> From<String> for HoverTextLine<'a> {
    fn from(value: String) -> Self {
        Self {
            spans: vec![HoverTextSpan::new(Span::raw(value), "")],
            ..Default::default()
        }
    }
}

impl Widget for HoverTextLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        buf.set_style(area, self.style);
        let width = self.width() as u16;
        let offset = match self.alignment {
            Some(Alignment::Left) => 0,
            Some(Alignment::Center) => (area.width.saturating_sub(width)) / 2,
            Some(Alignment::Right) => area.width.saturating_sub(width),
            None => 0,
        };
        let mut x = area.left().saturating_add(offset);
        for span in self.spans.iter() {
            let span_width = span.width() as u16;
            let span_area = Rect {
                x,
                width: span_width.min(area.right() - x),
                ..area
            };
            span.clone().render(span_area, buf);
            x = x.saturating_add(span_width);
            if x >= area.right() {
                break;
            }
        }
    }
}

impl std::fmt::Display for HoverTextLine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for span in &self.spans {
            write!(f, "{span}")?;
        }
        Ok(())
    }
}

impl InteractiveWidget for HoverTextLine<'_> {
    fn layer(&self) -> usize {
        0
    }

    fn hover_text(&self) -> Text<'_> {
        if self.hovered_span_index < self.spans.len() {
            self.spans[self.hovered_span_index].hover_text()
        } else {
            "".into()
        }
    }

    fn before_rendering(
        &mut self,
        area: Rect,
        callback_registry: &mut super::ui_callback::CallbackRegistry,
    ) {
        let width = self.width() as u16;
        let offset = match self.alignment {
            Some(Alignment::Left) => 0,
            Some(Alignment::Center) => (area.width.saturating_sub(width)) / 2,
            Some(Alignment::Right) => area.width.saturating_sub(width),
            None => 0,
        };
        let mut x = area.left().saturating_add(offset);
        for (index, span) in self.spans.iter().enumerate() {
            let span_width = span.width() as u16;
            let span_area = Rect {
                x,
                width: span_width.min(area.right() - x),
                ..area
            };

            if callback_registry.is_hovering(span_area) {
                self.hovered_span_index = index;
                break;
            }

            x = x.saturating_add(span_width);
            if x >= area.right() {
                break;
            }
        }
    }
}
