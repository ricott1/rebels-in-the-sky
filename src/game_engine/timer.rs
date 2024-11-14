use serde::{Deserialize, Serialize};
use strum::Display;

use crate::{types::Tick, world::constants::TickInterval};

const MINUTES_PER_QUARTER: u16 = 10;
const MINUTES_PER_BREAK: u16 = 2;
// const HALFTIME_BREAK_DURATION: u16 = 10;
// const QUARTERS: u16 = 4;
const SECONDS_PER_MINUTE: u16 = 60;
const MAX_TIME: u16 = SECONDS_PER_MINUTE * (MINUTES_PER_QUARTER * 4 + MINUTES_PER_BREAK * 3);

#[derive(Debug, Display, Default, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum Period {
    #[default]
    NotStarted,
    Q1,
    B1,
    Q2,
    B2,
    Q3,
    B3,
    Q4,
    B4,
}

impl Period {
    pub fn next(&self) -> Period {
        match self {
            Self::NotStarted => Self::Q1,
            Self::Q1 => Self::B1,
            Self::B1 => Self::Q2,
            Self::Q2 => Self::B2,
            Self::B2 => Self::Q3,
            Self::Q3 => Self::B3,
            Self::B3 => Self::Q4,
            Self::Q4 => Self::B4,
            Self::B4 => Self::B4,
        }
    }

    pub fn previous(&self) -> Period {
        match self {
            Self::NotStarted => Self::NotStarted,
            Self::Q1 => Self::NotStarted,
            Self::B1 => Self::Q1,
            Self::Q2 => Self::B1,
            Self::B2 => Self::Q2,
            Self::Q3 => Self::B2,
            Self::B3 => Self::Q3,
            Self::Q4 => Self::B3,
            Self::B4 => Self::Q4,
        }
    }
    pub fn start(&self) -> u16 {
        match self {
            Self::NotStarted => 0,
            Self::Q1 => 1,
            Self::B1 | Self::B2 | Self::B3 | Self::B4 => {
                &self.previous().start() + SECONDS_PER_MINUTE * MINUTES_PER_QUARTER
            }
            Self::Q2 | Self::Q3 | Self::Q4 => {
                &self.previous().start() + SECONDS_PER_MINUTE * MINUTES_PER_BREAK
            }
        }
    }

    pub fn end(&self) -> u16 {
        match self {
            Self::NotStarted => 0,
            Self::Q1 | Self::Q2 | Self::Q3 | Self::Q4 => {
                &self.previous().end() + SECONDS_PER_MINUTE * MINUTES_PER_QUARTER
            }
            Self::B1 | Self::B2 | Self::B3 => {
                &self.previous().end() + SECONDS_PER_MINUTE * MINUTES_PER_BREAK
            }
            Self::B4 => MAX_TIME,
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy, Serialize, Deserialize)]
pub struct Timer {
    pub value: u16,
}

impl Default for Timer {
    fn default() -> Self {
        Self { value: 0 }
    }
}

impl From<u16> for Timer {
    fn from(value: u16) -> Self {
        Self { value }
    }
}

impl Timer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_tick(&self) -> Tick {
        // Returns how many ticks have elapsed since timer started.
        // Value ticks once per TickInterval::Short.
        self.value as Tick * TickInterval::SHORT
    }

    pub fn period(&self) -> Period {
        match self.value {
            x if x < Period::Q1.start() => Period::Q1.previous(),
            x if x < Period::B1.start() => Period::B1.previous(),
            x if x < Period::Q2.start() => Period::Q2.previous(),
            x if x < Period::B2.start() => Period::B2.previous(),
            x if x < Period::Q3.start() => Period::Q3.previous(),
            x if x < Period::B3.start() => Period::B3.previous(),
            x if x < Period::Q4.start() => Period::Q4.previous(),
            x if x < Period::B4.start() => Period::B4.previous(),
            _ => Period::B4,
        }
    }

