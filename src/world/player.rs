use super::{
    constants::{COST_PER_VALUE, EXPERIENCE_PER_SKILL_MULTIPLIER, REPUTATION_PER_EXPERIENCE},
    jersey::Jersey,
    planet::Planet,
    position::{GamePosition, PlayingStyle, MAX_POSITION},
    role::CrewRole,
    skill::{GameSkill, Skill, MAX_SKILL},
    types::{PlayerLocation, Pronoun, TrainingFocus},
    utils::PLAYER_DATA,
};
use crate::{
    image::{player::PlayerImage, types::Gif},
    types::{PlanetId, PlayerId, TeamId},
    world::{
        position::Position,
        skill::{Athleticism, Defense, Mental, Offense, Rated, Technical},
        types::Population,
        utils::skill_linear_interpolation,
    },
};
use libp2p::PeerId;
use rand::{seq::SliceRandom, Rng};
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

const HOOK_MAX_BALL_HANDLING: f32 = 4.0;
const EYE_PATCH_MAX_VISION: f32 = 4.0;
const WOODEN_LEG_MAX_QUICKNESS: f32 = 4.0;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub peer_id: Option<PeerId>,
    pub version: u64,
    pub info: InfoStats,
    pub team: Option<TeamId>,
    pub jersey_number: Option<usize>,
    pub reputation: f32,
    pub playing_style: PlayingStyle,
    pub athleticism: Athleticism,
    pub offense: Offense,
    pub technical: Technical,
    pub defense: Defense,
    pub mental: Mental,
    pub image: PlayerImage,
    pub current_location: PlayerLocation,
    pub previous_skills: [Skill; 20], // This is for displaying purposes to show the skills that were recently modified
    pub training_focus: Option<TrainingFocus>,
    pub tiredness: f32,
}

impl Player {
    pub fn load(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut player = serde_json::from_slice::<Player>(&data)?;
        player.compose_image()?;
        player.previous_skills = player.current_skill_array();
        Ok(player)
    }

    pub fn current_skill_array(&self) -> [Skill; 20] {
        // assert!(self.previous_skills.len() == 20);
        (0..20)
            .map(|idx| self.skill_at_index(idx))
            .collect::<Vec<Skill>>()
            .try_into()
            .unwrap()
    }

    fn player_value(&self) -> u32 {
        let value = self.total_skills() as f32;
        let age_diff = self.info.age - 36.0;
        // Multiply value by distribution decaying for age = 36.
        // age factor is basically a sigmoid function
        let age_factor = 1.0 / (1.0 + (0.5 * age_diff).exp());
        (value * (0.5 + age_factor)) as u32
    }

    pub fn hire_cost(&self, team_reputation: f32) -> u32 {
        if self.reputation <= team_reputation {
            return 0;
        }

        COST_PER_VALUE * self.player_value() * (self.reputation - team_reputation) as u32
    }

    pub fn release_cost(&self) -> u32 {
        // COST_PER_VALUE * self.player_value() / 2
        0
    }

