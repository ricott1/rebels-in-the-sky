use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

const DEFAULT_RATING: f32 = 1200.0;
const FLOOR_RATING: f32 = 100.0;
const K_FACTOR_REDUCTION_THRESHOLD: usize = 10;

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum GameResult {
    Win,
    Draw,
    Loss,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameRating {
    pub rating: f32,
    pub record: HashMap<GameResult, usize>,
    has_been_above_2400: bool,
}

impl Default for GameRating {
    fn default() -> Self {
        Self {
            rating: DEFAULT_RATING,
            record: HashMap::default(),
            has_been_above_2400: false,
        }
    }
}

impl GameRating {
    fn num_games(&self) -> usize {
        self.record.values().sum()
    }

    fn k_factor(&self) -> usize {
        // K = 30: for a player new to the rating list until the completion of events with a total of 30 games.
        // K = 15: for players who have always been rated under 2400.
        // K = 10: for players with any published rating of at least 2400 and at least 30 games played in previous events. Thereafter it remains permanently at 10.
        let n = self.num_games();
        if n < K_FACTOR_REDUCTION_THRESHOLD {
            30
        } else if !self.has_been_above_2400 {
            15
        } else {
            10
        }
    }

    fn expected_score(&self, other_rating: &GameRating) -> f32 {
        1.0 / (1.0 + 10.0_f32.powf((other_rating.rating - self.rating) / 400.0))
    }

    pub fn update(&mut self, result: GameResult, other_rating: &GameRating) {
        self.record
            .entry(result)
            .and_modify(|e| *e += 1)
            .or_insert(1);

        let pa = self.expected_score(other_rating);

        let outcome = match result {
            GameResult::Win => 1.0,
            GameResult::Draw => 0.5,
            GameResult::Loss => -1.0,
        };

        let new_rating = self.rating + self.k_factor() as f32 * (outcome - pa);

        self.rating = new_rating.max(FLOOR_RATING);

        if !self.has_been_above_2400 && self.rating >= 2400.0 {
            self.has_been_above_2400 = true;
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::core::{GameRating, GameResult};

    #[test]
    fn test_rating_update() {
        let mut rating_a = GameRating::default();
        let mut rating_b = GameRating::default();

        for _ in 0..35 {
            rating_a.update(GameResult::Win, &rating_b);
            rating_b.update(GameResult::Loss, &rating_a);

            assert!(rating_a.rating > rating_b.rating);
            print!("{rating_a:#?} vs {rating_b:#?}");
        }

        rating_a.update(GameResult::Draw, &rating_b);
        rating_b.update(GameResult::Draw, &rating_a);
        print!("{rating_a:#?} vs {rating_b:#?}");
    }
}
