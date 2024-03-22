use std::{sync::Arc, sync::Mutex};

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Styled},
    text::Text,
    widgets::{Block, HighlightSpacing, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use super::ui_callback::{CallbackRegistry, UiCallbackPreset};

/// A [`ClickableCell`] contains the [`Text`] to be displayed in a [`ClickableRow`] of a [`Table`].
///
/// It can be created from anything that can be converted to a [`Text`].
/// ```rust
/// # use ratatui::widgets::ClickableCell;
/// # use ratatui::style::{Style, Modifier};
/// # use ratatui::text::{Span, Line, Text};
/// # use std::borrow::Cow;
/// ClickableCell::from("simple string");
///
/// ClickableCell::from(Span::from("span"));
///
/// ClickableCell::from(Line::from(vec![
///     Span::raw("a vec of "),
///     Span::styled("spans", Style::default().add_modifier(Modifier::BOLD))
/// ]));
///
/// ClickableCell::from(Text::from("a text"));
///
/// ClickableCell::from(Text::from(Cow::Borrowed("hello")));
/// ```
///
/// You can apply a [`Style`] on the entire [`ClickableCell`] using [`ClickableCell::style`] or rely on the styling
/// capabilities of [`Text`].
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ClickableCell<'a> {
    content: Text<'a>,
    style: Style,
}

impl<'a> ClickableCell<'a> {
    /// Set the `Style` of this cell.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl<'a, T> From<T> for ClickableCell<'a>
where
    T: Into<Text<'a>>,
{
    fn from(content: T) -> ClickableCell<'a> {
        ClickableCell {
            content: content.into(),
            style: Style::default(),
        }
    }
}

impl<'a> Styled for ClickableCell<'a> {
    type Item = ClickableCell<'a>;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style.into())
    }
}

/// Holds data to be displayed in a [`Table`] widget.
///
/// A [`ClickableRow`] is a collection of cells. It can be created from simple strings:
/// ```rust
/// # use ratatui::widgets::ClickableRow;
/// ClickableRow::new(vec!["Cell1", "Cell2", "Cell3"]);
/// ```
///
/// But if you need a bit more control over individual cells, you can explicitly create [`ClickableCell`]s:
/// ```rust
/// # use ratatui::widgets::{ClickableRow, ClickableCell};
/// # use ratatui::style::{Style, Color};
/// ClickableRow::new(vec![
///     ClickableCell::from("Cell1"),
///     ClickableCell::from("Cell2").style(Style::default().fg(Color::Yellow)),
/// ]);
/// ```
///
/// You can also construct a row from any type that can be converted into [`Text`]:
/// ```rust
/// # use std::borrow::Cow;
/// # use ratatui::widgets::ClickableRow;
/// ClickableRow::new(vec![
///     Cow::Borrowed("hello"),
///     Cow::Owned("world".to_uppercase()),
/// ]);
/// ```
///
/// By default, a row has a height of 1 but you can change this using [`ClickableRow::height`].
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ClickableRow<'a> {
    cells: Vec<ClickableCell<'a>>,
    height: u16,
    style: Style,
    bottom_margin: u16,
}

impl<'a> ClickableRow<'a> {
    /// Creates a new [`ClickableRow`] from an iterator where items can be converted to a [`ClickableCell`].
    pub fn new<T>(cells: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<ClickableCell<'a>>,
    {
        Self {
            height: 1,
            cells: cells.into_iter().map(Into::into).collect(),
            style: Style::default(),
            bottom_margin: 0,
        }
    }

    /// Set the fixed height of the [`ClickableRow`]. Any [`ClickableCell`] whose content has more lines than this
    /// height will see its content truncated.
    pub fn _height(mut self, height: u16) -> Self {
        self.height = height;
        self
    }

    /// Set the [`Style`] of the entire row. This [`Style`] can be overridden by the [`Style`] of a
    /// any individual [`ClickableCell`] or event by their [`Text`] content.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the bottom margin. By default, the bottom margin is `0`.
    pub fn _bottom_margin(mut self, margin: u16) -> Self {
        self.bottom_margin = margin;
        self
    }

    /// Returns the total height of the row.
    fn total_height(&self) -> u16 {
        self.height.saturating_add(self.bottom_margin)
    }
}

impl<'a> Styled for ClickableRow<'a> {
    type Item = ClickableRow<'a>;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style.into())
    }
}

