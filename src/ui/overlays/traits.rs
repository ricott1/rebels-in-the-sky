use crate::core::world::World;
use crate::types::AppResult;
use crate::ui::dropdown::DropdownState;
use crate::ui::panels::{HelpContent, SplitPanel};
use crate::ui::ui_callback::UiCallback;
use crate::ui::ui_frame::UiFrame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

/// A panel drawn on top of the active screen.
/// The screen underneath stops claiming input.
pub trait Overlay: std::fmt::Debug {
    fn title(&self, world: &World) -> String;

    fn rect(&self, screen_area: Rect) -> Rect;

    fn update(&mut self, _world: &World) -> AppResult<()> {
        Ok(())
    }

    fn tick(&mut self) {}

    /// Interactive widgets must register on `layer`.
    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        layer: usize,
    ) -> AppResult<()>;

    /// `Some(UiCallback::None)` swallows the key,
    /// `None` lets it fall through to the button hotkeys.
    fn handle_key_events(&mut self, _key_event: KeyEvent, _world: &World) -> Option<UiCallback> {
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

    /// Takes list navigation instead of the panel underneath.
    fn as_split_panel(&mut self) -> Option<&mut dyn SplitPanel> {
        None
    }

    /// Title and body for '?'. `None` makes '?' close the overlay.
    fn help_content(&self) -> Option<(String, HelpContent)> {
        None
    }

    /// When false, Left/Right switch tab.
    fn consumes_tab_keys(&self) -> bool {
        true
    }
}
