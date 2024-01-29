use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use ratatui::{
    buffer::Buffer,
    layout::{Corner, Rect},
    style::{Style, Styled},
    text::Text,
    widgets::{Block, HighlightSpacing, StatefulWidget, Widget},
};
use std::{cell::RefCell, rc::Rc};
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

#[derive(Debug, PartialEq)]
pub struct ClickableList<'a> {
    block: Option<Block<'a>>,
    items: Vec<ClickableListItem<'a>>,
    callback_registry: Rc<RefCell<CallbackRegistry>>,
    /// Style used as a base style for the widget
    style: Style,
    start_corner: Corner,
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
}

impl<'a> ClickableList<'a> {
    pub fn new<T>(items: T, callback_registry: Rc<RefCell<CallbackRegistry>>) -> ClickableList<'a>
    where
        T: Into<Vec<ClickableListItem<'a>>>,
    {
        ClickableList {
            block: None,
            style: Style::default(),
            items: items.into(),
            callback_registry,
            start_corner: Corner::TopLeft,
            highlight_style: Style::default(),
            hovering_style: Style::default(),
            highlight_symbol: None,
            repeat_highlight_symbol: false,
            highlight_spacing: HighlightSpacing::default(),
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

    /// Set when to show the highlight spacing
    ///
    /// See [`HighlightSpacing`] about which variant affects spacing in which way
    pub fn highlight_spacing(mut self, value: HighlightSpacing) -> Self {
        self.highlight_spacing = value;
        self
    }

    pub fn start_corner(mut self, corner: Corner) -> ClickableList<'a> {
        self.start_corner = corner;
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

impl<'a> StatefulWidget for ClickableList<'a> {
    type State = ClickableListState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(area, self.style);
        let list_area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        if list_area.width < 1 || list_area.height < 1 {
            return;
        }

        if self.items.is_empty() {
            return;
        }
        let list_height = list_area.height as usize;

        let (start, end) = self.get_items_bounds(state.selected, state.offset, list_height);
        state.offset = start;

        let highlight_symbol = self.highlight_symbol.unwrap_or("");
        let blank_symbol = " ".repeat(highlight_symbol.width());

        let mut current_height = 0;
        let selection_spacing = state.selected.is_some();

        let mut selected_element: Option<(Rect, usize)> = None;
        for (i, item) in self
            .items
            .iter_mut()
            .enumerate()
            .skip(state.offset)
            .take(end - start)
        {
            let (x, y) = if self.start_corner == Corner::BottomLeft {
                current_height += item.height() as u16;
                (list_area.left(), list_area.bottom() - current_height)
            } else {
                let pos = (list_area.left(), list_area.top() + current_height);
                current_height += item.height() as u16;
                pos
            };
            let area = Rect {
                x,
                y,
                width: list_area.width,
                height: item.height() as u16,
            };

            let item_style = self.style.patch(item.style);
            buf.set_style(area, item_style);

            let is_selected = state.selected.map_or(false, |s| s == i);
            for (j, line) in item.content.lines.iter().enumerate() {
                // if the item is selected, we need to display the highlight symbol:
                // - either for the first line of the item only,
                // - or for each line of the item if the appropriate option is set
                let symbol = if is_selected && (j == 0 || self.repeat_highlight_symbol) {
                    highlight_symbol
                } else {
                    &blank_symbol
                };
                let (elem_x, max_element_width) = if selection_spacing {
                    let (elem_x, _) = buf.set_stringn(
                        x,
                        y + j as u16,
                        symbol,
                        list_area.width as usize,
                        item_style,
                    );
                    (elem_x, (list_area.width - (elem_x - x)))
                } else {
                    (x, list_area.width)
                };
                buf.set_line(elem_x, y + j as u16, line, max_element_width);
            }
            if self.callback_registry.borrow().is_hovering(area) {
                selected_element = Some((area, i));
                buf.set_style(area, self.hovering_style);
            }
            if is_selected {
                buf.set_style(area, self.highlight_style);
            }
        }
        if let Some((area, index)) = selected_element {
            self.callback_registry.borrow_mut().register_callback(
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                Some(area),
                UiCallbackPreset::SetPanelIndex { index },
            );
        }

        self.callback_registry.borrow_mut().register_callback(
            crossterm::event::MouseEventKind::ScrollDown,
            None,
            UiCallbackPreset::NextPanelIndex,
        );

        self.callback_registry.borrow_mut().register_callback(
            crossterm::event::MouseEventKind::ScrollUp,
            None,
            UiCallbackPreset::PreviousPanelIndex,
        );
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

    fn set_style(self, style: Style) -> Self::Item {
        self.style(style)
    }
}
