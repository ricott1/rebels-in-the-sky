use super::constants::MAX_PLAYERS_PER_GAME;

pub type GamePosition = u8;
pub const NUM_GAME_POSITIONS: GamePosition = 5;

pub trait GamePositionUtils {
    fn as_str(&self) -> &str;
    fn weights(&self) -> [f32; 20];
}

impl GamePositionUtils for GamePosition {
    fn as_str(&self) -> &str {
        match self {
            0 => "PG",
            1 => "SG",
            2 => "SF",
            3 => "PF",
            4 => "C",
            &x if x < MAX_PLAYERS_PER_GAME as u8 => "Bench",
            _ => "Out",
        }
    }
    fn weights(&self) -> [f32; 20] {
        match self {
            0 => [
                4.0, 2.0, 2.0, 3.0, 2.0, 3.0, 3.0, 4.0, 4.0, 2.0, 5.0, 2.0, 4.0, 4.0, 1.0, 2.0,
                4.0, 2.0, 4.0, 3.0,
            ],
            1 => [
                4.0, 3.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 4.0, 2.0, 4.0, 2.0, 3.0, 5.0, 1.0, 2.0,
                4.0, 1.0, 3.0, 3.0,
            ],
            2 => [
                3.0, 5.0, 3.0, 5.0, 2.0, 2.0, 5.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 2.0, 3.0,
                2.0, 1.0, 2.0, 4.0,
            ],

            3 => [
                2.0, 3.0, 4.0, 3.0, 4.0, 3.0, 3.0, 2.0, 1.0, 4.0, 2.0, 4.0, 1.0, 3.0, 5.0, 4.0,
                2.0, 4.0, 3.0, 3.0,
            ],
            4 => [
                2.0, 3.0, 4.0, 3.0, 3.0, 4.0, 3.0, 2.0, 2.0, 4.0, 2.0, 4.0, 2.0, 2.0, 5.0, 5.0,
                1.0, 3.0, 3.0, 3.0,
            ],

            idx => panic!("Invalid position: {idx}"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::{GamePositionUtils, NUM_GAME_POSITIONS};
    use itertools::Itertools;

    #[test]
    fn test_weights() {
        let means = (0..NUM_GAME_POSITIONS)
            .map(|p| p.weights().iter().map(|w| w).sum::<f32>())
            .collect_vec();
        println!("{:?}", means);

        let m = means[0];
        for mean in means {
            assert!(mean == m);
        }

        let stds = (0..NUM_GAME_POSITIONS)
            .map(|p| p.weights().iter().map(|w| w.powf(2.0)).sum::<f32>())
            .collect_vec();
        println!("{:?}", stds);

        let s = stds[0];
        for std in stds {
            assert!(std == s);
        }
    }
}
