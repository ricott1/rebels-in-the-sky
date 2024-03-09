use std::cmp::min;

use super::constants::MAX_TIREDNESS;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

pub fn roll(rng: &mut ChaCha8Rng, tiredness: f32) -> u8 {
    if tiredness == MAX_TIREDNESS {
        return 0;
    }
    min(
        ((MAX_TIREDNESS - tiredness / 2.0) / 2.0).round() as u8,
        rng.gen_range(1..=50),
    )
}

#[cfg(test)]
mod tests {
    use super::roll;
    use crate::engine::types::GameStats;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_roll() {
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let mut stats = GameStats::default();
        stats.tiredness = 0.0;
        for _ in 0..100 {
            let r = roll(&mut rng, stats.tiredness);
            assert_eq!((r > 0 && r <= 50), true);
        }
        stats.tiredness = 100.0;
        for _ in 0..100 {
            let r = roll(&mut rng, stats.tiredness);
            assert_eq!((r > 0 && r <= 25), true);
        }
    }
}
