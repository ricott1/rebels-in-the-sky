use super::constants::UiStyle;
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::ui_frame::UiFrame;
use crate::core::resources::Resource;
use crate::core::world::World;
use crate::core::{Kartoffel, Trait};
use crate::image::utils::Gif;
use crate::ui::utils::{img_to_lines, normalize_index, IndexBound};
use crate::{core::skill::Rated, types::AppResult};
use ratatui::crossterm;
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{StatefulWidget, Widget},
};

pub type ImageLines = Vec<Line<'static>>;
pub type GifLines = Vec<ImageLines>;

pub trait Screen {
    fn tick(&mut self);
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

    fn render_help_widget(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        debug_view: bool,
    ) -> AppResult<()>;

    /// Returns true when the panel currently has an active text input that
    /// should receive raw character keys. Suppresses global character-key
    /// shortcuts (currently '?' for help) so the user can type those characters.
    fn is_capturing_text(&self) -> bool {
        false
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
    fn index_bound(&self) -> IndexBound {
        IndexBound::Wrap
    }
    fn previous_index(&mut self) {
        let len = self.max_index();
        let bound = self.index_bound();
        if let Some(i) = self.index() {
            if let Some(next) = normalize_index(i + 1, len, bound) {
                self.set_index(next);
            }
        }
    }
    fn next_index(&mut self) {
        let len = self.max_index();
        let bound = self.index_bound();
        if let Some(i) = self.index() {
            let raw = match bound {
                IndexBound::Wrap => (i + len).saturating_sub(1),
                IndexBound::Clamp => i.saturating_sub(1),
            };
            if let Some(next) = normalize_index(raw, len, bound) {
                self.set_index(next);
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
            Self::Killer => UiStyle::DEFAULT.fg(Color::Red),
            Self::Showpirate => UiStyle::DEFAULT.fg(Color::Magenta),
            Self::Relentless => UiStyle::DEFAULT.fg(Color::Blue),
            Self::Spugna => UiStyle::DEFAULT.fg(Color::LightRed),
            Self::Crumiro => UiStyle::DEFAULT.fg(Color::Rgb(212, 175, 55)),
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
            Self::GOLD => [240, 230, 140],
            Self::SCRAPS => [192, 192, 192],
            Self::RUM => [114, 47, 55],
            Self::FUEL => [64, 224, 208],
            Self::SATOSHI => [255, 255, 255],
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