    pub fn minutes(&self) -> u16 {
        (self.period().end() - self.value) / SECONDS_PER_MINUTE
    }

    pub fn seconds(&self) -> u16 {
        if self.value > MAX_TIME {
            return 0;
        }
        (MAX_TIME - self.value) % SECONDS_PER_MINUTE
    }

    pub fn is_break(&self) -> bool {
        match self.period() {
            Period::NotStarted | Period::B1 | Period::B2 | Period::B3 | Period::B4 => true,
            _ => false,
        }
    }

    pub fn plus(&self, seconds: u16) -> Self {
        Self {
            value: self.value + seconds,
        }
    }

    pub fn reached(&self, goal: u16) -> bool {
        self.value >= goal
    }

    pub fn format(&self) -> String {
        if self.has_ended() {
            return "Q4 00:00".to_string();
        }

        if self.is_break() && self.value == self.period().end() {
            format!("{:2} 10:00", self.period().next(),)
        } else {
            format!(
                "{:2} {:02}:{:02}",
                self.period(),
                self.minutes(),
                self.seconds(),
            )
        }

        // match self.value {
        // x if x == Period::Q1.end() => "Q1 00:00".to_string(),
        // x if x == Period::B1.start() => "B1 02:00".to_string(),
        // // x if x == Period::B1.end() => "B1 00:00".to_string(),
        // x if x == Period::Q2.start() => "Q2 10:00".to_string(),
        // x if x == Period::Q2.end() => "Q2 00:00".to_string(),
        // x if x == Period::B2.start() => "B2 02:00".to_string(),
        // // x if x == Period::B2.end() => "B2 00:00".to_string(),
        // x if x == Period::Q3.start() => "Q3 10:00".to_string(),
        // x if x == Period::Q3.end() => "Q3 00:00".to_string(),
        // x if x == Period::B3.start() => "B3 02:00".to_string(),
        // // x if x == Period::B3.end() => "B3 00:00".to_string(),
        // x if x == Period::Q4.start() => "Q4 10:00".to_string(),
        // x if x == Period::Q4.end() => "Q4 00:00".to_string(),
        // x if x == Period::B4.start() => "B4 02:00".to_string(),
        // x if x == Period::B4.end() => "B4 00:00".to_string(),
        // _ => {
        //     if self.is_break() && self.value == self.period().end() {
        //         format!("{:2} 10:00", self.period().next(),)
        //     } else {
        //         format!(
        //             "{:2} {:02}:{:02}",
        //             self.period(),
        //             self.minutes(),
        //             self.seconds(),
        //         )
        //     }
        // }
        // }
    }

    pub fn tick(&mut self) {
        if self.has_ended() {
            return;
        }
        self.value += 1;
    }

    pub fn tick_by(&mut self, seconds: u16) {
        self.value += seconds;
    }

    pub fn has_started(&self) -> bool {
        self.value > 0
    }

    pub fn has_ended(&self) -> bool {
        self.period() == Period::B4
    }
}

#[cfg(test)]
mod tests {
    use crate::game_engine::timer::{self, Period, Timer, MINUTES_PER_BREAK, MINUTES_PER_QUARTER};
    use std::io::{stdout, Write};

    #[test]
    fn test_timer() {
        let mut timer = timer::Timer::new();
        let mut stdout = stdout();
        const BACKSPACE: char = 8u8 as char;
        while !timer.has_ended() {
            print!("{}\r{}", BACKSPACE, timer.format());
            stdout.flush().unwrap();
            timer.tick_by(1);
        }
    }

