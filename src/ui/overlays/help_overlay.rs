use super::{centered_rect, Overlay};
use crate::core::world::World;
use crate::types::AppResult;
use crate::ui::button::Button;
use crate::ui::constants::{UiStyle, UiText};
use crate::ui::panels::{render_help_content, HelpContent};
use crate::ui::renders::default_block;
use crate::ui::ui_callback::UiCallback;
use crate::ui::ui_frame::UiFrame;
use crate::ui::ui_key;
use ratatui::layout::{Constraint, Layout, Rect};

/// Proportions of the help overlay: ~60% wide, ~80% tall, never below 50x20.
const HELP_OVERLAY_RECT: (u16, u16, (u16, u16)) = (60, 80, (50, 20));

#[derive(Debug)]
pub struct HelpOverlay {
    title: String,
    content: HelpContent,
}

impl HelpOverlay {
    pub fn new(title: String, content: HelpContent) -> Self {
        Self { title, content }
    }
}

impl Overlay for HelpOverlay {
    fn title(&self, _world: &World) -> String {
        format!("Help - {}", self.title)
    }

    fn rect(&self, screen_area: Rect) -> Rect {
        let (pct_w, pct_h, min) = HELP_OVERLAY_RECT;
        centered_rect(screen_area, pct_w, pct_h, min)
    }

    fn render(
        &mut self,
        frame: &mut UiFrame,
        _world: &World,
        area: Rect,
        layer: usize,
    ) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(area);

        render_help_content(frame, split[0], &self.content, layer);

        let button_split = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(20),
            Constraint::Min(0),
        ])
        .split(split[1]);
        let close_button = Button::new(UiText::YES, UiCallback::CloseOverlay)
            .hover_text("Close help")
            .hotkey(ui_key::YES_TO_DIALOG)
            .block(default_block().border_style(UiStyle::OK));
        frame.render_interactive_widget_on_layer(close_button, button_split[1], layer);

        Ok(())
    }

    fn help_content(&self) -> Option<(String, HelpContent)> {
        None
    }

    fn consumes_tab_keys(&self) -> bool {
        false
    }
}
