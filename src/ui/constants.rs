use ratatui::style::{Color, Modifier, Style};

use crate::core::{MAX_SKILL_INCREASE_PER_LONG_TICK, SKILL_DECREMENT_PER_LONG_TICK};

pub const UI_SCREEN_SIZE: (u16, u16) = (160, 48);

pub const LEFT_PANEL_WIDTH: u16 = 36;
pub const IMG_FRAME_WIDTH: u16 = 80;
pub const MIN_NAME_LENGTH: usize = 3;
pub const MAX_NAME_LENGTH: usize = 12;

pub const BARS_LENGTH: usize = 25;

// Used for improvement indicators
pub const TREND_WEAK_UP: f32 = 0.05 * MAX_SKILL_INCREASE_PER_LONG_TICK;
pub const TREND_STRONG_UP: f32 = 0.12 * MAX_SKILL_INCREASE_PER_LONG_TICK;
pub const TREND_WEAK_DOWN: f32 = 1.4 * -SKILL_DECREMENT_PER_LONG_TICK;
pub const TREND_STRONG_DOWN: f32 = 2.2 * -SKILL_DECREMENT_PER_LONG_TICK;

pub struct UiStyle;

impl UiStyle {
    pub const DEFAULT: Style = Style {
        fg: None,
        bg: None,
        underline_color: None,
        add_modifier: Modifier::empty(),
        sub_modifier: Modifier::empty(),
    };
    pub const SELECTED: Style = Self::DEFAULT.bg(Color::Rgb(70, 70, 86));
    pub const SELECTED_BUTTON: Style = Self::DEFAULT.fg(Color::Rgb(118, 213, 192));
    pub const UNSELECTABLE: Style = Self::DEFAULT.fg(Color::DarkGray);
    pub const ERROR: Style = Self::DEFAULT.fg(Color::Red);
    pub const OWN_TEAM: Style = Self::DEFAULT.fg(Color::Rgb(185, 225, 125));
    pub const HEADER: Style = Self::DEFAULT.fg(Color::LightBlue);
    pub const NETWORK: Style = Self::DEFAULT.fg(Color::Rgb(204, 144, 184));
    pub const DISCONNECTED: Style = Self::DEFAULT.fg(Color::DarkGray);
    pub const SHADOW: Style = Self::DEFAULT.fg(Color::Rgb(244, 255, 232));
    pub const HIGHLIGHT: Style = Self::DEFAULT.fg(Color::Rgb(118, 213, 192));
    pub const OK: Style = Self::DEFAULT.fg(Color::Green);
    pub const WARNING: Style = Self::DEFAULT.fg(Color::Yellow);
    pub const SHIELD: Style = Self::DEFAULT.fg(Color::LightMagenta);
    pub const HELP_LINK: Style = Self::DEFAULT
        .fg(Color::Rgb(255, 165, 0))
        .add_modifier(Modifier::UNDERLINED);
}

pub struct UiText;

impl UiText {
    pub const YES: &'static str = "Ayay";
    pub const NO: &'static str = "Nay!";
}