    #[test]
    fn test_format() {
        let mut timer = super::Timer::new();
        assert_eq!(timer.format(), "Q1 10:00");
        assert_eq!(timer.period(), super::Period::NotStarted);

        timer.tick();
        assert_eq!(timer.format(), "Q1 09:59");
        assert_eq!(timer.period(), super::Period::Q1);

        timer.tick_by(60);
        assert_eq!(timer.format(), "Q1 08:59");
        assert_eq!(timer.period(), super::Period::Q1);

        timer.tick_by(60 * 8);
        assert_eq!(timer.format(), "Q1 00:59");
        assert_eq!(timer.period(), super::Period::Q1);

        timer.tick_by(59);
        assert_eq!(timer.format(), "Q1 00:00");
        assert_eq!(timer.period(), super::Period::Q1);

        timer.tick();
        assert_eq!(timer.format(), "B1 01:59");
        assert_eq!(timer.period(), super::Period::B1);

        timer.tick();
        assert_eq!(timer.format(), "B1 01:58");
        assert_eq!(timer.period(), super::Period::B1);

        timer.tick_by(60 * 2 - 2);
        assert_eq!(timer.format(), "Q2 10:00");
        assert_eq!(timer.period(), super::Period::B1);

        timer.tick();
        assert_eq!(timer.format(), "Q2 09:59");
        assert_eq!(timer.period(), super::Period::Q2);

        timer.tick_by(60 * 10 - 1);
        assert_eq!(timer.format(), "Q2 00:00");
        assert_eq!(timer.period(), super::Period::Q2);

        timer.tick_by(60 * 2 - 1);
        assert_eq!(timer.format(), "B2 00:01");
        assert_eq!(timer.period(), super::Period::B2);

        timer.tick_by(1);
        assert_eq!(timer.format(), "Q3 10:00");
        assert_eq!(timer.period(), super::Period::B2);

        timer.tick_by(60 * 8);
        assert_eq!(timer.format(), "Q3 02:00");
        assert_eq!(timer.period(), super::Period::Q3);

        timer.tick_by(60 * 2);
        assert_eq!(timer.format(), "Q3 00:00");
        assert_eq!(timer.period(), super::Period::Q3);

        timer.tick_by(60 * 2 - 1);
        assert_eq!(timer.format(), "B3 00:01");
        assert_eq!(timer.period(), super::Period::B3);

        timer.tick_by(1);
        assert_eq!(timer.format(), "Q4 10:00");
        assert_eq!(timer.period(), super::Period::B3);

        timer.tick();
        assert_eq!(timer.format(), "Q4 09:59");
        assert_eq!(timer.period(), super::Period::Q4);

        timer.tick_by(1);
        assert_eq!(timer.format(), "Q4 09:58");
        assert_eq!(timer.period(), super::Period::Q4);

        timer.tick_by(60 * 10 - 2);
        assert_eq!(timer.format(), "Q4 00:00");
        assert_eq!(timer.period(), super::Period::Q4);
        assert_eq!(timer.has_ended(), false);

        timer.tick();
        assert_eq!(timer.format(), "Q4 00:00");
        assert_eq!(timer.period(), super::Period::B4);
        assert_eq!(timer.has_ended(), true);
    }

    #[test]
    fn test_seconds() {
        let mut timer = super::Timer::new();
        assert_eq!(timer.seconds(), 0);
        timer.tick();
        assert_eq!(timer.seconds(), 59);
        timer.tick_by(60);
        assert_eq!(timer.seconds(), 59);
        timer.tick_by(60 * 9);
        assert_eq!(timer.seconds(), 59);
        timer.tick_by(17);
        assert_eq!(timer.seconds(), 42);
        timer.tick_by(42);
        assert_eq!(timer.seconds(), 0);
    }

