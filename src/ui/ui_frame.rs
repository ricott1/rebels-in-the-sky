use ratatui::crossterm::event::{KeyCode, MouseEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    text::Text,
    widgets::{Clear, Paragraph, StatefulWidget, Widget},
    Frame,
};

use super::{
    traits::{InteractiveStatefulWidget, InteractiveWidget},
    ui_callback::{CallbackRegistry, UiCallback},
    UI_SCREEN_SIZE,
};

type DeferredRender = Box<dyn FnOnce(&mut Buffer)>;

pub struct UiFrame<'a, 'b> {
    inner: &'a mut Frame<'b>,
    hover_text_area: Rect,
    callback_registry: CallbackRegistry,
    layered: Vec<DeferredRender>,
}

impl<'a, 'b> UiFrame<'a, 'b> {
    const fn is_hovered(&self, rect: Rect, layer: usize) -> bool {
        self.callback_registry.is_hovering(rect) && layer == self.get_active_layer()
    }

    pub const fn set_active_layer(&mut self, layer: usize) {
        self.callback_registry.set_active_layer(layer);
    }

    pub const fn get_active_layer(&self) -> usize {
        self.callback_registry.get_active_layer()
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

    pub const fn is_hovering(&self, rect: Rect) -> bool {
        self.callback_registry.is_hovering(rect)
    }

    pub const fn set_hovering(&mut self, position: (u16, u16)) {
        self.callback_registry.set_hovering(position);
    }

    pub const fn callback_registry(&self) -> &CallbackRegistry {
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
            layered: Vec::new(),
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

    fn draw_hover_text(&mut self, text: Text<'_>) {
        self.render_widget(Clear, self.hover_text_area);
        self.render_widget(Paragraph::new(text).centered(), self.hover_text_area);
    }

    pub fn render_interactive_widget<W>(&mut self, widget: W, area: Rect)
    where
        W: InteractiveWidget,
    {
        self.render_interactive_widget_on_layer(widget, area, 0);
    }

    pub fn render_interactive_widget_on_layer<W>(&mut self, widget: W, area: Rect, layer: usize)
    where
        W: InteractiveWidget,
    {
        let is_hovered = self.is_hovered(area, layer);
        let mut widget = widget;
        widget.before_rendering(area, &mut self.callback_registry, layer);
        if is_hovered {
            self.draw_hover_text(widget.hover_text());
        }
        self.render_widget(widget, area);
    }

    pub fn render_stateful_interactive_widget<W>(
        &mut self,
        widget: W,
        area: Rect,
        state: &mut W::State,
    ) where
        W: InteractiveStatefulWidget,
    {
        let is_hovered = self.is_hovered(area, 0);
        widget.before_rendering(area, &mut self.callback_registry, state, 0);
        if is_hovered {
            self.draw_hover_text(widget.hover_text());
        }
        self.render_stateful_widget(widget, area, state);
    }

    // Defers the widget rendering to the end of the render cycle, so that it can
    // draw over content rendered after it in the current pass.
    pub fn render_layered_stateful_interactive_widget<W>(
        &mut self,
        widget: W,
        area: Rect,
        state: &mut W::State,
        layer: usize,
    ) where
        W: InteractiveStatefulWidget + 'static,
        W::State: Clone + 'static,
    {
        let is_hovered = self.is_hovered(area, layer);
        widget.before_rendering(area, &mut self.callback_registry, state, layer);
        if is_hovered {
            self.draw_hover_text(widget.hover_text());
        }
        if layer == 0 {
            self.render_stateful_widget(widget, area, state);
        } else {
            let mut state = state.clone();
            self.layered.push(Box::new(move |buffer| {
                StatefulWidget::render(widget, area, buffer, &mut state)
            }));
        }
    }

    pub fn render_layered_widgets(&mut self) {
        let layered = std::mem::take(&mut self.layered);
        let buffer = self.inner.buffer_mut();
        for render in layered {
            render(buffer);
        }
    }
}
