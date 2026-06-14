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
        // Loosely based on the FIDE K-factor tiers, with the new-player threshold
        // lowered to K_FACTOR_REDUCTION_THRESHOLD games:
        // K = 30: until the team has completed K_FACTOR_REDUCTION_THRESHOLD games.
        // K = 15: for teams that have always been rated under 2400.
        // K = 10: once a team has reached a rating of at least 2400 (permanent thereafter).
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
        let k_factor = self.k_factor();

        self.record
            .entry(result)
            .and_modify(|e| *e += 1)
            .or_insert(1);

        let pa = self.expected_score(other_rating);

        let outcome = match result {
            GameResult::Win => 1.0,
            GameResult::Draw => 0.5,
            GameResult::Loss => 0.0,
        };

        let new_rating = self.rating + k_factor as f32 * (outcome - pa);

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

    #[test]
    fn test_rating_update_is_zero_sum() {
        // Both equal-rated and unequal-rated (same K, since both are fresh) teams.
        for (start_a, start_b) in [(1200.0, 1200.0), (1500.0, 1100.0)] {
            let mut rating_a = GameRating::default();
            rating_a.rating = start_a;
            let mut rating_b = GameRating::default();
            rating_b.rating = start_b;

            let pre_a = rating_a.clone();
            let pre_b = rating_b.clone();

            rating_a.update(GameResult::Win, &pre_b);
            rating_b.update(GameResult::Loss, &pre_a);

            let winner_gain = rating_a.rating - start_a;
            let loser_loss = start_b - rating_b.rating;

            assert!(winner_gain > 0.0, "winner should gain rating");
            assert!(loser_loss > 0.0, "loser should lose rating");
            assert!(
                (winner_gain - loser_loss).abs() < 1e-3,
                "Elo must be zero-sum for equal K: winner gained {winner_gain}, loser lost {loser_loss}"
            );
        }
    }
}
