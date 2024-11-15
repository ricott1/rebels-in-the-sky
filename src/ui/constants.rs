use crate::world::position::Position;
use crossterm::event::KeyCode;
use ratatui::style::{Color, Modifier, Style};

pub const UI_SCREEN_SIZE: (u16, u16) = (160, 48);

pub const LEFT_PANEL_WIDTH: u16 = 36;
pub const IMG_FRAME_WIDTH: u16 = 80;
pub const MIN_NAME_LENGTH: usize = 3;
pub const MAX_NAME_LENGTH: usize = 12;

pub const BARS_LENGTH: usize = 25;

#[derive(Debug, Clone, Copy)]
pub struct UiKey;

impl UiKey {
    pub const ESC: KeyCode = KeyCode::Esc;
    pub const NEXT_TAB: KeyCode = KeyCode::Right;
    pub const PREVIOUS_TAB: KeyCode = KeyCode::Left;
    pub const NEXT_SELECTION: KeyCode = KeyCode::Char(']');
    pub const PREVIOUS_SELECTION: KeyCode = KeyCode::Char('[');
    pub const UI_DEBUG_MODE: KeyCode = KeyCode::Tab;
    pub const TOGGLE_AUDIO: KeyCode = KeyCode::Char('|');
    pub const PREVIOUS_RADIO: KeyCode = KeyCode::Char('<');
    pub const NEXT_RADIO: KeyCode = KeyCode::Char('>');
    pub const GO_TO_TEAM: KeyCode = KeyCode::Backspace;
    pub const GO_TO_TEAM_ALTERNATIVE: KeyCode = KeyCode::Char('t');
    pub const GO_TO_GAME: KeyCode = KeyCode::Char('g');
    pub const ON_PLANET: KeyCode = KeyCode::Char('O');
    pub const GO_TO_PLANET: KeyCode = KeyCode::Char('G');
    pub const GO_TO_HOME_PLANET: KeyCode = KeyCode::Char('H');
    pub const CHALLENGE_TEAM: KeyCode = KeyCode::Char('C');
    pub const TRAINING_FOCUS: KeyCode = KeyCode::Char('T');
    pub const AUTO_ASSIGN: KeyCode = KeyCode::Char('a');
    pub const SET_TACTIC: KeyCode = KeyCode::Char('t');
    pub const CYCLE_VIEW: KeyCode = KeyCode::Char('V');
    pub const HIRE: KeyCode = KeyCode::Char('H');
    pub const FIRE: KeyCode = KeyCode::Char('F');
    pub const LOCK_PLAYER: KeyCode = KeyCode::Char('L');
    pub const UNLOCK_PLAYER: KeyCode = KeyCode::Char('U');
    pub const SET_CAPTAIN: KeyCode = KeyCode::Char('c');
    pub const SET_DOCTOR: KeyCode = KeyCode::Char('d');
    pub const SET_PILOT: KeyCode = KeyCode::Char('p');
    pub const PITCH_VIEW: KeyCode = KeyCode::Char('v');
    pub const PLAYER_STATUS_VIEW: KeyCode = KeyCode::Char('s');
    pub const TRAVEL: KeyCode = KeyCode::Char('T');
    pub const EXPLORE: KeyCode = KeyCode::Char('x');
    pub const SPACE_ADVENTURE: KeyCode = KeyCode::Char('A');
    pub const DRINK: KeyCode = KeyCode::Char('D');
    pub const UPGRADE_SPACESHIP: KeyCode = KeyCode::Char('U');
    pub const REPAIR_SPACESHIP: KeyCode = KeyCode::Char('R');
    pub const BUY_SCRAPS: KeyCode = KeyCode::Char('s');
    pub const BUY_FUEL: KeyCode = KeyCode::Char('u');
    pub const BUY_GOLD: KeyCode = KeyCode::Char('g');
    pub const BUY_RUM: KeyCode = KeyCode::Char('r');
    pub const SELL_SCRAPS: KeyCode = KeyCode::Char('S');
    pub const SELL_FUEL: KeyCode = KeyCode::Char('U');
    pub const SELL_GOLD: KeyCode = KeyCode::Char('G');
    pub const SELL_RUM: KeyCode = KeyCode::Char('R');
    pub const CREATE_TRADE: KeyCode = KeyCode::Char('P');
    pub const ACCEPT_TRADE: KeyCode = KeyCode::Char('A');
    pub const DECLINE_TRADE: KeyCode = KeyCode::Char('D');
    pub const SPACE_MOVE_LEFT: KeyCode = KeyCode::Left;
    pub const SPACE_MOVE_RIGHT: KeyCode = KeyCode::Right;
    pub const SPACE_MOVE_DOWN: KeyCode = KeyCode::Down;
    pub const SPACE_MOVE_UP: KeyCode = KeyCode::Up;
    pub const SPACE_AUTOFIRE: KeyCode = KeyCode::Char('a');
    pub const SPACE_SHOOT: KeyCode = KeyCode::Char('z');
    pub const SPACE_RELEASE_SCRAPS: KeyCode = KeyCode::Char('s');
    pub const SPACE_BACK_TO_BASE: KeyCode = KeyCode::Char('x');
    pub const YES_TO_DIALOG: KeyCode = KeyCode::Enter;
    pub const NO_TO_DIALOG: KeyCode = KeyCode::Backspace;
    pub const fn set_player_position(position: Position) -> KeyCode {
        match position {
            0 => KeyCode::Char('1'),
            1 => KeyCode::Char('2'),
            2 => KeyCode::Char('3'),
            3 => KeyCode::Char('4'),
            4 => KeyCode::Char('5'),
            _ => panic!("Invalid position for SET_PLAYER_POSITION UiKey."),
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
    pub const SELECTED: Style = DEFAULT_STYLE.bg(Color::Rgb(70, 70, 86));
    pub const SELECTED_BUTTON: Style = DEFAULT_STYLE.fg(Color::Rgb(118, 213, 192));
    pub const UNSELECTABLE: Style = DEFAULT_STYLE.fg(Color::DarkGray);
    pub const ERROR: Style = DEFAULT_STYLE.fg(Color::Red);
    pub const OWN_TEAM: Style = DEFAULT_STYLE.fg(Color::Green);
    pub const HEADER: Style = DEFAULT_STYLE.fg(Color::LightBlue);
    pub const NETWORK: Style = DEFAULT_STYLE.fg(Color::Rgb(204, 144, 184));
    pub const DISCONNECTED: Style = DEFAULT_STYLE.fg(Color::DarkGray);
    pub const SHADOW: Style = DEFAULT_STYLE.fg(Color::Rgb(244, 255, 232));
    pub const HIGHLIGHT: Style = DEFAULT_STYLE.fg(Color::Rgb(118, 213, 192));
    pub const OK: Style = DEFAULT_STYLE.fg(Color::Green);
    pub const WARNING: Style = DEFAULT_STYLE.fg(Color::Yellow);
    pub const STORAGE_KARTOFFEL: Style = DEFAULT_STYLE.fg(Color::Magenta);
    pub const TRAIT_KILLER: Style = DEFAULT_STYLE.fg(Color::Red);
    pub const TRAIT_SHOWPIRATE: Style = DEFAULT_STYLE.fg(Color::Magenta);
    pub const TRAIT_RELENTLESS: Style = DEFAULT_STYLE.fg(Color::Blue);
    pub const TRAIT_SPUGNA: Style = DEFAULT_STYLE.fg(Color::LightRed);
}

pub struct UiText;

impl UiText {
    pub const YES: &'static str = "Ayay";
    pub const NO: &'static str = "Nay!";
}