    #[test]
    fn test_minutes() {
        let mut timer = super::Timer::new();
        assert_eq!(timer.minutes(), 0);
        assert_eq!(timer.seconds(), 0);
        timer.tick();

        //09:59
        assert_eq!(timer.minutes(), 9);
        assert_eq!(timer.seconds(), 59);
        timer.tick_by(60);
        //08:59
        assert_eq!(timer.minutes(), 8);
        assert_eq!(timer.seconds(), 59);
        timer.tick_by(60 * 8);
        //00:59
        assert_eq!(timer.minutes(), 0);
        assert_eq!(timer.seconds(), 59);
        timer.tick_by(59);

        //00:00
        assert_eq!(timer.minutes(), 0);
        assert_eq!(timer.seconds(), 0);
        timer.tick_by(60 * 2 + 1);
        assert_eq!(timer.minutes(), 9);
        assert_eq!(timer.seconds(), 59);
        timer.tick_by(60 * 10);
        assert_eq!(timer.minutes(), 1);
        assert_eq!(timer.seconds(), 59);
        timer.tick_by(60);
        assert_eq!(timer.minutes(), 0);
        assert_eq!(timer.seconds(), 59);
    }

    #[test]
    fn test_period() {
        let mut timer = super::Timer::new();
        assert_eq!(timer.period(), super::Period::NotStarted);
        timer.tick();
        assert_eq!(timer.period(), super::Period::Q1);
        timer.tick_by(59);
        assert_eq!(timer.period(), super::Period::Q1);
        timer.tick_by(60 * 9);
        assert_eq!(timer.period(), super::Period::Q1);
        timer.tick();
        assert_eq!(timer.period(), super::Period::B1);
        timer.tick_by(60 * MINUTES_PER_BREAK - 1);
        assert_eq!(timer.period(), super::Period::B1);

        timer.tick();
        assert_eq!(timer.period(), super::Period::Q2);

        timer.tick_by(60 * MINUTES_PER_QUARTER);
        assert_eq!(timer.period(), super::Period::B2);

        timer.tick_by(60 * MINUTES_PER_BREAK);
        assert_eq!(timer.period(), super::Period::Q3);

        timer.tick_by(60 * MINUTES_PER_QUARTER);
        assert_eq!(timer.period(), super::Period::B3);

        timer.tick_by(60 * MINUTES_PER_BREAK);
        assert_eq!(timer.period(), super::Period::Q4);

        timer.tick_by(60 * MINUTES_PER_QUARTER - 1);
        assert_eq!(timer.period(), super::Period::Q4);
        assert_eq!(timer.has_ended(), false);

        timer.tick_by(1);
        assert_eq!(timer.period(), super::Period::B4);
        assert_eq!(timer.has_ended(), true);
    }

    #[test]
    fn test_end_period() {
        assert_eq!(Timer::from(Period::NotStarted.start()).format(), "Q1 10:00");
        assert_eq!(Timer::from(Period::NotStarted.end()).format(), "Q1 10:00");
        assert_eq!(Timer::from(Period::Q1.start()).format(), "Q1 09:59");
        assert_eq!(Timer::from(Period::Q1.end()).format(), "Q1 00:00");
        assert_eq!(Timer::from(Period::B1.start()).format(), "B1 01:59");
        assert_eq!(Timer::from(Period::B1.end()).format(), "Q2 10:00");
        assert_eq!(Timer::from(Period::Q2.start()).format(), "Q2 09:59");
        assert_eq!(Timer::from(Period::Q2.end()).format(), "Q2 00:00");
        assert_eq!(Timer::from(Period::B2.start()).format(), "B2 01:59");
        assert_eq!(Timer::from(Period::B2.end()).format(), "Q3 10:00");
        assert_eq!(Timer::from(Period::Q3.start()).format(), "Q3 09:59");
        assert_eq!(Timer::from(Period::Q3.end()).format(), "Q3 00:00");
        assert_eq!(Timer::from(Period::B3.start()).format(), "B3 01:59");
        assert_eq!(Timer::from(Period::B3.end()).format(), "Q4 10:00");
        assert_eq!(Timer::from(Period::Q4.start()).format(), "Q4 09:59");
        assert_eq!(Timer::from(Period::Q4.end()).format(), "Q4 00:00");
        assert_eq!(Timer::from(Period::B4.start()).format(), "Q4 00:00");
    }
}
