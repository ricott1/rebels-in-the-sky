#![allow(dead_code)]
use crate::ui::constants::UiStyle;
use crate::ui::traits::InteractiveStatefulWidget;
use crate::ui::ui_callback::{CallbackRegistry, UiCallback};
use ratatui::crossterm;
use ratatui::crossterm::event::KeyCode;
use ratatui::widgets::StatefulWidget;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Block, Clear, Paragraph, Widget},
};

const OPEN_LAYER: usize = 1;
const CLOSED_MARKER: char = '▾';
const OPEN_MARKER: char = '▴';

type DropdownCallback = Box<dyn Fn(usize) -> UiCallback>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OpenDirection {
    #[default]
    Down,
    Up,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DropdownState {
    selected: usize,
    is_open: bool,
    hovered: Rect,
}

impl DropdownState {
    pub fn new(selected: usize) -> Self {
        Self {
            selected,
            ..Default::default()
        }
    }

    pub const fn selected(&self) -> usize {
        self.selected
    }

    pub const fn is_open(&self) -> bool {
        self.is_open
    }

    pub const fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub const fn close(&mut self) {
        self.is_open = false;
    }

    pub const fn select(&mut self, index: usize) {
        self.selected = index;
        self.is_open = false;
    }
}

pub struct Dropdown<'a> {
    id: usize,
    options: Vec<Text<'a>>,
    on_select: DropdownCallback,
    hotkey: Option<KeyCode>,
    open_direction: OpenDirection,
    block: Option<Block<'a>>,
    style: Style,
    selected_style: Style,
    hover_style: Style,
    layer: usize,
    title: Option<String>,
    disabled: bool,
    hover_text: Text<'a>,
}

impl<'a> Dropdown<'a> {
    pub fn new(id: usize, options: Vec<Text<'a>>, on_select: DropdownCallback) -> Self {
        Self {
            id,
            on_select,
            options,
            hotkey: None,
            open_direction: OpenDirection::Down,
            block: None,
            style: UiStyle::DEFAULT,
            selected_style: UiStyle::SELECTED,
            hover_style: UiStyle::HIGHLIGHT,
            layer: 0,
            title: None,
            disabled: false,
            hover_text: Text::default(),
        }
    }

    pub const fn hotkey(mut self, key: KeyCode) -> Self {
        self.hotkey = Some(key);
        self
    }

    pub const fn layer(mut self, layer: usize) -> Self {
        self.layer = layer;
        self
    }

    pub const fn open_direction(mut self, direction: OpenDirection) -> Self {
        self.open_direction = direction;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn set_hover_text(mut self, hover_text: impl Into<Text<'a>>) -> Self {
        self.hover_text = hover_text.into();
        self
    }

    fn styled_title(&self, title: &str) -> Line<'static> {
        super::underline_hotkey(title, self.hotkey)
    }

    fn full_rect(&self, area: Rect) -> Rect {
        // top border + header + one row per option + bottom border
        let n = self.options.len() as u16;
        let y = match self.open_direction {
            OpenDirection::Down => area.y,
            OpenDirection::Up => area.y.saturating_sub(n),
        };
        Rect::new(area.x, y, area.width, n.saturating_add(3))
    }

    fn row_rect(&self, area: Rect, index: usize) -> Rect {
        // Rows sit inside the borders (x + 1, width - 2) and below the header row.
        let n = self.options.len() as u16;
        let i = index as u16;
        let y = match self.open_direction {
            OpenDirection::Down => area.y.saturating_add(2).saturating_add(i),
            OpenDirection::Up => area.y.saturating_sub(n).saturating_add(1).saturating_add(i),
        };
        Rect::new(area.x.saturating_add(1), y, area.width.saturating_sub(2), 1)
    }

    fn inner_area(&self, area: Rect) -> Rect {
        self.block.as_ref().map_or(area, |block| block.inner(area))
    }

    fn hovered_row(&self, area: Rect, registry: &CallbackRegistry) -> Option<(Rect, usize)> {
        (0..self.options.len())
            .map(|i| (self.row_rect(area, i), i))
            .find(|(row, _)| registry.is_hovering(*row))
    }
}

