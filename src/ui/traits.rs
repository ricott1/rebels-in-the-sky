use super::constants::UiStyle;
use super::ui_callback::CallbackRegistry;
use crate::core::resources::Resource;
use crate::core::skill::Rated;
use crate::core::{GameSkill, Kartoffel, Skill, Trait};
use crate::image::utils::Gif;
use crate::ui::utils::img_to_lines;
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{StatefulWidget, Widget},
};

pub type ImageLines = Vec<Line<'static>>;
pub type GifLines = Vec<ImageLines>;

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

impl UiStyled for Skill {
    fn style(&self) -> Style {
        match self.value() {
            x if x == 0 => Style::default().fg(Color::DarkGray),
            x if x <= 2 => Style::default().fg(Color::Red),
            x if x <= 4 => Style::default().fg(Color::LightRed),
            x if x <= 6 => Style::default().fg(Color::Yellow),
            x if x <= 8 => Style::default().fg(Color::LightYellow),
            x if x <= 10 => Style::default().fg(Color::White),
            x if x <= 12 => Style::default().fg(Color::White),
            x if x <= 14 => Style::default().fg(Color::LightGreen),
            x if x <= 16 => Style::default().fg(Color::Green),
            x if x <= 18 => Style::default().fg(Color::Cyan),
            x if x <= 20 => Style::default().fg(Color::Rgb(155, 95, 205)),
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
    fn before_rendering(
        &mut self,
        area: Rect,
        callback_registry: &mut CallbackRegistry,
        layer: usize,
    );
    fn hover_text(&self) -> Text<'_>;
}

pub trait InteractiveStatefulWidget: StatefulWidget {
    fn before_rendering(
        &self,
        area: Rect,
        callback_registry: &mut CallbackRegistry,
        state: &mut Self::State,
        layer: usize,
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
