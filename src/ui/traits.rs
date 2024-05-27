use super::ui_callback::UiCallbackPreset;
use crate::world::world::World;
use crate::{types::AppResult, world::skill::Rated};
use core::fmt::Debug;
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    text::Span,
    Frame,
};

pub trait Screen {
    fn name(&self) -> &str;

    fn update(&mut self, _world: &World) -> AppResult<()>;
    fn render(&mut self, _frame: &mut Frame, _world: &World, _area: Rect) -> AppResult<()>;

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        world: &World,
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

pub trait StyledRating: Rated {
    fn style(&self) -> Style {
        match self.rating() {
            0 => Style::default().fg(Color::DarkGray),
            1..=2 => Style::default().fg(Color::Red),
            3..=4 => Style::default().fg(Color::LightRed),
            5..=6 => Style::default().fg(Color::Yellow),
            7..=8 => Style::default().fg(Color::LightYellow),
            9..=10 => Style::default().fg(Color::White),
            11..=12 => Style::default().fg(Color::White),
            13..=14 => Style::default().fg(Color::LightGreen),
            15..=16 => Style::default().fg(Color::Green),
            17..=18 => Style::default().fg(Color::Cyan),
            19..=20 => Style::default().fg(Color::LightBlue),
            _ => panic!("Invalid rating"),
        }
    }
}

impl StyledRating for f32 {}
impl StyledRating for u8 {}

pub trait PercentageRating: Rated {
    fn percentage(&self) -> u8;
}

impl PercentageRating for f32 {
    fn percentage(&self) -> u8 {
        (5.0 * self) as u8
    }
}