    pub fn random(
        rng: &mut ChaCha8Rng,
        id: PlayerId,
        position: Option<Position>,
        home_planet: &Planet,
        mut base_level: f32,
    ) -> Self {
        if position.is_none() {
            let position = rng.gen_range(0..5);
            return Self::random(rng, id, Some(position), home_planet, base_level);
        }

        let info = InfoStats::for_position(position, rng, home_planet);
        let population = info.population;

        if base_level > info.age as f32 / 8.0 {
            base_level = info.age as f32 / 8.0;
        }

        let athleticism = Athleticism::for_position(position.unwrap(), rng, base_level);
        let offense = Offense::for_position(position.unwrap(), rng, base_level);
        let technical = Technical::for_position(position.unwrap(), rng, base_level);
        let defense = Defense::for_position(position.unwrap(), rng, base_level);
        let mental = Mental::for_position(position.unwrap(), rng, base_level);

        let image = PlayerImage::from_info(&info, rng);

        let mut player = Self {
            id,
            peer_id: None,
            version: 0,
            info,
            team: None,
            jersey_number: None,
            reputation: 0.0,
            playing_style: PlayingStyle::random(rng),
            athleticism,
            offense,
            technical,
            defense,
            mental,
            current_location: PlayerLocation::OnPlanet {
                planet_id: home_planet.id,
            },
            image,
            previous_skills: [Skill::default(); 20],
            training_focus: None,
            tiredness: 0.0,
        };

        player
            .info
            .population
            .apply_skill_modifiers(&mut player.clone());

        player.apply_info_modifiers();

        if athleticism.quickness < WOODEN_LEG_MAX_QUICKNESS {
            player.image.set_wooden_leg(rng);
            player.mental.charisma = (player.mental.charisma + 1.0).bound();
        }
        if mental.vision < EYE_PATCH_MAX_VISION {
            player.image.set_eye_patch(rng, &population);
            player.mental.charisma = (player.mental.charisma + 1.0).bound();
        }

        if technical.ball_handling < HOOK_MAX_BALL_HANDLING {
            player.image.set_hook(rng);
            player.mental.charisma = (player.mental.charisma + 1.0).bound();
        }

        player.previous_skills = player.current_skill_array();

        player.reputation =
            (player.total_skills() as f32 / 100.0 + player.info.age as f32 / 24.0).bound();

        player
    }

    fn apply_info_modifiers(&mut self) {
        self.athleticism.quickness = skill_linear_interpolation(
            self.athleticism.quickness,
            self.info.weight,
            [90.0, 1.0, 130.0, 0.5],
        );
        self.athleticism.vertical = skill_linear_interpolation(
            self.athleticism.vertical,
            self.info.weight,
            [90.0, 1.0, 130.0, 0.5],
        );
        self.athleticism.strength = skill_linear_interpolation(
            self.athleticism.strength,
            self.info.weight,
            [75.0, 0.5, 135.0, 1.3],
        );
        self.athleticism.stamina = skill_linear_interpolation(
            self.athleticism.stamina,
            self.info.weight,
            [90.0, 1.0, 130.0, 0.8],
        );
        self.technical.rebounding = skill_linear_interpolation(
            self.technical.rebounding,
            self.info.height,
            [190.0, 0.75, 215.0, 1.25],
        );
        self.defense.block = skill_linear_interpolation(
            self.defense.block,
            self.info.height,
            [190.0, 0.75, 215.0, 1.25],
        );

        self.mental.vision =
            skill_linear_interpolation(self.mental.vision, self.info.age, [16.0, 0.5, 38.0, 1.75]);
        self.mental.charisma = skill_linear_interpolation(
            self.mental.charisma,
            self.info.age,
            [16.0, 0.75, 38.0, 1.25],
        );

        self.athleticism.stamina = skill_linear_interpolation(
            self.athleticism.stamina,
            self.info.age,
            [16.0, 1.3, 44.0, 0.7],
        );

        if self.info.first_name == "Costantino" && self.info.last_name == "Frittura" {
            self.athleticism.vertical = MAX_SKILL;
            self.offense.dunk = MAX_SKILL;
            self.offense.long_range = MAX_SKILL;
            self.defense.steal = MAX_SKILL;
            self.technical.ball_handling = MAX_SKILL;
            self.technical.post_moves = MAX_SKILL;
            self.mental.vision = MAX_SKILL;
            self.info.age = 35.0;
        } else if self.info.first_name == "Neko" && self.info.last_name == "Neko" {
            self.athleticism.quickness = MAX_SKILL;
            self.offense.close_range = MAX_SKILL;
            self.defense.steal = MAX_SKILL;
            self.technical.ball_handling = MAX_SKILL;
            self.mental.vision = MAX_SKILL;
            self.mental.off_ball_movement = MAX_SKILL;
            self.info.age = 16.0;
        }
    }

    pub fn set_jersey(&mut self, jersey: &Jersey) {
        self.image.set_jersey(jersey, &self.info);
        self.version += 1;
    }

    pub fn compose_image(&self) -> Result<Gif, Box<dyn std::error::Error>> {
        self.image.compose(&self.info)
    }

