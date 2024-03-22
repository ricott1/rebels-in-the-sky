#![deny(missing_docs)]
use ratatui::{prelude::*, widgets::Widget};

use super::hover_text_span::HoverTextSpan;

/// A line of text, consisting of one or more [`HoverTextSpan`]s.
///
/// [`HoverTextLine`]s are used wherever text is displayed in the terminal and represent a single line of
/// text. When a [`HoverTextLine`] is rendered, it is rendered as a single line of text, with each [`HoverTextSpan`]
/// being rendered in order (left to right).
///
/// [`HoverTextLine`]s can be created from [`HoverTextSpan`]s, [`String`]s, and [`&str`]s. They can be styled with a
/// [`Style`], and have an [`Alignment`].
///
/// The line's [`Alignment`] is used by the rendering widget to determine how to align the line
/// within the available space. If the line is longer than the available space, the alignment is
/// ignored and the line is truncated.
///
/// The line's [`Style`] is used by the rendering widget to determine how to style the line. If the
/// line is longer than the available space, the style is applied to the entire line, and the line
/// is truncated. Each [`HoverTextSpan`] in the line will be styled with the [`Style`] of the line, and then
/// with its own [`Style`].
///
/// `HoverTextLine` implements the [`Widget`] trait, which means it can be rendered to a [`Buffer`]. Usually
/// apps will use the [`Paragraph`] widget instead of rendering a [`HoverTextLine`] directly as it provides
/// more functionality.
///
/// # Constructor Methods
///
/// - [`HoverTextLine::default`] creates a line with empty content and the default style.
/// - [`HoverTextLine::raw`] creates a line with the given content and the default style.
/// - [`HoverTextLine::styled`] creates a line with the given content and style.
///
/// # Setter Methods
///
/// These methods are fluent setters. They return a `HoverTextLine` with the property set.
///
/// - [`HoverTextLine::spans`] sets the content of the line.
/// - [`HoverTextLine::style`] sets the style of the line.
/// - [`HoverTextLine::alignment`] sets the alignment of the line.
///
/// # Other Methods
///
/// - [`HoverTextLine::patch_style`] patches the style of the line, adding modifiers from the given style.
/// - [`HoverTextLine::reset_style`] resets the style of the line.
/// - [`HoverTextLine::width`] returns the unicode width of the content held by this line.
/// - [`HoverTextLine::styled_graphemes`] returns an iterator over the graphemes held by this line.
///
/// # Compatibility Notes
///
/// Before v0.26.0, [`HoverTextLine`] did not have a `style` field and instead relied on only the styles that
/// were set on each [`HoverTextSpan`] contained in the `spans` field. The [`HoverTextLine::patch_style`] method was
/// the only way to set the overall style for individual lines. For this reason, this field may not
/// be supported yet by all widgets (outside of the `ratatui` crate itself).
///
/// # Examples
///
/// ```rust
/// use ratatui::prelude::*;
///
/// HoverTextLine::raw("unstyled");
/// HoverTextLine::styled("yellow text", Style::new().yellow());
/// HoverTextLine::from("red text").style(Style::new().red());
/// HoverTextLine::from(String::from("unstyled"));
/// HoverTextLine::from(vec![
///     HoverTextSpan::styled("Hello", Style::new().blue()),
///     HoverTextSpan::raw(" world!"),
/// ]);
/// ```
///
/// [`Paragraph`]: crate::widgets::Paragraph
#[derive(Debug, Default, Clone)]
pub struct HoverTextLine<'a> {
    /// The spans that make up this line of text.
    pub spans: Vec<HoverTextSpan<'a>>,

    /// The style of this line of text.
    pub style: Style,

    /// The alignment of this line of text.
    pub alignment: Option<Alignment>,
}

