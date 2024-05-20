use core::fmt;

use super::position::{GamePosition, Position};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

const NORMAL_AVG: f32 = 0.6;
const NORMAL_STD: f32 = 4.4;
const LEVEL_BONUS: u8 = 2;
pub const WEIGHT_MOD: f32 = 1.45;
pub const MIN_SKILL: f32 = 0.0;
pub const MAX_SKILL: f32 = 20.0;
pub const SKILL_NAMES: [&'static str; 20] = [
    "Quickness",
    "Vertical",
    "Strength",
    "Stamina",
    "Dunk",
    "Close",
    "Medium",
    "Long",
    "Steal",
    "Block",
    "Perimeter",
    "Interior",
    "Passing",
    "Handling",
    "Posting",
    "Rebounds",
    "Vision",
    "Positioning",
    "Off-ball",
    "Charisma",
];

pub trait Rated {
    fn rating(&self) -> u8;
    fn stars(&self) -> String {
        match self.rating() {
            0 => "☆☆☆☆☆".to_string(),
            1..=2 => "½☆☆☆☆".to_string(),
            3..=4 => "★☆☆☆☆".to_string(),
            5..=6 => "★½☆☆☆".to_string(),
            7..=8 => "★★☆☆☆".to_string(),
            9..=10 => "★★½☆☆".to_string(),
            11..=12 => "★★★☆☆".to_string(),
            13..=14 => "★★★½☆".to_string(),
            15..=16 => "★★★★☆".to_string(),
            17..=18 => "★★★★½".to_string(),
            19..=20 => "★★★★★".to_string(),
            _ => panic!("Invalid rating"),
        }
    }
}

impl Rated for f32 {
    fn rating(&self) -> u8 {
        *self as u8
    }
}

impl Rated for u8 {
    fn rating(&self) -> u8 {
        *self
    }
}

pub type Skill = f32;

pub trait GameSkill: fmt::Display + fmt::Debug {
    fn value(&self) -> u8 {
        self.bound() as u8
    }
    fn raw_value(&self) -> f32 {
        self.bound()
    }
    fn bound(&self) -> f32;
    fn normal_sample(&self, rng: &mut ChaCha8Rng) -> f32;
}

impl GameSkill for Skill {
    fn bound(&self) -> Skill {
        self.max(MIN_SKILL).min(MAX_SKILL)
    }

    fn normal_sample(&self, rng: &mut ChaCha8Rng) -> Skill {
        Normal::new(NORMAL_AVG + self, NORMAL_STD)
            .unwrap()
            .sample(rng)
            .round()
            .bound()
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Copy, Clone, PartialEq)]
pub struct Athleticism {
    pub quickness: Skill,
    pub vertical: Skill,
    pub strength: Skill,
    pub stamina: Skill,
}

impl Athleticism {
    pub fn for_position(position: Position, rng: &mut ChaCha8Rng, base_level: f32) -> Self {
        let weights = position.weights();
        let level = base_level + rng.gen_range(0..=LEVEL_BONUS) as f32;
        let quickness = (level + WEIGHT_MOD * weights[0] as f32).normal_sample(rng);
        let vertical = (level + WEIGHT_MOD * weights[1] as f32).normal_sample(rng);
        let strength = (level + WEIGHT_MOD * weights[2] as f32).normal_sample(rng);
        let stamina = (level + WEIGHT_MOD * weights[3] as f32).normal_sample(rng);
        Self {
            quickness,
            vertical,
            strength,
            stamina,
        }
    }
}

