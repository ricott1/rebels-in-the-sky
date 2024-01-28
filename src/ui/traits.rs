use super::ui_callback::UiCallbackPreset;
use crate::types::AppResult;
use crate::world::world::World;
use core::fmt::Debug;
use ratatui::{
    prelude::{CrosstermBackend, Rect},
    text::Span,
    Frame,
};
use std::io::Stdout;

pub trait Screen {
    fn name(&self) -> &str;

    fn update(&mut self, _world: &World) -> AppResult<()>;
    fn render(
        &mut self,
        _frame: &mut Frame<CrosstermBackend<Stdout>>,
        _world: &World,
        _area: Rect,
    ) -> AppResult<()>;

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
    ) -> Option<UiCallbackPreset>;

    fn footer_spans(&self) -> Vec<Span> {
        vec![]
    }
}

impl Debug for dyn Screen {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Screen{{{:?}}}", self)
    }
}

pub trait SplitPanel {
    fn index(&self) -> usize;
    fn max_index(&self) -> usize;
    fn set_index(&mut self, index: usize);
    fn previous_index(&mut self) {
        if self.max_index() > 0 {
            let current_index = self.index();
            self.set_index((current_index + 1) % self.max_index());
        }
    }
    fn next_index(&mut self) {
        if self.max_index() > 0 {
            let current_index = self.index();
            self.set_index((current_index + self.max_index() - 1) % self.max_index());
        }
    }
}

impl Debug for dyn SplitPanel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SplitPanel{{{:?}}}", self)
    }
}