impl<'a> HoverTextLine<'a> {
    /// Sets the spans of this line of text.
    ///
    /// `spans` accepts any iterator that yields items that are convertible to [`HoverTextSpan`] (e.g.
    /// [`&str`], [`String`], [`HoverTextSpan`], or your own type that implements [`Into<HoverTextSpan>`]).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::prelude::*;
    /// let line = HoverTextLine::default().spans(vec!["Hello".blue(), " world!".green()]);
    /// let line = HoverTextLine::default().spans([1, 2, 3].iter().map(|i| format!("Item {}", i)));
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn spans<I>(mut self, spans: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<HoverTextSpan<'a>>,
    {
        self.spans = spans.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the style of this line of text.
    ///
    /// Defaults to [`Style::default()`].
    ///
    /// Note: This field was added in v0.26.0. Prior to that, the style of a line was determined
    /// only by the style of each [`HoverTextSpan`] contained in the line. For this reason, this field may
    /// not be supported by all widgets (outside of the `ratatui` crate itself).
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// # Examples
    /// ```rust
    /// # use ratatui::prelude::*;
    /// let mut line = HoverTextLine::from("foo").style(Style::new().red());
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Sets the target alignment for this line of text.
    ///
    /// Defaults to: [`None`], meaning the alignment is determined by the rendering widget.
    /// Setting the alignment of a HoverTextLine generally overrides the alignment of its
    /// parent Text or Widget.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::prelude::*;
    /// let mut line = HoverTextLine::from("Hi, what's up?");
    /// assert_eq!(None, line.alignment);
    /// assert_eq!(
    ///     Some(Alignment::Right),
    ///     line.alignment(Alignment::Right).alignment
    /// )
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn alignment(self, alignment: Alignment) -> Self {
        Self {
            alignment: Some(alignment),
            ..self
        }
    }

    /// Left-aligns this line of text.
    ///
    /// Convenience shortcut for `HoverTextLine::alignment(Alignment::Left)`.
    /// Setting the alignment of a HoverTextLine generally overrides the alignment of its
    /// parent Text or Widget, with the default alignment being inherited from the parent.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::prelude::*;
    /// let line = HoverTextLine::from("Hi, what's up?").left_aligned();
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn left_aligned(self) -> Self {
        self.alignment(Alignment::Left)
    }

    /// Center-aligns this line of text.
    ///
    /// Convenience shortcut for `HoverTextLine::alignment(Alignment::Center)`.
    /// Setting the alignment of a HoverTextLine generally overrides the alignment of its
    /// parent Text or Widget, with the default alignment being inherited from the parent.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::prelude::*;
    /// let line = HoverTextLine::from("Hi, what's up?").centered();
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn centered(self) -> Self {
        self.alignment(Alignment::Center)
    }

    /// Right-aligns this line of text.
    ///
    /// Convenience shortcut for `HoverTextLine::alignment(Alignment::Right)`.
    /// Setting the alignment of a HoverTextLine generally overrides the alignment of its
    /// parent Text or Widget, with the default alignment being inherited from the parent.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::prelude::*;
    /// let line = HoverTextLine::from("Hi, what's up?").right_aligned();
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn right_aligned(self) -> Self {
        self.alignment(Alignment::Right)
    }

    /// Returns the width of the underlying string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::prelude::*;
    /// let line = HoverTextLine::from(vec!["Hello".blue(), " world!".green()]);
    /// assert_eq!(12, line.width());
    /// ```
    pub fn width(&self) -> usize {
        self.spans.iter().map(HoverTextSpan::width).sum()
    }

    /// Patches the style of this HoverTextLine, adding modifiers from the given style.
    ///
    /// This is useful for when you want to apply a style to a line that already has some styling.
    /// In contrast to [`HoverTextLine::style`], this method will not overwrite the existing style, but
    /// instead will add the given style's modifiers to this HoverTextLine's style.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::prelude::*;
    /// let line = HoverTextLine::styled("My text", Modifier::ITALIC);
    ///
    /// let styled_line = HoverTextLine::styled("My text", (Color::Yellow, Modifier::ITALIC));
    ///
    /// assert_eq!(styled_line, line.patch_style(Color::Yellow));
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn patch_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = self.style.patch(style);
        self
    }

    /// Resets the style of this HoverTextLine.
    ///
    /// Equivalent to calling `patch_style(Style::reset())`.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::prelude::*;
    /// # let style = Style::default().yellow();
    /// let line = HoverTextLine::styled("My text", style);
    ///
    /// assert_eq!(Style::reset(), line.reset_style().style);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn reset_style(self) -> Self {
        self.patch_style(Style::reset())
    }

    /// Returns an iterator over the spans of this line.
    pub fn iter(&self) -> std::slice::Iter<HoverTextSpan<'a>> {
        self.spans.iter()
    }

    /// Returns a mutable iterator over the spans of this line.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<HoverTextSpan<'a>> {
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
