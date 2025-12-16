use super::constants::UiStyle;
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::ui_frame::UiFrame;
use crate::world::resources::Resource;
use crate::world::world::World;
use crate::world::{Kartoffel, Trait};
use crate::{types::AppResult, world::skill::Rated};
use ratatui::text::Text;
use ratatui::widgets::{StatefulWidget, Widget};
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
};

pub trait Screen {
    fn update(&mut self, world: &World) -> AppResult<()>;
    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        debug_view: bool,
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
    fn index(&self) -> Option<usize> {
        None
    }
    fn max_index(&self) -> usize {
        0
    }
    fn set_index(&mut self, _index: usize) {}
    fn previous_index(&mut self) {
        if self.max_index() > 0 {
            if let Some(current_index) = self.index() {
                self.set_index((current_index + 1) % self.max_index());
            }
        }
    }
    fn next_index(&mut self) {
        if self.max_index() > 0 {
            if let Some(current_index) = self.index() {
                self.set_index((current_index + self.max_index() - 1) % self.max_index());
            }
        }
    }
}

pub trait UiStyled {
    fn style(&self) -> Style;
}

impl UiStyled for Trait {
    fn style(&self) -> Style {
        match self {
            Trait::Killer => UiStyle::DEFAULT.fg(Color::Red),
            Trait::Showpirate => UiStyle::DEFAULT.fg(Color::Magenta),
            Trait::Relentless => UiStyle::DEFAULT.fg(Color::Blue),
            Trait::Spugna => UiStyle::DEFAULT.fg(Color::LightRed),
            Trait::Crumiro => UiStyle::DEFAULT.fg(Color::Rgb(212, 175, 55)),
        }
    }
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
        let [r, g, b] = match self {
            Resource::GOLD => [240, 230, 140],
            Resource::SCRAPS => [192, 192, 192],
            Resource::RUM => [114, 47, 55],
            Resource::FUEL => [64, 224, 208],
            Resource::SATOSHI => [255, 255, 255],
        };

        UiStyle::DEFAULT.fg(Color::Rgb(r, g, b))
    }
}

impl UiStyled for Kartoffel {
    fn style(&self) -> Style {
        UiStyle::DEFAULT.fg(Color::Magenta)
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