    fn skill_at_index(&self, idx: usize) -> Skill {
        match idx {
            0 => self.athleticism.quickness,
            1 => self.athleticism.vertical,
            2 => self.athleticism.strength,
            3 => self.athleticism.stamina,
            4 => self.offense.dunk,
            5 => self.offense.close_range,
            6 => self.offense.medium_range,
            7 => self.offense.long_range,
            8 => self.defense.steal,
            9 => self.defense.block,
            10 => self.defense.perimeter_defense,
            11 => self.defense.interior_defense,
            12 => self.technical.passing,
            13 => self.technical.ball_handling,
            14 => self.technical.post_moves,
            15 => self.technical.rebounding,
            16 => self.mental.vision,
            17 => self.mental.positioning,
            18 => self.mental.off_ball_movement,
            19 => self.mental.charisma,
            _ => panic!("Invalid skill index"),
        }
    }

    pub fn total_skills(&self) -> u16 {
        (0..20)
            .map(|idx| self.skill_at_index(idx).value() as u16)
            .sum::<u16>()
    }

    pub fn has_hat(&self) -> bool {
        self.image.hat.is_some()
    }

    pub fn has_wooden_leg(&self) -> bool {
        self.image.wooden_leg.is_some()
    }

    pub fn has_eye_patch(&self) -> bool {
        self.image.eye_patch.is_some()
    }

    pub fn has_hook(&self) -> bool {
        self.image.hook.is_some()
    }

    fn modify_skill(&mut self, idx: usize, mut value: f32) {
        // Quickness cannot improve beyond WOODEN_LEG_MAX_QUICKNESS if player has a wooden leg
        if self.has_wooden_leg()
            && idx == 0
            && self.athleticism.quickness >= WOODEN_LEG_MAX_QUICKNESS
        {
            return;
        }
        // Vision cannot improve beyond EYE_PATCH_MAX_VISION if player has an eye patch
        if self.has_eye_patch() && idx == 16 && self.mental.vision >= EYE_PATCH_MAX_VISION {
            return;
        }
        // Charisma improves quicker if player has an eye patch
        if self.has_eye_patch() && idx == 19 {
            value *= 1.5;
        }

        // Ball handling cannot improve beyond HOOK_MAX_BALL_HANDLING if player has a hook
        if self.has_hook() && idx == 13 && self.technical.ball_handling >= HOOK_MAX_BALL_HANDLING {
            return;
        }
        // Strength improves quicker if player has a hook
        if self.has_hook() && idx == 2 {
            value *= 1.5;
        }

        let new_value = (self.skill_at_index(idx) + value).bound();
        match idx {
            0 => self.athleticism.quickness = new_value,
            1 => self.athleticism.vertical = new_value,
            2 => self.athleticism.strength = new_value,
            3 => self.athleticism.stamina = new_value,
            4 => self.offense.dunk = new_value,
            5 => self.offense.close_range = new_value,
            6 => self.offense.medium_range = new_value,
            7 => self.offense.long_range = new_value,
            8 => self.defense.steal = new_value,
            9 => self.defense.block = new_value,
            10 => self.defense.perimeter_defense = new_value,
            11 => self.defense.interior_defense = new_value,
            12 => self.technical.passing = new_value,
            13 => self.technical.ball_handling = new_value,
            14 => self.technical.post_moves = new_value,
            15 => self.technical.rebounding = new_value,
            16 => self.mental.vision = new_value,
            17 => self.mental.positioning = new_value,
            18 => self.mental.off_ball_movement = new_value,
            19 => self.mental.charisma = new_value,
            _ => panic!("Invalid skill index {}", idx),
        }
    }

