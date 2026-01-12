use crossterm::event::KeyCode;

pub const ESC: KeyCode = KeyCode::Esc;

pub const NEXT_TAB: KeyCode = KeyCode::Right;
pub const PREVIOUS_TAB: KeyCode = KeyCode::Left;

pub const NEXT_SELECTION: KeyCode = KeyCode::Char(']');
pub const PREVIOUS_SELECTION: KeyCode = KeyCode::Char('[');

pub const UI_DEBUG_MODE: KeyCode = KeyCode::Char('`');
pub const CYCLE_VIEW: KeyCode = KeyCode::Tab;

pub const YES_TO_DIALOG: KeyCode = KeyCode::Enter;
pub const NO_TO_DIALOG: KeyCode = KeyCode::Backspace;

pub const CREATE_TRADE: KeyCode = KeyCode::Char('P');
pub const ACCEPT_TRADE: KeyCode = KeyCode::Char('A');
pub const DECLINE_TRADE: KeyCode = KeyCode::Char('D');

pub const ORGANIZE_TOURNAMENT: KeyCode = KeyCode::Char('t');
pub const REGISTER_TO_TOURNAMENT: KeyCode = KeyCode::Char('R');

pub const GO_TO_TEAM: KeyCode = KeyCode::Backspace;
pub const GO_TO_TEAM_ALT: KeyCode = KeyCode::Char('t');
pub const GO_TO_GAME: KeyCode = KeyCode::Char('g');
pub const GO_TO_CURRENT_GAME: KeyCode = KeyCode::Char('C');

pub const ON_PLANET: KeyCode = KeyCode::Char('O');
pub const GO_TO_PLANET: KeyCode = KeyCode::Char('G');
pub const GO_TO_SPACE_COVE: KeyCode = KeyCode::Char('s');
pub const GO_TO_HOME_PLANET: KeyCode = KeyCode::Char('H');

pub const TRAVEL: KeyCode = KeyCode::Char('T');
pub const EXPLORE: KeyCode = KeyCode::Char('x');
pub const SPACE_ADVENTURE: KeyCode = KeyCode::Char('A');
pub const ABANDON_ASTEROID: KeyCode = KeyCode::Char('A');
pub const BUILD_ASTEROID_UPGRADE: KeyCode = KeyCode::Char('B');
pub const UPGRADE_SPACESHIP: KeyCode = KeyCode::Char('U');
pub const REPAIR_SPACESHIP: KeyCode = KeyCode::Char('R');

pub mod space {
    use super::KeyCode;

    pub const MOVE_LEFT: KeyCode = KeyCode::Left;
    pub const MOVE_RIGHT: KeyCode = KeyCode::Right;
    pub const MOVE_DOWN: KeyCode = KeyCode::Down;
    pub const MOVE_UP: KeyCode = KeyCode::Up;

    pub const AUTOFIRE: KeyCode = KeyCode::Char('a');
    pub const SHOOT: KeyCode = KeyCode::Char('z');
    pub const RELEASE_SCRAPS: KeyCode = KeyCode::Char('r');
    pub const TOGGLE_SHIELD: KeyCode = KeyCode::Char('s');
    pub const BACK_TO_BASE: KeyCode = KeyCode::Char('x');

    pub const ALL: &[KeyCode] = &[
        MOVE_LEFT,
        MOVE_RIGHT,
        MOVE_DOWN,
        MOVE_UP,
        AUTOFIRE,
        SHOOT,
        RELEASE_SCRAPS,
        TOGGLE_SHIELD,
        BACK_TO_BASE,
    ];
}

#[cfg(feature = "audio")]
pub mod radio {
    use super::KeyCode;
    pub const TOGGLE_AUDIO: KeyCode = KeyCode::Char('|');
    pub const PREVIOUS_RADIO: KeyCode = KeyCode::Char('<');
    pub const NEXT_RADIO: KeyCode = KeyCode::Char('>');
}

pub mod game {
    use super::KeyCode;
    pub const PITCH_VIEW: KeyCode = KeyCode::Char('v');
    pub const PLAYER_STATUS_VIEW: KeyCode = KeyCode::Char('s');
    pub const CHALLENGE_TEAM: KeyCode = KeyCode::Char('C');
}

pub mod player {
    use super::KeyCode;
    pub const HIRE: KeyCode = KeyCode::Char('H');
    pub const FIRE: KeyCode = KeyCode::Char('F');
    pub const LOCK_PLAYER: KeyCode = KeyCode::Char('L');
    pub const UNLOCK_PLAYER: KeyCode = KeyCode::Char('U');
    pub const DRINK: KeyCode = KeyCode::Char('D');
    pub const PLAYER_STATUS_VIEW: KeyCode = KeyCode::Char('s');
}

pub mod team {
    use crate::core::GamePosition;

    use super::KeyCode;
    pub const TRAINING_FOCUS: KeyCode = KeyCode::Char('T');
    pub const AUTO_ASSIGN: KeyCode = KeyCode::Char('a');
    pub const SET_TACTIC: KeyCode = KeyCode::Char('t');

    pub const TOGGLE_ACCEPT_LOCAL_CHALLENGES: KeyCode = KeyCode::Char('l');
    pub const TOGGLE_ACCEPT_NETWORK_CHALLENGES: KeyCode = KeyCode::Char('n');

    pub const SET_CAPTAIN: KeyCode = KeyCode::Char('c');
    pub const SET_DOCTOR: KeyCode = KeyCode::Char('d');
    pub const SET_ENGINEER: KeyCode = KeyCode::Char('e');
    pub const SET_PILOT: KeyCode = KeyCode::Char('p');

    pub const fn set_player_position(position: GamePosition) -> KeyCode {
        match position {
            0 => KeyCode::Char('1'),
            1 => KeyCode::Char('2'),
            2 => KeyCode::Char('3'),
            3 => KeyCode::Char('4'),
            4 => KeyCode::Char('5'),
            5 => KeyCode::Char('6'),
            6 => KeyCode::Char('7'),
            _ => panic!("Invalid position for SET_PLAYER_POSITION"),
        }
    }
}

/// Trading & economy
pub mod market {
    use super::KeyCode;
    pub const BUY_SCRAPS: KeyCode = KeyCode::Char('s');
    pub const BUY_FUEL: KeyCode = KeyCode::Char('u');
    pub const BUY_GOLD: KeyCode = KeyCode::Char('g');
    pub const BUY_RUM: KeyCode = KeyCode::Char('r');

    pub const SELL_SCRAPS: KeyCode = KeyCode::Char('S');
    pub const SELL_FUEL: KeyCode = KeyCode::Char('U');
    pub const SELL_GOLD: KeyCode = KeyCode::Char('G');
    pub const SELL_RUM: KeyCode = KeyCode::Char('R');
}