impl StatefulWidget for &Dropdown<'_> {
    type State = DropdownState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let box_area = if state.is_open {
            self.full_rect(area)
        } else {
            area
        };
        Clear.render(box_area, buf);

        if state.is_open {
            for (i, option) in self.options.iter().enumerate() {
                let row_style = if i == state.selected {
                    self.selected_style
                } else {
                    self.style
                };
                Paragraph::new(option.clone())
                    .style(row_style)
                    .render(self.row_rect(area, i), buf);
            }

            if state.hovered.width > 0 && state.hovered.height > 0 {
                buf.set_style(state.hovered, self.hover_style);
            }
        }

        if let Some(mut block) = self.block.clone() {
            if let Some(title) = &self.title {
                block = block.title(self.styled_title(title));
            }
            block.render(box_area, buf);
        }

        let header = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1);
        let is_closed_and_hovered = !state.is_open && state.hovered == area;
        if header.width > 0 {
            let marker = if state.is_open {
                OPEN_MARKER
            } else {
                CLOSED_MARKER
            };
            let style = if self.disabled {
                UiStyle::UNSELECTABLE
            } else if state.is_open {
                self.selected_style
            } else if is_closed_and_hovered {
                self.hover_style
            } else {
                self.style
            };

            let label = self
                .options
                .get(state.selected)
                .cloned()
                .unwrap_or_default();
            let label_width = header.width.saturating_sub(1);
            Paragraph::new(label)
                .style(style)
                .render(Rect::new(header.x, header.y, label_width, 1), buf);
            let marker_style = if self.disabled {
                UiStyle::UNSELECTABLE
            } else {
                self.selected_style
            };
            Paragraph::new(Line::from(marker.to_string()))
                .style(marker_style)
                .render(Rect::new(header.x + label_width, header.y, 1, 1), buf);
        }
    }
}

impl StatefulWidget for Dropdown<'_> {
    type State = DropdownState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buf, state);
    }
}
impl InteractiveStatefulWidget for &Dropdown<'_> {
    fn layer(&self) -> usize {
        self.layer
    }

    fn before_rendering(
        &self,
        area: Rect,
        callback_registry: &mut CallbackRegistry,
        state: &mut Self::State,
    ) {
        state.hovered = Rect::default();

        if self.inner_area(area).area() == 0 || self.options.is_empty() {
            return;
        }

        if self.disabled {
            state.close();
            return;
        }

        if callback_registry.get_active_layer() != self.layer() {
            return;
        }

        // Hotkey fires whenever this dropdown's layer is active, not only on hover.
        if let Some(key) = self.hotkey {
            let next = (state.selected + 1) % self.options.len();
            callback_registry.register_keyboard_callback(
                key,
                UiCallback::SelectDropdown {
                    id: self.id,
                    index: next,
                    on_select: Some(Box::new((self.on_select)(next))),
                },
            );
        }

        let hit_area = if state.is_open {
            self.full_rect(area)
        } else {
            area
        };
        let is_hovered = callback_registry.is_hovering(hit_area);

        if !is_hovered && !state.is_open {
            return;
        }

        if is_hovered && !state.is_open {
            state.hovered = area;
        }

        if is_hovered {
            // When open, only the header row toggles (closes) so it doesn't
            // collide with the first option row.
            let toggle_rect = if state.is_open {
                Rect::new(area.x, area.y + 1, area.width, 1)
            } else {
                area
            };
            callback_registry.register_mouse_callback(
                crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                Some(toggle_rect),
                UiCallback::ToggleDropdown { id: self.id },
            );
        }

        if state.is_open {
            if let Some((row, index)) = self.hovered_row(area, callback_registry) {
                if state.selected != index {
                    state.hovered = row;
                }
                callback_registry.register_mouse_callback(
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                    Some(row),
                    UiCallback::SelectDropdown {
                        id: self.id,
                        index,
                        on_select: Some(Box::new((self.on_select)(index))),
                    },
                );
            }
        }
    }

    fn hover_text(&self) -> Text<'_> {
        self.hover_text.clone()
    }
}

impl InteractiveStatefulWidget for Dropdown<'_> {
    fn layer(&self) -> usize {
        self.layer
    }

    fn hover_text(&self) -> Text<'_> {
        self.hover_text.clone()
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    fn options() -> Vec<Text<'static>> {
        vec![Text::from("Alpha"), Text::from("Beta"), Text::from("Gamma")]
    }

    fn row_string(buf: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol().to_string()))
            .collect()
    }

    fn left_click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[test]
    fn state_toggle_and_select() {
        let mut state = DropdownState::new(1);
        assert_eq!(state.selected(), 1);
        assert!(!state.is_open());

        state.toggle();
        assert!(state.is_open());

        state.select(2);
        assert_eq!(state.selected(), 2);
        assert!(!state.is_open(), "selecting closes the dropdown");
    }
}
