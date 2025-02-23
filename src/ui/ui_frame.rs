use crossterm::event::{KeyCode, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::{Clear, Paragraph, StatefulWidget, Widget},
    Frame,
};

use super::{
    traits::{InteractiveStatefulWidget, InteractiveWidget},
    ui_callback::{CallbackRegistry, UiCallback},
    UI_SCREEN_SIZE,
};

pub struct UiFrame<'a, 'b> {
    inner: &'a mut Frame<'b>,
    hover_text_area: Rect,
    callback_registry: CallbackRegistry,
}

impl<'a, 'b> UiFrame<'a, 'b> {
    fn is_hovered(&self, rect: Rect, layer: usize) -> bool {
        self.callback_registry.is_hovering(rect) && layer == self.get_max_layer()
    }

    pub fn set_max_layer(&mut self, layer: usize) {
        self.callback_registry.set_max_layer(layer);
    }

    pub fn get_max_layer(&self) -> usize {
        self.callback_registry.get_max_layer()
    }

    pub fn register_mouse_callback(
        &mut self,
        event_kind: MouseEventKind,
        rect: Option<Rect>,
        callback: UiCallback,
    ) {
        self.callback_registry
            .register_mouse_callback(event_kind, rect, callback);
    }

    pub fn register_keyboard_callback(&mut self, key_code: KeyCode, callback: UiCallback) {
        self.callback_registry
            .register_keyboard_callback(key_code, callback);
    }

    pub fn clear(&mut self) {
        self.callback_registry.clear();
    }

    pub fn is_hovering(&self, rect: Rect) -> bool {
        self.callback_registry.is_hovering(rect)
    }

    pub fn set_hovering(&mut self, position: (u16, u16)) {
        self.callback_registry.set_hovering(position);
    }

    pub fn callback_registry(&self) -> &CallbackRegistry {
        &self.callback_registry
    }

    // Create a rect with the correct coordinates relative to the centered screen.
    pub fn to_screen_rect(&self, rect: Rect) -> Rect {
        let screen_area = self.screen_area();
        Rect::new(
            rect.x + screen_area.x,
            rect.y + screen_area.y,
            rect.width,
            rect.height,
        )
    }

    pub fn screen_area(&self) -> Rect {
        // If area is bigger than UI_SCREEN_SIZE, use a centered rect of the correct size.
        let frame_width = self.inner.area().width;
        let frame_height = self.inner.area().height;
        let (target_width, target_height) = UI_SCREEN_SIZE;
        Rect::new(
            frame_width.saturating_sub(target_width) / 2,
            frame_height.saturating_sub(target_height) / 2,
            target_width.min(frame_width),
            target_height.min(frame_height),
        )
    }

    pub fn new(frame: &'a mut Frame<'b>) -> UiFrame<'a, 'b> {
        let mut ui_frame = Self {
            inner: frame,
            hover_text_area: Rect::default(),
            callback_registry: CallbackRegistry::new(),
        };

        let screen_area = ui_frame.screen_area();
        let split = Layout::vertical([
            Constraint::Min(6),    // body
            Constraint::Length(1), // footer
            Constraint::Length(1), // hover text
        ])
        .split(screen_area);
        ui_frame.hover_text_area = split[2];
        ui_frame
    }

    pub const fn area(&self) -> Rect {
        self.inner.area()
    }

    pub fn render_widget<W: Widget>(&mut self, widget: W, area: Rect) {
        self.inner.render_widget(widget, area);
    }

    pub fn render_stateful_widget<W>(&mut self, widget: W, area: Rect, state: &mut W::State)
    where
        W: StatefulWidget,
    {
        self.inner.render_stateful_widget(widget, area, state);
    }

    pub fn render_interactive<W>(&mut self, mut widget: W, area: Rect)
    where
        W: InteractiveWidget,
    {
        let is_hovered = self.is_hovered(area, widget.layer());
        widget.before_rendering(area, &mut self.callback_registry);
        if is_hovered {
            self.render_widget(Clear, self.hover_text_area);

            let hover_text = Paragraph::new(widget.hover_text()).centered();
            self.render_widget(hover_text, self.hover_text_area);
        }
        self.render_widget(widget, area);
    }

    pub fn render_stateful_interactive<W>(
        &mut self,
        mut widget: W,
        area: Rect,
        state: &mut W::State,
    ) where
        W: InteractiveStatefulWidget,
    {
        let is_hovered = self.is_hovered(area, widget.layer());
        widget.before_rendering(area, &mut self.callback_registry, state);
        if is_hovered {
            self.render_widget(Clear, self.hover_text_area);

            let hover_text = Paragraph::new(widget.hover_text()).centered();
            self.render_widget(hover_text, self.hover_text_area);
        }
        self.render_stateful_widget(widget, area, state);
    }
}
