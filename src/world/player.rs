use std::cmp::min;

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
    engine::constants::MAX_TIREDNESS,
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
use serde::{de::Visitor, ser::SerializeStruct, Deserialize, Serialize};
use strum_macros::Display;

const HOOK_MAX_BALL_HANDLING: f32 = 4.0;
const EYE_PATCH_MAX_VISION: f32 = 4.0;
const WOODEN_LEG_MAX_QUICKNESS: f32 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub peer_id: Option<PeerId>,
    pub version: u64,
    pub info: InfoStats,
    pub team: Option<TeamId>,
    pub special_trait: Option<Trait>,
    pub reputation: f32,
    pub playing_style: PlayingStyle,
    pub athleticism: Athleticism,
    pub offense: Offense,
    pub defense: Defense,
    pub technical: Technical,
    pub mental: Mental,
    pub image: PlayerImage,
    pub current_location: PlayerLocation,
    pub previous_skills: [Skill; 20], // This is for displaying purposes to show the skills that were recently modified
    pub training_focus: Option<TrainingFocus>,
    pub tiredness: f32,
}

impl Serialize for Player {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Don't serialize athleticism, offense, technical, defense, mental
        // and serialize them in a vector which is then deserialized
        // into the corresponding fields
        let compact_skills = self.current_skill_array().to_vec();
        let mut state = serializer.serialize_struct("Player", 14)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("peer_id", &self.peer_id)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("info", &self.info)?;
        state.serialize_field("team", &self.team)?;
        state.serialize_field("special_trait", &self.special_trait)?;
        state.serialize_field("reputation", &self.reputation)?;
        state.serialize_field("playing_style", &self.playing_style)?;
        state.serialize_field("image", &self.image)?;
        state.serialize_field("current_location", &self.current_location)?;
        state.serialize_field("previous_skills", &self.previous_skills)?;
        state.serialize_field("training_focus", &self.training_focus)?;
        state.serialize_field("tiredness", &self.tiredness)?;
        state.serialize_field("compact_skills", &compact_skills)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Player {
    // Deserialize compact_skills into the corresponding fields.
    // compact_skills is a vector of 20 skills.
    // The first 4 skills are athleticism, the next 4 are offense, etc.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        enum Field {
            Id,
            PeerId,
            Version,
            Info,
            Team,
            SpecialTrait,
            Reputation,
            PlayingStyle,
            Image,
            CurrentLocation,
            PreviousSkills,
            TrainingFocus,
            Tiredness,
            CompactSkills,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("field name")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "id" => Ok(Field::Id),
                            "peer_id" => Ok(Field::PeerId),
                            "version" => Ok(Field::Version),
                            "info" => Ok(Field::Info),
                            "team" => Ok(Field::Team),
                            "special_trait" => Ok(Field::SpecialTrait),
                            "reputation" => Ok(Field::Reputation),
                            "playing_style" => Ok(Field::PlayingStyle),
                            "image" => Ok(Field::Image),
                            "current_location" => Ok(Field::CurrentLocation),
                            "previous_skills" => Ok(Field::PreviousSkills),
                            "training_focus" => Ok(Field::TrainingFocus),
                            "tiredness" => Ok(Field::Tiredness),
                            "compact_skills" => Ok(Field::CompactSkills),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct PlayerVisitor;

        impl<'de> Visitor<'de> for PlayerVisitor {
            type Value = Player;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Player")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Player, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let id = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let peer_id = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let version = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                let info = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(3, &self))?;
                let team = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(4, &self))?;
                let special_trait = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(5, &self))?;
                let reputation = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(6, &self))?;
                let playing_style = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(7, &self))?;
                let image = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(8, &self))?;
                let current_location = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(9, &self))?;
                let previous_skills = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(10, &self))?;
                let training_focus = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(11, &self))?;
                let tiredness = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(12, &self))?;
                let compact_skills: Vec<Skill> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(13, &self))?;

                let mut player = Player {
                    id,
                    peer_id,
                    version,
                    info,
                    team,
                    special_trait,
                    reputation,
                    playing_style,
                    athleticism: Athleticism::default(),
                    offense: Offense::default(),
                    defense: Defense::default(),
                    technical: Technical::default(),
                    mental: Mental::default(),
                    image,
                    current_location,
                    previous_skills,
                    training_focus,
                    tiredness,
                };

                player.athleticism = Athleticism {
                    quickness: compact_skills[0],
                    vertical: compact_skills[1],
                    strength: compact_skills[2],
                    stamina: compact_skills[3],
                };
                player.offense = Offense {
                    dunk: compact_skills[4],
                    close_range: compact_skills[5],
                    medium_range: compact_skills[6],
                    long_range: compact_skills[7],
                };
                player.defense = Defense {
                    steal: compact_skills[8],
                    block: compact_skills[9],
                    perimeter_defense: compact_skills[10],
                    interior_defense: compact_skills[11],
                };
                player.technical = Technical {
                    passing: compact_skills[12],
                    ball_handling: compact_skills[13],
                    post_moves: compact_skills[14],
                    rebounding: compact_skills[15],
                };
                player.mental = Mental {
                    vision: compact_skills[16],
                    aggression: compact_skills[17],
                    off_ball_movement: compact_skills[18],
                    charisma: compact_skills[19],
                };

                Ok(player)
            }

            fn visit_map<V>(self, mut map: V) -> Result<Player, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut id = None;
                let mut peer_id = None;
                let mut version = None;
                let mut info = None;
                let mut team = None;
                let mut special_trait = None;
                let mut reputation = None;
                let mut playing_style = None;
                let mut image = None;
                let mut current_location = None;
                let mut previous_skills = None;
                let mut training_focus = None;
                let mut tiredness = None;
                let mut compact_skills: Option<Vec<Skill>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Id => {
                            if id.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id = Some(map.next_value()?);
                        }
                        Field::PeerId => {
                            if peer_id.is_some() {
                                return Err(serde::de::Error::duplicate_field("peer_id"));
                            }
                            peer_id = Some(map.next_value()?);
                        }
                        Field::Version => {
                            if version.is_some() {
                                return Err(serde::de::Error::duplicate_field("version"));
                            }
                            version = Some(map.next_value()?);
                        }
                        Field::Info => {
                            if info.is_some() {
                                return Err(serde::de::Error::duplicate_field("info"));
                            }
                            info = Some(map.next_value()?);
                        }
                        Field::Team => {
                            if team.is_some() {
                                return Err(serde::de::Error::duplicate_field("team"));
                            }
                            team = Some(map.next_value()?);
                        }
                        Field::SpecialTrait => {
                            if special_trait.is_some() {
                                return Err(serde::de::Error::duplicate_field("special_trait"));
                            }
                            special_trait = Some(map.next_value()?);
                        }
                        Field::Reputation => {
                            if reputation.is_some() {
                                return Err(serde::de::Error::duplicate_field("reputation"));
                            }
                            reputation = Some(map.next_value()?);
                        }
                        Field::PlayingStyle => {
                            if playing_style.is_some() {
                                return Err(serde::de::Error::duplicate_field("playing_style"));
                            }
                            playing_style = Some(map.next_value()?);
                        }
                        Field::Image => {
                            if image.is_some() {
                                return Err(serde::de::Error::duplicate_field("image"));
                            }
                            image = Some(map.next_value()?);
                        }
                        Field::CurrentLocation => {
                            if current_location.is_some() {
                                return Err(serde::de::Error::duplicate_field("current_location"));
                            }
                            current_location = Some(map.next_value()?);
                        }
                        Field::PreviousSkills => {
                            if previous_skills.is_some() {
                                return Err(serde::de::Error::duplicate_field("previous_skills"));
                            }
                            previous_skills = Some(map.next_value()?);
                        }
                        Field::TrainingFocus => {
                            if training_focus.is_some() {
                                return Err(serde::de::Error::duplicate_field("training_focus"));
                            }
                            training_focus = Some(map.next_value()?);
                        }
                        Field::Tiredness => {
                            if tiredness.is_some() {
                                return Err(serde::de::Error::duplicate_field("tiredness"));
                            }
                            tiredness = Some(map.next_value()?);
                        }
                        Field::CompactSkills => {
                            if compact_skills.is_some() {
                                return Err(serde::de::Error::duplicate_field("compact_skills"));
                            }
                            compact_skills = Some(map.next_value()?);
                        }
                    }
                }

                let id = id.ok_or_else(|| serde::de::Error::missing_field("id"))?;
                let peer_id = peer_id.ok_or_else(|| serde::de::Error::missing_field("peer_id"))?;
                let version = version.ok_or_else(|| serde::de::Error::missing_field("version"))?;
                let info = info.ok_or_else(|| serde::de::Error::missing_field("info"))?;
                let team = team.ok_or_else(|| serde::de::Error::missing_field("team"))?;
                let special_trait = special_trait
                    .ok_or_else(|| serde::de::Error::missing_field("special_trait"))?;
                let reputation =
                    reputation.ok_or_else(|| serde::de::Error::missing_field("reputation"))?;
                let playing_style = playing_style
                    .ok_or_else(|| serde::de::Error::missing_field("playing_style"))?;
                let image = image.ok_or_else(|| serde::de::Error::missing_field("image"))?;
                let current_location = current_location
                    .ok_or_else(|| serde::de::Error::missing_field("current_location"))?;
                let previous_skills = previous_skills
                    .ok_or_else(|| serde::de::Error::missing_field("previous_skills"))?;
                let training_focus = training_focus
                    .ok_or_else(|| serde::de::Error::missing_field("training_focus"))?;
                let tiredness =
                    tiredness.ok_or_else(|| serde::de::Error::missing_field("tiredness"))?;
                let compact_skills = compact_skills
                    .ok_or_else(|| serde::de::Error::missing_field("compact_skills"))?;

                let mut player = Player {
                    id,
                    peer_id,
                    version,
                    info,
                    team,
                    special_trait,
                    reputation,
                    playing_style,
                    athleticism: Athleticism::default(),
                    offense: Offense::default(),
                    defense: Defense::default(),
                    technical: Technical::default(),
                    mental: Mental::default(),
                    image,
                    current_location,
                    previous_skills,
                    training_focus,
                    tiredness,
                };

                player.athleticism = Athleticism {
                    quickness: compact_skills[0],
                    vertical: compact_skills[1],
                    strength: compact_skills[2],
                    stamina: compact_skills[3],
                };
                player.offense = Offense {
                    dunk: compact_skills[4],
                    close_range: compact_skills[5],
                    medium_range: compact_skills[6],
                    long_range: compact_skills[7],
                };
                player.defense = Defense {
                    steal: compact_skills[8],
                    block: compact_skills[9],
                    perimeter_defense: compact_skills[10],
                    interior_defense: compact_skills[11],
                };
                player.technical = Technical {
                    passing: compact_skills[12],
                    ball_handling: compact_skills[13],
                    post_moves: compact_skills[14],
                    rebounding: compact_skills[15],
                };
                player.mental = Mental {
                    vision: compact_skills[16],
                    aggression: compact_skills[17],
                    off_ball_movement: compact_skills[18],
                    charisma: compact_skills[19],
                };

                Ok(player)
            }
        }

        const FIELDS: &'static [&'static str] = &[
            "id",
            "peer_id",
            "version",
            "info",
            "team",
            "jersey_number",
            "reputation",
            "playing_style",
            "image",
            "current_location",
            "previous_skills",
            "training_focus",
            "tiredness",
            "compact_skills",
        ];
        deserializer.deserialize_struct("Player", FIELDS, PlayerVisitor)
    }
}

