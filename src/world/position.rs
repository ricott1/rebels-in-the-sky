use super::{constants::MAX_PLAYERS_PER_GAME, skill::Skill};

pub type Position = u8;
pub const MAX_POSITION: Position = 5;

pub trait GamePosition {
    fn as_str(&self) -> &str;
    fn weights(&self) -> [f32; 20];
    fn player_rating(&self, skills: [Skill; 20]) -> f32 {
        let mut rating = 0.0;
        let weights = self.weights();
        let mut total_weight = 0.0;
        for i in 0..skills.len() {
            let w = weights[i] as f32;
            rating += w * w * skills[i];
            total_weight += w * w;
        }
        (rating / total_weight).round()
    }
    fn best(skills: [Skill; 20]) -> Self
    where
        Self: Sized;
}

impl GamePosition for Position {
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

            _ => panic!("Invalid position"),
        }
    }

    fn player_rating(&self, skills: [Skill; 20]) -> f32 {
        let mut rating = 0 as f32;
        let weights = self.weights();
        let mut total_weight = 0 as f32;
        for i in 0..skills.len() {
            let w = weights[i] as f32;
            rating += w * w * skills[i];
            total_weight += w * w;
        }
        (rating / total_weight).round()
    }

    fn best(skills: [Skill; 20]) -> Self
    where
        Self: Sized,
    {
        let mut best = 0;
        let mut best_rating = 0.0;
        for i in 0..MAX_POSITION {
            let rating = i.player_rating(skills);
            if rating > best_rating {
                best = i;
                best_rating = rating;
            }
        }
        best
    }
}

#[cfg(test)]
mod test {
    use super::{GamePosition, MAX_POSITION};
    use itertools::Itertools;

    #[test]
    fn test_weights() {
        let means = (0..MAX_POSITION)
            .map(|p| p.weights().iter().map(|w| w).sum::<f32>())
            .collect_vec();
        println!("{:?}", means);

        let m = means[0];
        for mean in means {
            assert!(mean == m);
        }

        let stds = (0..MAX_POSITION)
            .map(|p| p.weights().iter().map(|w| w.powf(2.0)).sum::<f32>())
            .collect_vec();
        println!("{:?}", stds);

        let s = stds[0];
        for std in stds {
            assert!(std == s);
        }
    }
}