    pub fn apply_end_of_game_logic(&mut self, experience_at_position: &[u16; 5], tiredness: f32) {
        self.reputation = (self.reputation
            + REPUTATION_PER_EXPERIENCE / self.reputation
                * experience_at_position.iter().sum::<u16>() as f32
                * self.mental.charisma)
            .bound();

        let mut experience_per_skill: [u16; 20] =
            (0..20).map(|_| 0).collect::<Vec<u16>>().try_into().unwrap();

        for p in 0..MAX_POSITION {
            for (idx, &w) in p.weights().iter().enumerate() {
                experience_per_skill[idx] += experience_at_position[p as usize] * w as u16;
            }
        }

        self.tiredness = tiredness;

        self.previous_skills = self.current_skill_array();

        for idx in 0..20 {
            let mut increment = experience_per_skill[idx] as f32 * EXPERIENCE_PER_SKILL_MULTIPLIER;
            match self.training_focus {
                Some(focus) => {
                    if focus.is_focus(idx) {
                        increment *= 2.0;
                    } else {
                        increment *= 0.5;
                    }
                }
                None => {}
            }
            self.modify_skill(idx, increment);
        }

        self.version += 1;
    }
}

impl Rated for Player {
    fn rating(&self) -> u8 {
        let mut ratings = (0..MAX_POSITION)
            .map(|p: Position| p.player_rating(self.current_skill_array()) as u8)
            .collect::<Vec<u8>>();
        ratings.sort();
        ratings.reverse();
        ratings[0]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InfoStats {
    pub first_name: String,
    pub last_name: String,
    pub crew_role: CrewRole,
    pub home_planet_id: PlanetId,
    pub population: Population,
    pub age: f32,
    pub pronouns: Pronoun,
    pub height: f32,
    pub weight: f32,
}

impl InfoStats {
    pub fn for_position(
        position: Option<Position>,
        rng: &mut ChaCha8Rng,
        home_planet: &Planet,
    ) -> Self {
        let p_data = PLAYER_DATA.as_ref().unwrap();
        let population = home_planet.random_population(rng).unwrap_or_default();
        let pronouns = if population == Population::Polpett {
            Pronoun::They
        } else {
            Pronoun::random()
        };
        let idx = population as usize;
        let first_name = match pronouns {
            Pronoun::He => p_data.first_names_he[idx].choose(rng).unwrap().to_string(),
            Pronoun::She => p_data.first_names_she[idx].choose(rng).unwrap().to_string(),
            Pronoun::They => match rng.gen_range(0..2) {
                0 => p_data.first_names_he[idx].choose(rng).unwrap().to_string(),
                _ => p_data.first_names_she[idx].choose(rng).unwrap().to_string(),
            },
        };
        let last_name = p_data.last_names[idx].choose(rng).unwrap().to_string();
        let age = rng.gen_range(16..=38) as f32;
        let height = match position {
            Some(x) => Normal::new(192.0 + 3.5 * x as f32, 5.0)
                .unwrap()
                .sample(rng),
            None => rng.gen_range(180..=220) as f32,
        };
        let bmi = rng.gen_range(12..22) as f32 + height / 20.0;
        let weight = bmi * height * height / 10000.0;

        Self {
            first_name,
            last_name,
            crew_role: CrewRole::default(),
            home_planet_id: home_planet.id,
            population,
            age,
            pronouns,
            height,
            weight,
        }
    }
}

#[cfg(test)]

mod tests {
    use crate::{
        types::{IdSystem, PlayerId},
        world::{planet::Planet, player::Player, skill::GameSkill},
    };
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_apply_end_of_game_logic() {
        let mut player = Player::random(
            &mut ChaCha8Rng::from_seed([0; 32]),
            PlayerId::new(),
            None,
            &Planet::default(),
            0.0,
        );
        let skills_before = (0..20)
            .map(|idx| player.skill_at_index(idx).raw_value())
            .sum::<f32>();

        println!("quickness before: {}", player.skill_at_index(0).raw_value());

        let experience_at_position = [1000, 1000, 1000, 1000, 1000];
        player.apply_end_of_game_logic(&experience_at_position, 0.0);
        assert_eq!(player.version, 1);

        let skills_after = (0..20)
            .map(|idx| player.skill_at_index(idx).raw_value())
            .sum::<f32>();

        println!("quickness after: {}", player.skill_at_index(0).raw_value());

        println!("skills before: {}", skills_before);

        println!("skills after: {}", skills_after);
        assert!(skills_after > skills_before);
    }
}
