use super::{
    constants::UiStyle,
    traits::InteractiveStatefulWidget,
    ui_callback::{CallbackRegistry, UiCallback},
};
use ratatui::crossterm;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::Style,
    text::Text,
    widgets::{Block, Cell, HighlightSpacing, Row, StatefulWidget, Table, TableState, Widget},
};

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ClickableCell<'a> {
    content: Text<'a>,
    style: Style,
}

impl<'a> ClickableCell<'a> {
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
        Self {
            content: content.into(),
            style: Style::default(),
        }
    }
}

impl<'a> From<ClickableCell<'a>> for Cell<'a> {
    fn from(cell: ClickableCell<'a>) -> Self {
        Cell::new(cell.content).style(cell.style)
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ClickableRow<'a> {
    cells: Vec<ClickableCell<'a>>,
    height: u16,
    style: Style,
}

impl<'a> ClickableRow<'a> {
    pub fn new<T>(cells: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<ClickableCell<'a>>,
    {
        Self {
            height: 1,
            cells: cells.into_iter().map(Into::into).collect(),
            style: Style::default(),
        }
    }
}

impl<'a> From<ClickableRow<'a>> for Row<'a> {
    fn from(row: ClickableRow<'a>) -> Self {
        Row::new(row.cells.into_iter().map(Cell::from))
            .height(row.height)
            .style(row.style)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ClickableTable<'a> {
    inner: Table<'a>,
    block: Option<Block<'a>>,
    row_heights: Vec<u16>,
    header_offset: u16,
    hover_style: Style,
}

impl<'a> ClickableTable<'a> {
    pub fn new<T>(rows: T) -> Self
    where
        T: IntoIterator<Item = ClickableRow<'a>>,
    {
        let rows: Vec<ClickableRow<'a>> = rows.into_iter().collect();
        let row_heights = rows.iter().map(|r| r.height).collect();
        let inner = Table::new(rows.into_iter().map(Row::from), Vec::<Constraint>::new())
            .row_highlight_style(UiStyle::SELECTED)
            .highlight_spacing(HighlightSpacing::Never);
        Self {
            inner,
            block: None,
            row_heights,
            header_offset: 0,
            hover_style: UiStyle::HIGHLIGHT,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.inner = self.inner.block(block.clone());
        self.block = Some(block);
        self
    }

    pub fn header(mut self, header: ClickableRow<'a>) -> Self {
        self.header_offset = header.height;
        self.inner = self.inner.header(Row::from(header));
        self
    }

    pub fn widths(mut self, widths: &[Constraint]) -> Self {
        let between_0_and_100 = |&w| match w {
            Constraint::Percentage(p) => p <= 100,
            _ => true,
        };
        assert!(
            widths.iter().all(between_0_and_100),
            "Percentages should be between 0 and 100 inclusively."
        );
        self.inner = self.inner.widths(widths.iter().copied());
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.inner = self.inner.style(style);
        self
    }

    pub fn column_spacing(mut self, spacing: u16) -> Self {
        self.inner = self.inner.column_spacing(spacing);
        self
    }

    fn inner_area(&self, area: Rect) -> Rect {
        self.block.as_ref().map_or(area, |block| block.inner(area))
    }

    /// Largest scroll offset that still fills the viewport from the bottom.
    /// A persisted offset left over from a longer table (e.g. before a filter
    /// switch shrank it) would otherwise scroll rows off the top.
    fn max_offset(&self, area: Rect) -> usize {
        let inner_height = self
            .inner_area(area)
            .height
            .saturating_sub(self.header_offset);
        let mut acc = 0u16;
        for (i, &height) in self.row_heights.iter().enumerate().rev() {
            acc = acc.saturating_add(height);
            if acc >= inner_height {
                return i;
            }
        }
        0
    }

    fn hovered_row(
        &self,
        area: Rect,
        offset: usize,
        callback_registry: &CallbackRegistry,
    ) -> Option<(Rect, usize)> {
        let inner = self.inner_area(area);
        let mut y = inner.top().saturating_add(self.header_offset);
        let offset = offset.min(self.row_heights.len().saturating_sub(1));
        for i in offset..self.row_heights.len() {
            if y >= inner.bottom() {
                break;
            }
            let height = self.row_heights[i].min(inner.bottom() - y);
            let row = Rect {
                x: inner.left(),
                y,
                width: inner.width,
                height,
            };
            if callback_registry.is_hovering(row) {
                return Some((row, i));
            }
            y = y.saturating_add(self.row_heights[i]);
        }
        None
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ClickableTableState {
    offset: usize,
    selected: Option<usize>,
    hovered: Rect,
}

impl ClickableTableState {
    pub const fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    pub const fn reset_offset(&mut self) {
        self.offset = 0;
    }
}

impl StatefulWidget for &ClickableTable<'_> {
    type State = ClickableTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.offset = state.offset.min(self.max_offset(area));

        let mut inner_state = TableState::default()
            .with_offset(state.offset)
            .with_selected(state.selected);
        StatefulWidget::render(&self.inner, area, buf, &mut inner_state);
        state.offset = inner_state.offset();

        if state.hovered.width > 0 && state.hovered.height > 0 {
            buf.set_style(state.hovered, self.hover_style);
        }
    }
}

impl StatefulWidget for ClickableTable<'_> {
    type State = ClickableTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buf, state);
    }
}

impl Widget for ClickableTable<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ClickableTableState::default();
        StatefulWidget::render(&self, area, buf, &mut state);
    }
}

impl InteractiveStatefulWidget for &ClickableTable<'_> {
    fn layer(&self) -> usize {
        0
    }

    fn hover_text(&self) -> Text<'_> {
        "".into()
    }

    fn before_rendering(
        &self,
        area: Rect,
        callback_registry: &mut CallbackRegistry,
        state: &mut Self::State,
    ) {
        state.hovered = Rect::default();

        if self.inner_area(area).area() == 0 || self.row_heights.is_empty() {
            return;
        }

        let is_hovered = callback_registry.is_hovering(area)
            && callback_registry.get_active_layer() == self.layer();

        if !is_hovered {
            return;
        }

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

        if let Some((row, index)) = self.hovered_row(area, state.offset, callback_registry) {
            if state.selected != Some(index) {
                state.hovered = row;
            }
            callback_registry.register_mouse_callback(
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                Some(row),
                UiCallback::SetPanelIndex { index },
            );
        }
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
        &self,
        area: Rect,
        callback_registry: &mut CallbackRegistry,
        state: &mut Self::State,
    ) {
        InteractiveStatefulWidget::before_rendering(&self, area, callback_registry, state);
    }
}
