use crossterm::event::KeyCode;
use ratatui::style::{Color, Modifier, Style};

pub const LEFT_PANEL_WIDTH: u16 = 36;
pub const IMG_FRAME_WIDTH: u16 = 80;

pub struct UiKey;

impl UiKey {
    pub const NEXT_TAB: KeyCode = KeyCode::Char(']');
    pub const PREVIOUS_TAB: KeyCode = KeyCode::Char('[');
    pub const DATA_VIEW: KeyCode = KeyCode::Tab;
    pub const MUSIC_TOGGLE: KeyCode = KeyCode::Char('|');
    pub const MUSIC_NEXT: KeyCode = KeyCode::Char('>');
    pub const MUSIC_PREVIOUS: KeyCode = KeyCode::Char('<');
    pub const GO_TO_TEAM: KeyCode = KeyCode::Backspace;
    pub const GO_TO_TEAM_ALTERNATIVE: KeyCode = KeyCode::Char('t');
    pub const GO_TO_PLANET: KeyCode = KeyCode::Char('p');
    pub const CHALLENGE_TEAM: KeyCode = KeyCode::Char('c');
    pub const TRAINING_FOCUS: KeyCode = KeyCode::Char('f');
    pub const AUTO_ASSIGN: KeyCode = KeyCode::Char('a');
    pub const SET_TACTIC: KeyCode = KeyCode::Char('t');
    pub const CYCLE_FILTER: KeyCode = KeyCode::Char('=');
    pub const HIRE_FIRE: KeyCode = KeyCode::Char('s');
    pub const LOCK_PLAYER: KeyCode = KeyCode::Char('l');
    pub const UNLOCK_PLAYER: KeyCode = KeyCode::Char('u');
    pub const SET_CAPTAIN: KeyCode = KeyCode::Char('c');
    pub const SET_DOCTOR: KeyCode = KeyCode::Char('d');
    pub const SET_PILOT: KeyCode = KeyCode::Char('e');
    pub const PITCH_VIEW: KeyCode = KeyCode::Char('v');
    pub const TRAVEL: KeyCode = KeyCode::Char('t');
    pub const EXPLORE: KeyCode = KeyCode::Char('x');
    pub const BUY_FOOD: KeyCode = KeyCode::Char('o');
    pub const BUY_FUEL: KeyCode = KeyCode::Char('u');
    pub const BUY_GOLD: KeyCode = KeyCode::Char('g');
    pub const BUY_RUM: KeyCode = KeyCode::Char('r');
    pub const SELL_FOOD: KeyCode = KeyCode::Char('O');
    pub const SELL_FUEL: KeyCode = KeyCode::Char('U');
    pub const SELL_GOLD: KeyCode = KeyCode::Char('G');
    pub const SELL_RUM: KeyCode = KeyCode::Char('R');
}
pub trait PrintableKeyCode {
    fn to_string(&self) -> String;
}

impl PrintableKeyCode for KeyCode {
    fn to_string(&self) -> String {
        match self {
            KeyCode::Char(c) => format!("{}", c),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "BackTab".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Insert => "Insert".to_string(),
            KeyCode::F(u) => format!("F{}", u),
            KeyCode::Null => "Null".to_string(),
            KeyCode::Modifier(c) => format!("{:?}", c),
            KeyCode::CapsLock => "CapsLock".to_string(),
            KeyCode::ScrollLock => "ScrollLock".to_string(),
            KeyCode::NumLock => "NumLock".to_string(),
            KeyCode::PrintScreen => "PrintScreen".to_string(),
            KeyCode::Pause => "Pause".to_string(),
            KeyCode::Menu => "Menu".to_string(),
            KeyCode::KeypadBegin => "KeypadBegin".to_string(),
            KeyCode::Media(_) => "Media".to_string(),
        }
    }
}

const DEFAULT_STYLE: Style = Style {
    fg: None,
    bg: None,
    underline_color: None,
    add_modifier: Modifier::empty(),
    sub_modifier: Modifier::empty(),
};

pub struct UiStyle;

impl UiStyle {
    pub const DEFAULT: Style = DEFAULT_STYLE;
    pub const UNSELECTED: Style = DEFAULT_STYLE;
    pub const SELECTED: Style = DEFAULT_STYLE.fg(Color::Black).bg(Color::Rgb(244, 255, 232));
    pub const UNSELECTABLE: Style = DEFAULT_STYLE.fg(Color::DarkGray);
    pub const ERROR: Style = DEFAULT_STYLE.fg(Color::Red);
    pub const OWN_TEAM: Style = DEFAULT_STYLE.fg(Color::Green);
    pub const HEADER: Style = DEFAULT_STYLE.fg(Color::LightBlue);
    pub const NETWORK: Style = DEFAULT_STYLE.fg(Color::Rgb(244, 123, 123));
    pub const DISCONNECTED: Style = DEFAULT_STYLE.fg(Color::DarkGray);
    pub const FANCY: Style = DEFAULT_STYLE.fg(Color::Rgb(244, 255, 232));
    pub const HIGHLIGHT: Style = DEFAULT_STYLE.fg(Color::Rgb(118, 213, 192));
    pub const OK: Style = DEFAULT_STYLE.fg(Color::Green);
    pub const WARNING: Style = DEFAULT_STYLE.fg(Color::Yellow);
}

pub struct UiText;

impl UiText {
    pub const YES: &'static str = "Ayay";
    pub const NO: &'static str = "Nay!";
}
