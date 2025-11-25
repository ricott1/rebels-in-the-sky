use super::constants::UiStyle;
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::ui_frame::UiFrame;
use crate::world::resources::Resource;
use crate::world::world::World;
use crate::{types::AppResult, world::skill::Rated};
use ratatui::text::Text;
use ratatui::widgets::{StatefulWidget, Widget};
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
};

pub trait Screen {
    fn update(&mut self, _world: &World) -> AppResult<()>;
    fn render(
        &mut self,
        _frame: &mut UiFrame,
        _world: &World,
        _area: Rect,
        _debug_view: bool,
    ) -> AppResult<()>;

    fn handle_key_events(
        &mut self,
        _key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallback> {
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![]
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

pub trait UiStyled {
    fn style(&self) -> Style;
}

impl UiStyled for f32 {
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
impl UiStyled for u8 {
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

impl UiStyled for Resource {
    fn style(&self) -> Style {
        let [r, g, b, _] = self.color().0;
        UiStyle::DEFAULT.fg(Color::Rgb(r, g, b))
    }
}

pub trait PercentageRating: Rated {
    fn percentage(&self) -> u8;
}

impl PercentageRating for f32 {
    fn percentage(&self) -> u8 {
        (5.0 * self) as u8
    }
}

pub trait InteractiveWidget: Widget {
    fn layer(&self) -> usize;
    fn before_rendering(&mut self, area: Rect, callback_registry: &mut CallbackRegistry);
    fn hover_text(&self) -> Text<'_>;
}

pub trait InteractiveStatefulWidget: StatefulWidget {
    fn layer(&self) -> usize;
    fn before_rendering(
        &mut self,
        area: Rect,
        callback_registry: &mut CallbackRegistry,
        state: &mut Self::State,
    );
    fn hover_text(&self) -> Text<'_>;
}
