use super::constants::UiStyle;
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::ui_frame::UiFrame;
use crate::core::resources::Resource;
use crate::core::world::World;
use crate::core::{Kartoffel, Trait};
use crate::image::utils::Gif;
use crate::ui::utils::img_to_lines;
use crate::{core::skill::Rated, types::AppResult};
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{StatefulWidget, Widget},
};

pub type ImageLines = Vec<Line<'static>>;
pub type GifLines = Vec<ImageLines>;

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
            0.0 => Style::default().fg(Color::DarkGray),
            x if x <= 2.0 => Style::default().fg(Color::Red),
            x if x <= 4.0 => Style::default().fg(Color::LightRed),
            x if x <= 6.0 => Style::default().fg(Color::Yellow),
            x if x <= 8.0 => Style::default().fg(Color::LightYellow),
            x if x <= 10.0 => Style::default().fg(Color::White),
            x if x <= 12.0 => Style::default().fg(Color::White),
            x if x <= 14.0 => Style::default().fg(Color::LightGreen),
            x if x <= 16.0 => Style::default().fg(Color::Green),
            x if x <= 18.0 => Style::default().fg(Color::Cyan),
            x if x <= 20.0 => Style::default().fg(Color::Rgb(155, 95, 205)),
            _ => Style::default().fg(Color::Rgb(155, 95, 205)), // To support TeamBonus large than MaxSkill
        }
    }
}
impl UiStyled for u8 {
    fn style(&self) -> Style {
        self.rating().style()
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
        &self,
        area: Rect,
        callback_registry: &mut CallbackRegistry,
        state: &mut Self::State,
    );
    fn hover_text(&self) -> Text<'_>;
}

pub trait PrintableGif: Sized {
    fn to_lines(&self) -> GifLines;
}

impl PrintableGif for Gif {
    fn to_lines(&self) -> GifLines {
        self.iter().map(img_to_lines).collect()
    }
}
