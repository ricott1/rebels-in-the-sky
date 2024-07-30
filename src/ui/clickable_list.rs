use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    prelude::*,
    style::{Style, Styled},
    text::Text,
    widgets::{Block, HighlightSpacing, ListDirection, StatefulWidget, Widget},
};
use std::{sync::Arc, sync::Mutex};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct ClickableListState {
    offset: usize,
    selected: Option<usize>,
}

impl ClickableListState {
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    pub fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn select(&mut self, index: Option<usize>) {
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

    pub fn style(mut self, style: Style) -> ClickableListItem<'a> {
        self.style = style;
        self
    }

    pub fn height(&self) -> usize {
        self.content.height()
    }

    pub fn width(&self) -> usize {
        self.content.width()
    }
}

#[derive(Debug, Default)]
pub struct ClickableList<'a> {
    block: Option<Block<'a>>,
    items: Vec<ClickableListItem<'a>>,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    /// Style used as a base style for the widget
    style: Style,
    /// List display direction
    direction: ListDirection,
    /// Style used to render selected item
    highlight_style: Style,
    // Style used to render hovered item
    hovering_style: Style,
    /// Symbol in front of the selected item (Shift all items to the right)
    highlight_symbol: Option<&'a str>,
    /// Whether to repeat the highlight symbol for each line of the selected item
    repeat_highlight_symbol: bool,
    /// Decides when to allocate spacing for the selection symbol
    highlight_spacing: HighlightSpacing,
    /// How many items to try to keep visible before and after the selected item
    scroll_padding: usize,
}

impl<'a> ClickableList<'a> {
    pub fn new<T>(items: T, callback_registry: Arc<Mutex<CallbackRegistry>>) -> ClickableList<'a>
    where
        T: Into<Vec<ClickableListItem<'a>>>,
    {
        ClickableList {
            block: None,
            style: Style::default(),
            items: items.into(),
            callback_registry,
            direction: ListDirection::default(),
            ..Self::default()
        }
    }

    pub fn block(mut self, block: Block<'a>) -> ClickableList<'a> {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> ClickableList<'a> {
        self.style = style;
        self
    }

    pub fn highlight_symbol(mut self, highlight_symbol: &'a str) -> ClickableList<'a> {
        self.highlight_symbol = Some(highlight_symbol);
        self
    }

    pub fn highlight_style(mut self, style: Style) -> ClickableList<'a> {
        self.highlight_style = style;
        self
    }

    pub fn hovering_style(mut self, style: Style) -> ClickableList<'a> {
        self.hovering_style = style;
        self
    }

    pub fn repeat_highlight_symbol(mut self, repeat: bool) -> ClickableList<'a> {
        self.repeat_highlight_symbol = repeat;
        self
    }

    pub fn highlight_spacing(mut self, value: HighlightSpacing) -> Self {
        self.highlight_spacing = value;
        self
    }

    pub const fn direction(mut self, direction: ListDirection) -> Self {
        self.direction = direction;
        self
    }

    pub const fn scroll_padding(mut self, padding: usize) -> Self {
        self.scroll_padding = padding;
        self
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn get_items_bounds(
        &self,
        selected: Option<usize>,
        offset: usize,
        max_height: usize,
    ) -> (usize, usize) {
        let offset = offset.min(self.items.len().saturating_sub(1));
        let mut start = offset;
        let mut end = offset;
        let mut height = 0;
        for item in self.items.iter().skip(offset) {
            if height + item.height() > max_height {
                break;
            }
            height += item.height();
            end += 1;
        }

        let selected = selected.unwrap_or(0).min(self.items.len() - 1);
        while selected >= end {
            height = height.saturating_add(self.items[end].height());
            end += 1;
            while height > max_height {
                height = height.saturating_sub(self.items[start].height());
                start += 1;
            }
        }
        while selected < start {
            start -= 1;
            height = height.saturating_add(self.items[start].height());
            while height > max_height {
                end -= 1;
                height = height.saturating_sub(self.items[end].height());
            }
        }
        (start, end)
    }
}