impl Player {
    pub fn current_skill_array(&self) -> [Skill; 20] {
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
        if 2.0 * self.reputation <= team_reputation {
            return 0;
        }

        let special_trait_extra = if self.special_trait.is_some() { 2 } else { 1 };

        COST_PER_VALUE
            * special_trait_extra
            * self.player_value()
            * (2.0 * self.reputation - team_reputation) as u32
    }

    pub fn release_cost(&self) -> u32 {
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

        let info = InfoStats::for_position(position, None, rng, home_planet);
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
            special_trait: None,
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

        player.apply_info_modifiers();

        player
            .info
            .population
            .apply_skill_modifiers(&mut player.clone());

        if athleticism.quickness < WOODEN_LEG_MAX_QUICKNESS {
            player.image.set_wooden_leg(rng);
            player.mental.charisma = (player.mental.charisma + 1.0).bound();
        }
        if mental.vision < EYE_PATCH_MAX_VISION {
            player.image.set_eye_patch(rng, population);
            player.mental.charisma = (player.mental.charisma + 1.0).bound();
        }

        if technical.ball_handling < HOOK_MAX_BALL_HANDLING {
            player.image.set_hook(rng, population);
            player.mental.charisma = (player.mental.charisma + 1.0).bound();
        }

        if athleticism.strength > 15.0 && rng.gen_range(0..10) < 2 {
            player.special_trait = Some(Trait::Killer);
        } else if mental.charisma > 15.0 && rng.gen_range(0..10) < 2 {
            player.special_trait = Some(Trait::Showpirate);
        } else if mental.vision > 15.0 && rng.gen_range(0..10) < 2 {
            player.special_trait = Some(Trait::Merchant);
        } else if athleticism.stamina > 15.0 && rng.gen_range(0..10) < 2 {
            player.special_trait = Some(Trait::Relentless);
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
            17 => self.mental.aggression,
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

    pub fn is_knocked_out(&self) -> bool {
        self.tiredness == MAX_TIREDNESS
    }

    pub fn add_tiredness(&mut self, tiredness: f32) {
        let max_tiredness = if self.special_trait == Some(Trait::Relentless) {
            MAX_TIREDNESS - 1.0
        } else {
            MAX_TIREDNESS
        };
        self.tiredness = (self.tiredness + tiredness / (1.0 + self.athleticism.stamina / 20.0))
            .min(max_tiredness);
    }

    pub fn roll(&self, rng: &mut ChaCha8Rng) -> u8 {
        if self.tiredness == MAX_TIREDNESS {
            return 0;
        }
        min(
            ((MAX_TIREDNESS - self.tiredness / 2.0) / 2.0).round() as u8,
            rng.gen_range(1..=50),
        )
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
            17 => self.mental.aggression = new_value,
            18 => self.mental.off_ball_movement = new_value,
            19 => self.mental.charisma = new_value,
            _ => panic!("Invalid skill index {}", idx),
        }
    }

    pub fn apply_end_of_game_logic(
        &mut self,
        experience_at_position: [u16; MAX_POSITION as usize],
    ) {
        self.version += 1;
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

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
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
        population: Option<Population>,
        rng: &mut ChaCha8Rng,
        home_planet: &Planet,
    ) -> Self {
        let population = match population {
            Some(p) => p,
            None => home_planet.random_population(rng).unwrap(),
        };
        let p_data = PLAYER_DATA.get(&population).unwrap();
        let pronouns = if population == Population::Polpett {
            Pronoun::They
        } else {
            Pronoun::random()
        };
        let first_name = match pronouns {
            Pronoun::He => p_data.first_names_he.choose(rng).unwrap().to_string(),
            Pronoun::She => p_data.first_names_she.choose(rng).unwrap().to_string(),
            Pronoun::They => match rng.gen_range(0..2) {
                0 => p_data.first_names_he.choose(rng).unwrap().to_string(),
                _ => p_data.first_names_she.choose(rng).unwrap().to_string(),
            },
        };
        let last_name = p_data.last_names.choose(rng).unwrap().to_string();
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Display)]
pub enum Trait {
    Killer,
    Merchant,
    Relentless,
    Showpirate,
    Explorator,
}

impl Trait {
    pub fn description(&self, player: &Player) -> String {
        match self {
            Trait::Killer => format!(
                "Bonus when brawling in a match based on reputation (+{}).",
                player.reputation.value()
            ),
            Trait::Merchant => format!(
                "Cheaper ship upgrades based on reputation (-{}%)",
                player.reputation.value()
            ),
            Trait::Relentless => format!("Cannot get exhausted"),
            Trait::Showpirate => {
                format!(
                    "Increase games attendance based on reputation (+{}%)",
                    player.reputation.value()
                )
            }
            Trait::Explorator => format!(
                "Higher chance of finding something during exploration based on reputation (+{}%)",
                player.reputation.value()
            ),
        }
    }
}
