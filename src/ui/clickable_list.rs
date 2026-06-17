use super::{
    constants::UiStyle,
    traits::InteractiveStatefulWidget,
    ui_callback::{CallbackRegistry, UiCallback},
};
use ratatui::crossterm;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Text,
    widgets::{Block, HighlightSpacing, List, ListItem, ListState, StatefulWidget, Widget},
};

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ClickableListState {
    offset: usize,
    selected: Option<usize>,
    hovered: Rect,
}

impl ClickableListState {
    pub const fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub const fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ClickableListItem<'a> {
    content: Text<'a>,
    style: Style,
}

impl<'a> ClickableListItem<'a> {
    pub fn new<T>(content: T) -> ClickableListItem<'a>
    where
        T: Into<Text<'a>>,
    {
        ClickableListItem {
            content: content.into(),
            style: Style::default(),
        }
    }

    pub fn height(&self) -> usize {
        self.content.height()
    }
}

impl<'a> From<ClickableListItem<'a>> for ListItem<'a> {
    fn from(item: ClickableListItem<'a>) -> Self {
        ListItem::new(item.content).style(item.style)
    }
}

#[derive(Debug)]
pub struct ClickableList<'a> {
    inner: List<'a>,
    block: Option<Block<'a>>,
    heights: Vec<u16>,
    hover_style: Style,
    selection_offset: usize,
}

impl<'a> ClickableList<'a> {
    pub fn new<T>(items: T) -> ClickableList<'a>
    where
        T: Into<Vec<ClickableListItem<'a>>>,
    {
        let items = items.into();
        let heights = items.iter().map(|item| item.height() as u16).collect();
        let inner = List::new(items)
            .highlight_style(UiStyle::SELECTED)
            .highlight_spacing(HighlightSpacing::Never);
        ClickableList {
            inner,
            block: None,
            heights,
            hover_style: UiStyle::HIGHLIGHT,
            selection_offset: 0,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> ClickableList<'a> {
        self.inner = self.inner.block(block.clone());
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> ClickableList<'a> {
        self.inner = self.inner.style(style);
        self
    }

    pub const fn set_selection_offset(mut self, offset: usize) -> Self {
        self.selection_offset = offset;
        self
    }

    fn inner_area(&self, area: Rect) -> Rect {
        self.block.as_ref().map_or(area, |block| block.inner(area))
    }

    fn hovered_row(
        &self,
        area: Rect,
        offset: usize,
        callback_registry: &CallbackRegistry,
    ) -> Option<(Rect, usize)> {
        let inner = self.inner_area(area);
        let offset = offset.min(self.heights.len().saturating_sub(1));
        let mut y = inner.top();
        for (i, &height) in self.heights.iter().enumerate().skip(offset) {
            if y >= inner.bottom() {
                break;
            }
            let height = height.min(inner.bottom() - y);
            let row = Rect {
                x: inner.left(),
                y,
                width: inner.width,
                height,
            };
            if callback_registry.is_hovering(row) {
                return Some((row, i));
            }
            y += height;
        }
        None
    }
}

impl StatefulWidget for &ClickableList<'_> {
    type State = ClickableListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let mut inner_state = ListState::default()
            .with_offset(state.offset)
            .with_selected(state.selected);
        StatefulWidget::render(&self.inner, area, buf, &mut inner_state);
        state.offset = inner_state.offset();

        if state.hovered.width > 0 && state.hovered.height > 0 {
            buf.set_style(state.hovered, self.hover_style);
        }
    }
}

impl StatefulWidget for ClickableList<'_> {
    type State = ClickableListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buf, state);
    }
}

impl Widget for ClickableList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ClickableListState::default();
        StatefulWidget::render(&self, area, buf, &mut state);
    }
}

impl InteractiveStatefulWidget for &ClickableList<'_> {
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

        if self.inner_area(area).is_empty() {
            return;
        }

        if self.heights.is_empty() {
            state.select(None);
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
                UiCallback::SetPanelIndex {
                    index: index + self.selection_offset,
                },
            );
        }
    }
}

impl InteractiveStatefulWidget for ClickableList<'_> {
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
