use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Style, Styled},
    text::Text,
    widgets::{Block, HighlightSpacing, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

use super::{
    constants::UiStyle,
    traits::InteractiveStatefulWidget,
    ui_callback::{CallbackRegistry, UiCallback},
};

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
    select_style: Style,
    // Style used to render hovered item
    hover_style: Style,
    /// Symbol in front of the selected rom
    highlight_symbol: Option<&'a str>,
    /// Optional header
    header: Option<ClickableRow<'a>>,
    /// Data to display in each row
    rows: Vec<ClickableRow<'a>>,
    /// Decides when to allocate spacing for the row selection
    highlight_spacing: HighlightSpacing,
}

impl<'a> ClickableTable<'a> {
    pub fn new<T>(rows: T) -> Self
    where
        T: IntoIterator<Item = ClickableRow<'a>>,
    {
        Self {
            block: None,
            style: Style::default(),
            widths: &[],
            column_spacing: 1,
            select_style: UiStyle::SELECTED,
            hover_style: UiStyle::HIGHLIGHT,
            highlight_symbol: None,
            header: None,
            rows: rows.into_iter().collect(),
            highlight_spacing: HighlightSpacing::default(),
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

    pub const fn column_spacing(mut self, spacing: u16) -> Self {
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
    hovered: Rect,
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

        let (start, end) = self.get_row_bounds(state.selected, state.offset, rows_height);
        state.offset = start;
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
            if state.hovered == table_row_area {
                buf.set_style(table_row_area, self.hover_style);
            }
            if is_selected {
                buf.set_style(table_row_area, self.select_style);
            }
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

impl InteractiveStatefulWidget for ClickableTable<'_> {
    fn layer(&self) -> usize {
        0
    }

    fn hover_text(&self) -> Text<'_> {
        "".into()
    }

    fn before_rendering(
        &mut self,
        area: Rect,
        callback_registry: &mut CallbackRegistry,
        state: &mut Self::State,
    ) {
        if area.area() == 0 {
            return;
        }

        if self.rows.is_empty() {
            return;
        }

        let table_area = match self.block.as_ref() {
            Some(b) => b.inner(area),
            None => area,
        };

        if callback_registry.is_hovering(area) {
            callback_registry.register_mouse_callback(
                crossterm::event::MouseEventKind::ScrollDown,
                None,
                UiCallback::NextPanelIndex,
            );

            callback_registry.register_mouse_callback(
                crossterm::event::MouseEventKind::ScrollUp,
                None,
                UiCallback::PreviousPanelIndex,
            );
        }

        let mut current_height = 0;
        let mut rows_height = table_area.height;

        if let Some(ref header) = self.header {
            let max_header_height = table_area.height.min(header.total_height());
            current_height += max_header_height;
            rows_height = rows_height.saturating_sub(max_header_height);
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

            if callback_registry.is_hovering(table_row_area) {
                selected_element = Some((table_row_area, i));
                state.hovered = table_row_area;
                break;
            }
        }

        if let Some((area, index)) = selected_element {
            callback_registry.register_mouse_callback(
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                Some(area),
                UiCallback::SetPanelIndex { index },
            );
        }
    }
}