/// A widget to display data in formatted columns.
///
/// It is a collection of [`ClickableRow`]s, themselves composed of [`ClickableCell`]s:
/// ```rust
/// # use ratatui::widgets::{Block, Borders, Table, ClickableRow, ClickableCell};
/// # use ratatui::layout::Constraint;
/// # use ratatui::style::{Style, Color, Modifier};
/// # use ratatui::text::{Text, Line, Span};
/// Table::new(vec![
///     // ClickableRow can be created from simple strings.
///     ClickableRow::new(vec!["Row11", "Row12", "Row13"]),
///     // You can style the entire row.
///     ClickableRow::new(vec!["Row21", "Row22", "Row23"]).style(Style::default().fg(Color::Blue)),
///     // If you need more control over the styling you may need to create Cells directly
///     ClickableRow::new(vec![
///         ClickableCell::from("Row31"),
///         ClickableCell::from("Row32").style(Style::default().fg(Color::Yellow)),
///         ClickableCell::from(Line::from(vec![
///             Span::raw("ClickableRow"),
///             Span::styled("33", Style::default().fg(Color::Green))
///         ])),
///     ]),
///     // If a ClickableRow need to display some content over multiple lines, you just have to change
///     // its height.
///     ClickableRow::new(vec![
///         ClickableCell::from("ClickableRow\n41"),
///         ClickableCell::from("ClickableRow\n42"),
///         ClickableCell::from("ClickableRow\n43"),
///     ]).height(2),
/// ])
/// // You can set the style of the entire Table.
/// .style(Style::default().fg(Color::White))
/// // It has an optional header, which is simply a ClickableRow always visible at the top.
/// .header(
///     ClickableRow::new(vec!["Col1", "Col2", "Col3"])
///         .style(Style::default().fg(Color::Yellow))
///         // If you want some space between the header and the rest of the rows, you can always
///         // specify some margin at the bottom.
///         .bottom_margin(1)
/// )
/// // As any other widget, a Table can be wrapped in a Block.
/// .block(Block::default().title("Table"))
/// // Columns widths are constrained in the same way as Layout...
/// .widths(&[Constraint::Length(5), Constraint::Length(5), Constraint::Length(10)])
/// // ...and they can be separated by a fixed spacing.
/// .column_spacing(1)
/// // If you wish to highlight a row in any specific way when it is selected...
/// .highlight_style(Style::default().add_modifier(Modifier::BOLD))
/// // ...and potentially show a symbol in front of the selection.
/// .highlight_symbol(">>");
/// ```
#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
pub struct ClickableTable<'a> {
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    /// Base style for the widget
    style: Style,
    /// Width constraints for each column
    widths: &'a [Constraint],
    /// Space between each column
    column_spacing: u16,
    /// Style used to render the selected row
    highlight_style: Style,
    // Style used to render hovered item
    hovering_style: Style,
    /// Symbol in front of the selected rom
    highlight_symbol: Option<&'a str>,
    /// Optional header
    header: Option<ClickableRow<'a>>,
    /// Data to display in each row
    rows: Vec<ClickableRow<'a>>,
    /// Decides when to allocate spacing for the row selection
    highlight_spacing: HighlightSpacing,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
}