impl Rated for Athleticism {
    fn rating(&self) -> u8 {
        (self.quickness + self.vertical + self.strength + self.stamina) as u8 / 4
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Offense {
    pub dunk: Skill,
    pub close_range: Skill,
    pub medium_range: Skill,
    pub long_range: Skill,
}

impl Offense {
    pub fn for_position(position: Position, rng: &mut ChaCha8Rng, base_level: f32) -> Self {
        let weights = position.weights();
        let level = base_level + rng.gen_range(0..=LEVEL_BONUS) as f32;
        let dunk = (level + WEIGHT_MOD * weights[4] as f32).normal_sample(rng);
        let close_range = (level + WEIGHT_MOD * weights[5] as f32).normal_sample(rng);
        let medium_range = (level + WEIGHT_MOD * weights[6] as f32).normal_sample(rng);
        let long_range = (level + WEIGHT_MOD * weights[7] as f32).normal_sample(rng);
        Self {
            dunk,
            close_range,
            medium_range,
            long_range,
        }
    }
}

impl Rated for Offense {
    fn rating(&self) -> u8 {
        (self.dunk + self.close_range + self.medium_range + self.long_range) as u8 / 4
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Defense {
    pub steal: Skill,
    pub block: Skill,
    pub perimeter_defense: Skill,
    pub interior_defense: Skill,
}

impl Defense {
    pub fn for_position(position: Position, rng: &mut ChaCha8Rng, base_level: f32) -> Self {
        let weights = position.weights();
        let level = base_level + rng.gen_range(0..=LEVEL_BONUS) as f32;
        let steal = (level + WEIGHT_MOD * weights[8] as f32).normal_sample(rng);
        let block = (level + WEIGHT_MOD * weights[9] as f32).normal_sample(rng);
        let perimeter_defense = (level + WEIGHT_MOD * weights[10] as f32).normal_sample(rng);
        let interior_defense = (level + WEIGHT_MOD * weights[12] as f32).normal_sample(rng);
        Self {
            steal,
            block,
            perimeter_defense,
            interior_defense,
        }
    }
}

impl Rated for Defense {
    fn rating(&self) -> u8 {
        (self.steal + self.block + self.perimeter_defense + self.interior_defense) as u8 / 4
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Technical {
    pub passing: Skill,
    pub ball_handling: Skill,
    pub post_moves: Skill,
    pub rebounds: Skill,
}

impl Technical {
    pub fn for_position(position: Position, rng: &mut ChaCha8Rng, base_level: f32) -> Self {
        let weights = position.weights();
        let level = base_level + rng.gen_range(0..=LEVEL_BONUS) as f32;
        let passing = (level + WEIGHT_MOD * weights[12] as f32).normal_sample(rng);
        let ball_handling = (level + WEIGHT_MOD * weights[13] as f32).normal_sample(rng);
        let post_moves = (level + WEIGHT_MOD * weights[14] as f32).normal_sample(rng);
        let rebounds = (level + WEIGHT_MOD * weights[15] as f32).normal_sample(rng);
        Self {
            passing,
            ball_handling,
            post_moves,
            rebounds,
        }
    }
}

impl Rated for Technical {
    fn rating(&self) -> u8 {
        (self.passing + self.ball_handling + self.post_moves + self.rebounds) as u8 / 4
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Mental {
    pub vision: Skill,
    pub aggression: Skill,
    pub off_ball_movement: Skill,
    pub charisma: Skill,
}

impl Mental {
    pub fn for_position(position: Position, rng: &mut ChaCha8Rng, base_level: f32) -> Self {
        let weights = position.weights();
        let level = base_level + rng.gen_range(0..=LEVEL_BONUS) as f32;
        let vision = (level + WEIGHT_MOD * weights[16] as f32).normal_sample(rng);
        let aggression = (level + WEIGHT_MOD * weights[17] as f32).normal_sample(rng);
        let off_ball_movement = (level + WEIGHT_MOD * weights[18] as f32).normal_sample(rng);
        let charisma = (level + WEIGHT_MOD * weights[19] as f32).normal_sample(rng);
        Self {
            vision,
            aggression,
            off_ball_movement,
            charisma,
        }
    }
}

impl Rated for Mental {
    fn rating(&self) -> u8 {
        (self.vision + self.aggression + self.off_ball_movement + self.charisma) as u8 / 4
    }
}