impl StatefulWidget for ClickableList<'_> {
    type State = ClickableListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(area, self.style);
        self.block.render(area, buf);
        let list_area = self.block.inner_if_some(area);

        if list_area.is_empty() {
            return;
        }

        if self.items.is_empty() {
            state.select(None);
            return;
        }

        // If the selected index is out of bounds, set it to the last item
        if state.selected.is_some_and(|s| s >= self.items.len()) {
            state.select(Some(self.items.len().saturating_sub(1)));
        }

        if self.callback_registry.lock().unwrap().is_hovering(area) {
            self.callback_registry
                .lock()
                .unwrap()
                .register_mouse_callback(
                    crossterm::event::MouseEventKind::ScrollDown,
                    None,
                    UiCallbackPreset::NextPanelIndex,
                );

            self.callback_registry
                .lock()
                .unwrap()
                .register_mouse_callback(
                    crossterm::event::MouseEventKind::ScrollUp,
                    None,
                    UiCallbackPreset::PreviousPanelIndex,
                );
        }

        let list_height = list_area.height as usize;

        let (first_visible_index, last_visible_index) =
            self.get_items_bounds(state.selected, state.offset, list_height);

        // Important: this changes the state's offset to be the beginning of the now viewable items
        state.offset = first_visible_index;

        // Get our set highlighted symbol (if one was set)
        let highlight_symbol = self.highlight_symbol.unwrap_or("");
        let blank_symbol = " ".repeat(highlight_symbol.width());

        let mut current_height = 0;
        let selection_spacing = state.selected.is_some();

        let mut selected_element: Option<(Rect, usize)> = None;
        for (i, item) in self
            .items
            .iter()
            .enumerate()
            .skip(state.offset)
            .take(last_visible_index - first_visible_index)
        {
            let (x, y) = if self.direction == ListDirection::BottomToTop {
                current_height += item.height() as u16;
                (list_area.left(), list_area.bottom() - current_height)
            } else {
                let pos = (list_area.left(), list_area.top() + current_height);
                current_height += item.height() as u16;
                pos
            };

            let row_area = Rect {
                x,
                y,
                width: list_area.width,
                height: item.height() as u16,
            };

            let item_style = self.style.patch(item.style);
            buf.set_style(row_area, item_style);

            let is_selected = state.selected.map_or(false, |s| s == i);

            let item_area = if selection_spacing {
                let highlight_symbol_width = self.highlight_symbol.unwrap_or("").width() as u16;
                Rect {
                    x: row_area.x + highlight_symbol_width,
                    width: row_area.width.saturating_sub(highlight_symbol_width),
                    ..row_area
                }
            } else {
                row_area
            };
            item.content.clone().render(item_area, buf);

            for j in 0..item.content.height() {
                // if the item is selected, we need to display the highlight symbol:
                // - either for the first line of the item only,
                // - or for each line of the item if the appropriate option is set
                let symbol = if is_selected && (j == 0 || self.repeat_highlight_symbol) {
                    highlight_symbol
                } else {
                    &blank_symbol
                };
                if selection_spacing {
                    buf.set_stringn(
                        x,
                        y + j as u16,
                        symbol,
                        list_area.width as usize,
                        item_style,
                    );
                }
            }
            if self.callback_registry.lock().unwrap().is_hovering(row_area) {
                selected_element = Some((row_area, i));
                buf.set_style(row_area, self.hovering_style);
            }

            if is_selected {
                buf.set_style(row_area, self.highlight_style);
            }
        }

        if let Some((row_area, index)) = selected_element {
            self.callback_registry
                .lock()
                .unwrap()
                .register_mouse_callback(
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                    Some(row_area),
                    UiCallbackPreset::SetPanelIndex { index },
                );
        }
    }
}

impl<'a> Widget for ClickableList<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ClickableListState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

impl<'a> Styled for ClickableList<'a> {
    type Item = ClickableList<'a>;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style.into())
    }
}