impl<'a> ClickableTable<'a> {
    pub fn new<T>(rows: T, callback_registry: Arc<Mutex<CallbackRegistry>>) -> Self
    where
        T: IntoIterator<Item = ClickableRow<'a>>,
    {
        Self {
            block: None,
            style: Style::default(),
            widths: &[],
            column_spacing: 1,
            highlight_style: Style::default(),
            hovering_style: Style::default(),
            highlight_symbol: None,
            header: None,
            rows: rows.into_iter().collect(),
            highlight_spacing: HighlightSpacing::default(),
            callback_registry,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn header(mut self, header: ClickableRow<'a>) -> Self {
        self.header = Some(header);
        self
    }

    pub fn widths(mut self, widths: &'a [Constraint]) -> Self {
        let between_0_and_100 = |&w| match w {
            Constraint::Percentage(p) => p <= 100,
            _ => true,
        };
        assert!(
            widths.iter().all(between_0_and_100),
            "Percentages should be between 0 and 100 inclusively."
        );
        self.widths = widths;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn _highlight_symbol(mut self, highlight_symbol: &'a str) -> Self {
        self.highlight_symbol = Some(highlight_symbol);
        self
    }

    pub fn highlight_style(mut self, highlight_style: Style) -> Self {
        self.highlight_style = highlight_style;
        self
    }

    pub fn hovering_style(mut self, style: Style) -> Self {
        self.hovering_style = style;
        self
    }

    /// Set when to show the highlight spacing
    ///
    /// See [`HighlightSpacing`] about which variant affects spacing in which way
    pub fn _highlight_spacing(mut self, value: HighlightSpacing) -> Self {
        self.highlight_spacing = value;
        self
    }

    pub fn _column_spacing(mut self, spacing: u16) -> Self {
        self.column_spacing = spacing;
        self
    }

    /// Get all offsets and widths of all user specified columns
    /// Returns (x, width)
    fn get_columns_widths(&self, max_width: u16, selection_width: u16) -> Vec<(u16, u16)> {
        let mut constraints = Vec::with_capacity(self.widths.len() * 2 + 1);
        constraints.push(Constraint::Length(selection_width));
        for constraint in self.widths {
            constraints.push(*constraint);
            constraints.push(Constraint::Length(self.column_spacing));
        }
        if !self.widths.is_empty() {
            constraints.pop();
        }
        let chunks = Layout::horizontal(constraints)
            // .segment_size(SegmentSize::None)
            .split(Rect {
                x: 0,
                y: 0,
                width: max_width,
                height: 1,
            });
        chunks
            .iter()
            .skip(1)
            .step_by(2)
            .map(|c| (c.x, c.width))
            .collect()
    }

    fn get_row_bounds(
        &self,
        selected: Option<usize>,
        offset: usize,
        max_height: u16,
    ) -> (usize, usize) {
        let offset = offset.min(self.rows.len().saturating_sub(1));
        let mut start = offset;
        let mut end = offset;
        let mut height = 0;
        for item in self.rows.iter().skip(offset) {
            if height + item.height > max_height {
                break;
            }
            height += item.total_height();
            end += 1;
        }

        let selected = selected.unwrap_or(0).min(self.rows.len() - 1);
        while selected >= end {
            height = height.saturating_add(self.rows[end].total_height());
            end += 1;
            while height > max_height {
                height = height.saturating_sub(self.rows[start].total_height());
                start += 1;
            }
        }
        while selected < start {
            start -= 1;
            height = height.saturating_add(self.rows[start].total_height());
            while height > max_height {
                end -= 1;
                height = height.saturating_sub(self.rows[end].total_height());
            }
        }
        (start, end)
    }
}

impl<'a> Styled for ClickableTable<'a> {
    type Item = ClickableTable<'a>;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style.into())
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ClickableTableState {
    offset: usize,
    selected: Option<usize>,
}

impl ClickableTableState {
    pub fn _offset(&self) -> usize {
        self.offset
    }

    pub fn _offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    pub fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub fn _with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub fn _selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn _select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }
}

impl<'a> StatefulWidget for ClickableTable<'a> {
    type State = ClickableTableState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.area() == 0 {
            return;
        }
        buf.set_style(area, self.style);
        let table_area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        let selection_width = if state.selected.is_some() {
            self.highlight_symbol.map_or(0, |s| s.width() as u16)
        } else {
            0
        };
        let columns_widths = self.get_columns_widths(table_area.width, selection_width);
        let highlight_symbol = self.highlight_symbol.unwrap_or("");
        let mut current_height = 0;
        let mut rows_height = table_area.height;

        // Draw header
        if let Some(ref header) = self.header {
            let max_header_height = table_area.height.min(header.total_height());
            buf.set_style(
                Rect {
                    x: table_area.left(),
                    y: table_area.top(),
                    width: table_area.width,
                    height: table_area.height.min(header.height),
                },
                header.style,
            );
            let inner_offset = table_area.left();
            for ((x, width), cell) in columns_widths.iter().zip(header.cells.iter()) {
                render_cell(
                    buf,
                    cell,
                    Rect {
                        x: inner_offset + x,
                        y: table_area.top(),
                        width: *width,
                        height: max_header_height,
                    },
                );
            }
            current_height += max_header_height;
            rows_height = rows_height.saturating_sub(max_header_height);
        }

        // Draw rows
        if self.rows.is_empty() {
            return;
        }

        if self.callback_registry.lock().unwrap().is_hovering(area) {
            self.callback_registry.lock().unwrap().register_callback(
                crossterm::event::MouseEventKind::ScrollDown,
                None,
                UiCallbackPreset::NextPanelIndex,
            );

            self.callback_registry.lock().unwrap().register_callback(
                crossterm::event::MouseEventKind::ScrollUp,
                None,
                UiCallbackPreset::PreviousPanelIndex,
            );
        }

        let (start, end) = self.get_row_bounds(state.selected, state.offset, rows_height);
        state.offset = start;
        let mut selected_element: Option<(Rect, usize)> = None;
        for (i, table_row) in self
            .rows
            .iter_mut()
            .enumerate()
            .skip(state.offset)
            .take(end - start)
        {
            let (row, inner_offset) = (table_area.top() + current_height, table_area.left());
            current_height += table_row.total_height();
            let table_row_area = Rect {
                x: inner_offset,
                y: row,
                width: table_area.width,
                height: table_row.height,
            };
            buf.set_style(table_row_area, table_row.style);
            let is_selected = state.selected.map_or(false, |s| s == i);
            if selection_width > 0 && is_selected {
                // this should in normal cases be safe, because "get_columns_widths" allocates
                // "highlight_symbol.width()" space but "get_columns_widths"
                // currently does not bind it to max table.width()
                buf.set_stringn(
                    inner_offset,
                    row,
                    highlight_symbol,
                    table_area.width as usize,
                    table_row.style,
                );
            };
            for ((x, width), cell) in columns_widths.iter().zip(table_row.cells.iter()) {
                render_cell(
                    buf,
                    cell,
                    Rect {
                        x: inner_offset + x,
                        y: row,
                        width: *width,
                        height: table_row.height,
                    },
                );
            }
            if self
                .callback_registry
                .lock()
                .unwrap()
                .is_hovering(table_row_area)
            {
                selected_element = Some((table_row_area, i));
                buf.set_style(table_row_area, self.hovering_style);
            }
            if is_selected {
                buf.set_style(table_row_area, self.highlight_style);
            }
        }
        if let Some((area, index)) = selected_element {
            self.callback_registry.lock().unwrap().register_callback(
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                Some(area),
                UiCallbackPreset::SetPanelIndex { index },
            );
        }
    }
}

fn render_cell(buf: &mut Buffer, cell: &ClickableCell, area: Rect) {
    buf.set_style(area, cell.style);
    for (i, line) in cell.content.lines.iter().enumerate() {
        if i as u16 >= area.height {
            break;
        }

        let x_offset = match line.alignment {
            Some(Alignment::Center) => (area.width / 2).saturating_sub(line.width() as u16 / 2),
            Some(Alignment::Right) => area.width.saturating_sub(line.width() as u16),
            _ => 0,
        };

        buf.set_line(area.x + x_offset, area.y + i as u16, line, area.width);
    }
}

impl<'a> Widget for ClickableTable<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ClickableTableState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
