use crate::core::world::World;
use crate::types::AppResult;
use crate::ui::dropdown::DropdownState;
use crate::ui::panels::{HelpContent, SplitPanel};
use crate::ui::ui_callback::UiCallback;
use crate::ui::ui_frame::UiFrame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

/// A panel drawn on top of the active screen, owning its own state.
///
/// The screen underneath keeps rendering but stops claiming input: widgets only
/// register callbacks on the layer that is currently active, and an overlay sits
/// above layer 0.
pub trait Overlay: std::fmt::Debug {
    /// Text for the overlay's header bar. The chrome itself is drawn by UiScreen.
    fn title(&self, world: &World) -> String;

    /// Where the overlay sits inside the already-centered screen area.
    fn rect(&self, screen_area: Rect) -> Rect;

    fn update(&mut self, _world: &World) -> AppResult<()> {
        Ok(())
    }

    fn tick(&mut self) {}

    /// Draws the body, below the header. Every interactive widget inside must
    /// register on `layer`, or it will be inert.
    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        layer: usize,
    ) -> AppResult<()>;

    /// Consulted before the overlay's own button hotkeys. Return
    /// `Some(UiCallback::None)` to swallow a key without doing anything: plain
    /// `None` falls through to the callback registry and fires whatever button
    /// happens to own that key.
    fn handle_key_events(
        &mut self,
        _key_event: KeyEvent,
        _world: &World,
    ) -> Option<UiCallback> {
        None
    }

    fn is_capturing_text(&self) -> bool {
        false
    }

    fn dropdown(&mut self, _id: usize) -> Option<&mut DropdownState> {
        None
    }

    fn has_open_dropdown(&self) -> Option<usize> {
        None
    }

    /// Routes list navigation into the overlay instead of the panel underneath.
    fn as_split_panel(&mut self) -> Option<&mut dyn SplitPanel> {
        None
    }

    /// Title and body for '?' while this overlay is on top. `None` means '?'
    /// closes it instead, which is what the help overlay itself wants.
    fn help_content(&self) -> Option<(String, HelpContent)> {
        None
    }

    /// When false, Left/Right fall through and switch tab. The help overlay says
    /// false so tabs stay live over it, as they always have.
    fn consumes_tab_keys(&self) -> bool {
        true
    }
}
