use super::{
    constants::*,
    jersey::Jersey,
    position::{GamePosition, GamePositionUtils, MAX_GAME_POSITION},
    resources::Resource,
    role::CrewRole,
    skill::*,
    types::{PlayerLocation, Population, Pronoun, Region, TrainingFocus},
    utils::{skill_linear_interpolation, PLAYER_DATA},
    world::World,
};
use crate::{
    core::PLANET_DATA,
    game_engine::types::GameStats,
    image::{player::PlayerImage, utils::Gif},
    types::{AppResult, HashMapWithResult, PlanetId, PlayerId, StorableResourceMap, TeamId},
};
use anyhow::anyhow;

use libp2p::PeerId;
use rand::{
    seq::{IndexedRandom, IteratorRandom},
    Rng, SeedableRng,
};
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal};
use serde::{de::Visitor, ser::SerializeStruct, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum_macros::Display;

const HOOK_MAX_BALL_HANDLING: f32 = 4.0;
const EYE_PATCH_MAX_VISION: f32 = 4.0;
const WOODEN_LEG_MAX_QUICKNESS: f32 = 4.0;

#[derive(Debug, Clone, Default, PartialEq)]
struct PlayerBuildData {
    position: Option<GamePosition>,
    base_level: f32,
    population: Option<Population>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub peer_id: Option<PeerId>,
    pub version: u64,
    pub info: InfoStats,
    pub team: Option<TeamId>,
    pub special_trait: Option<Trait>,
    pub reputation: f32,
    pub potential: Skill,
    pub athletics: Athletics,
    pub offense: Offense,
    pub defense: Defense,
    pub technical: Technical,
    pub mental: Mental,
    pub image: PlayerImage,
    pub current_location: PlayerLocation,
    pub skills_training: [f32; 20],
    pub previous_skills: [Skill; 20], // This is for displaying purposes to show the skills that were recently modified
    // pub skills_potential: [Skill; 20], // Each skill has a separate potential. For retrocompatibility reasons, we allow this array to be all zeros, in which case we initialize it during deserialization.
    pub tiredness: f32,
    pub morale: f32,
    pub historical_stats: GameStats,
    build_data: PlayerBuildData, // Intermediate state used to build the random player. Not serialized
}

impl Default for Player {
    fn default() -> Self {
        Player {
            id: PlayerId::new_v4(),
            peer_id: None,
            version: 0,
            info: InfoStats::default(),
            team: None,
            special_trait: None,
            reputation: Skill::default(),
            potential: Skill::default(),
            athletics: Athletics::default(),
            offense: Offense::default(),
            defense: Defense::default(),
            technical: Technical::default(),
            mental: Mental::default(),
            image: PlayerImage::default(),
            current_location: PlayerLocation::default(),
            skills_training: [Skill::default(); 20],
            previous_skills: [Skill::default(); 20],
            tiredness: Skill::default(),
            morale: Skill::default(),
            historical_stats: GameStats::default(),
            build_data: PlayerBuildData::default(),
        }
    }
}

impl Serialize for Player {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Don't serialize athletics, offense, technical, defense, mental
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
        state.serialize_field("potential", &self.potential)?;
        state.serialize_field("image", &self.image)?;
        state.serialize_field("current_location", &self.current_location)?;
        state.serialize_field("previous_skills", &self.previous_skills)?;
        state.serialize_field("skills_training", &self.skills_training)?;
        state.serialize_field("tiredness", &self.tiredness)?;
        state.serialize_field("morale", &self.morale)?;
        state.serialize_field("compact_skills", &compact_skills)?;
        state.serialize_field("historical_stats", &self.historical_stats)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Player {
    // Deserialize compact_skills into the corresponding fields.
    // compact_skills is a vector of 20 skills.
    // The first 4 skills are athletics, the next 4 are offense, etc.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        enum Field {
            Id,
            PeerId,
            Version,
            Info,
            Team,
            SpecialTrait,
            Reputation,
            Potential,
            Image,
            CurrentLocation,
            PreviousSkills,
            SkillsTraining,
            Tiredness,
            Morale,
            CompactSkills,
            HistoricalStats,
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
                            "potential" => Ok(Field::Potential),
                            "image" => Ok(Field::Image),
                            "current_location" => Ok(Field::CurrentLocation),
                            "previous_skills" => Ok(Field::PreviousSkills),
                            "skills_training" => Ok(Field::SkillsTraining),
                            "tiredness" => Ok(Field::Tiredness),
                            "morale" => Ok(Field::Morale),
                            "compact_skills" => Ok(Field::CompactSkills),
                            "historical_stats" => Ok(Field::HistoricalStats),
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
                let potential = seq
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
                let skills_training = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(11, &self))?;
                let tiredness = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(12, &self))?;
                let morale = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(13, &self))?;
                let compact_skills: Vec<Skill> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(14, &self))?;
                let historical_stats = seq.next_element()?.unwrap_or_default();

                let mut player = Player {
                    id,

                    peer_id,
                    version,
                    info,
                    team,
                    special_trait,
                    reputation,
                    potential,
                    athletics: Athletics::default(),
                    offense: Offense::default(),
                    defense: Defense::default(),
                    technical: Technical::default(),
                    mental: Mental::default(),
                    image,
                    current_location,
                    skills_training,
                    previous_skills,
                    tiredness,
                    morale,
                    historical_stats,
                    build_data: PlayerBuildData::default(),
                };

                player.athletics = Athletics {
                    quickness: compact_skills[0],
                    vertical: compact_skills[1],
                    strength: compact_skills[2],
                    stamina: compact_skills[3],
                };
                player.offense = Offense {
                    brawl: compact_skills[4],
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
                    rebounds: compact_skills[15],
                };
                player.mental = Mental {
                    vision: compact_skills[16],
                    aggression: compact_skills[17],
                    intuition: compact_skills[18],
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
                let mut potential = None;
                let mut image = None;
                let mut current_location = None;
                let mut skills_training = None;
                let mut previous_skills = None;
                let mut tiredness = None;
                let mut morale = None;
                let mut compact_skills: Option<Vec<Skill>> = None;
                let mut historical_stats = None;

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
                        Field::Potential => {
                            if potential.is_some() {
                                return Err(serde::de::Error::duplicate_field("potential"));
                            }
                            potential = Some(map.next_value()?);
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
                        Field::SkillsTraining => {
                            if skills_training.is_some() {
                                return Err(serde::de::Error::duplicate_field("skills_training"));
                            }
                            skills_training = Some(map.next_value()?);
                        }
                        Field::PreviousSkills => {
                            if previous_skills.is_some() {
                                return Err(serde::de::Error::duplicate_field("previous_skills"));
                            }
                            previous_skills = Some(map.next_value()?);
                        }
                        Field::Tiredness => {
                            if tiredness.is_some() {
                                return Err(serde::de::Error::duplicate_field("tiredness"));
                            }
                            tiredness = Some(map.next_value()?);
                        }
                        Field::Morale => {
                            if morale.is_some() {
                                return Err(serde::de::Error::duplicate_field("morale"));
                            }
                            morale = Some(map.next_value()?);
                        }
                        Field::CompactSkills => {
                            if compact_skills.is_some() {
                                return Err(serde::de::Error::duplicate_field("compact_skills"));
                            }
                            compact_skills = Some(map.next_value()?);
                        }

                        Field::HistoricalStats => {
                            if historical_stats.is_some() {
                                return Err(serde::de::Error::duplicate_field("historical_statis"));
                            }
                            historical_stats = Some(map.next_value()?);
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
                let potential =
                    potential.ok_or_else(|| serde::de::Error::missing_field("potential"))?;
                let image = image.ok_or_else(|| serde::de::Error::missing_field("image"))?;
                let current_location = current_location
                    .ok_or_else(|| serde::de::Error::missing_field("current_location"))?;
                let skills_training = skills_training
                    .ok_or_else(|| serde::de::Error::missing_field("skills_training"))?;
                let previous_skills = previous_skills
                    .ok_or_else(|| serde::de::Error::missing_field("previous_skills"))?;
                let tiredness =
                    tiredness.ok_or_else(|| serde::de::Error::missing_field("tiredness"))?;
                let morale = morale.ok_or_else(|| serde::de::Error::missing_field("morale"))?;
                let compact_skills = compact_skills
                    .ok_or_else(|| serde::de::Error::missing_field("compact_skills"))?;
                let historical_stats = historical_stats.unwrap_or_default();

                let mut player = Player {
                    id,

                    peer_id,
                    version,
                    info,
                    team,
                    special_trait,
                    reputation,
                    potential,
                    athletics: Athletics::default(),
                    offense: Offense::default(),
                    defense: Defense::default(),
                    technical: Technical::default(),
                    mental: Mental::default(),
                    image,
                    current_location,
                    skills_training,
                    previous_skills,
                    tiredness,
                    morale,
                    historical_stats,
                    build_data: PlayerBuildData::default(),
                };

                player.athletics = Athletics {
                    quickness: compact_skills[0],
                    vertical: compact_skills[1],
                    strength: compact_skills[2],
                    stamina: compact_skills[3],
                };
                player.offense = Offense {
                    brawl: compact_skills[4],
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
                    rebounds: compact_skills[15],
                };
                player.mental = Mental {
                    vision: compact_skills[16],
                    aggression: compact_skills[17],
                    intuition: compact_skills[18],
                    charisma: compact_skills[19],
                };

                Ok(player)
            }
        }

        const FIELDS: &[&str] = &[
            "id",
            "peer_id",
            "version",
            "info",
            "team",
            "jersey_number",
            "reputation",
            "potential",
            "image",
            "current_location",
            "skills_training",
            "previous_skills",
            "tiredness",
            "morale",
            "compact_skills",
        ];
        deserializer.deserialize_struct("Player", FIELDS, PlayerVisitor)
    }
}

impl Player {
    pub fn randomize(mut self, rng: Option<&mut ChaCha8Rng>) -> Self {
        let rng = if let Some(r) = rng {
            r
        } else {
            &mut ChaCha8Rng::from_os_rng()
        };

        if self.info.home_planet_id == PlanetId::default() {
            let home_planet = PLANET_DATA
                .iter()
                .filter(|p| p.total_population() > 0)
                .choose(rng)
                .expect("There should be a planet.");
            self.info.home_planet_id = home_planet.id;
        }

        let mut build_base_level = self.build_data.base_level;
        let position = if let Some(pos) = self.build_data.position {
            pos
        } else {
            rng.random_range(0..MAX_GAME_POSITION)
        };

        self.info.population = if let Some(population) = self.build_data.population {
            population
        } else {
            let home_planet = PLANET_DATA
                .iter()
                .find(|p| p.id == self.info.home_planet_id)
                .expect("There should be a planet.");

            home_planet.random_population(rng).unwrap_or_default()
        };

        self.info.randomize_for_position(position, rng);

        // Base level modifier increases linearly from (0,0) to (PEAK_PERFORMANCE_RELATIVE_AGE, 1.0),
        // then decreases linearly from (PEAK_PERFORMANCE_RELATIVE_AGE, 1.0) to (1.0, 0.0).
        let base_level_modifier = if PEAK_PERFORMANCE_RELATIVE_AGE >= self.info.relative_age() {
            self.info.relative_age() / PEAK_PERFORMANCE_RELATIVE_AGE
        } else {
            (self.info.relative_age() - 1.0) / (PEAK_PERFORMANCE_RELATIVE_AGE - 1.0)
        };
        build_base_level *= base_level_modifier;

        self.athletics = Athletics::for_position(position, rng, build_base_level);
        self.offense = Offense::for_position(position, rng, build_base_level);
        self.technical = Technical::for_position(position, rng, build_base_level);
        self.defense = Defense::for_position(position, rng, build_base_level);
        self.mental = Mental::for_position(position, rng, build_base_level);

        self.image = PlayerImage::from_info(&self.info, rng);

        self.apply_population_skill_modifiers();
        self.apply_info_skill_modifiers();

        if self.athletics.quickness < WOODEN_LEG_MAX_QUICKNESS {
            self.image.set_wooden_leg(rng);
            self.mental.charisma = (self.mental.charisma + 1.25).bound();
            self.technical.post_moves = (self.technical.post_moves + 0.75).bound();
        }
        if self.mental.vision < EYE_PATCH_MAX_VISION {
            self.image.set_eye_patch(rng, self.info.population);
            self.mental.charisma = (self.mental.charisma + 2.0).bound();
        }

        if self.technical.ball_handling < HOOK_MAX_BALL_HANDLING {
            self.image.set_hook(rng, self.info.population);
            self.athletics.strength = (self.athletics.strength + 1.25).bound();
            self.mental.charisma = (self.mental.charisma + 0.75).bound();
        }

        if self.athletics.strength > 15.0 && rng.random_bool(TRAIT_PROBABILITY) {
            self.special_trait = Some(Trait::Killer);
        } else if self.mental.charisma > 15.0 && rng.random_bool(TRAIT_PROBABILITY) {
            self.special_trait = Some(Trait::Showpirate);
        } else if self.mental.intuition > 10.0 && rng.random_bool(TRAIT_PROBABILITY) {
            self.special_trait = Some(Trait::Spugna);
        } else if self.athletics.stamina > 15.0 && rng.random_bool(TRAIT_PROBABILITY) {
            self.special_trait = Some(Trait::Relentless);
        }

        self.previous_skills = self.current_skill_array();

        // Extra potential has a variance that depends
        let std_dev = 3.0 + 1.0 - self.info.relative_age();
        let normal = Normal::new(0.0, std_dev).expect("Should create valid normal distribution");
        let extra_potential = normal.sample(rng).abs();
        self.potential = (self.average_skill() + extra_potential)
            .max(self.average_skill())
            .bound();
        self.reputation = (self.average_skill() / 5.0 + self.info.relative_age() * 5.0).bound();

        self
    }

    pub fn with_name(
        mut self,
        first_name: impl Into<String>,
        last_name: impl Into<String>,
    ) -> Self {
        self.info.first_name = first_name.into();
        self.info.last_name = last_name.into();
        self
    }

    pub fn with_home_planet(mut self, home_planet_id: PlanetId) -> Self {
        self.info.home_planet_id = home_planet_id;
        self.current_location = if self.team.is_some() {
            PlayerLocation::WithTeam
        } else {
            PlayerLocation::OnPlanet {
                planet_id: home_planet_id,
            }
        };

        self
    }

    pub fn with_population(mut self, population: Population) -> Self {
        self.info.population = population;
        self.build_data.population = Some(population);

        self
    }

    pub fn with_random_population(mut self, rng: Option<&mut ChaCha8Rng>) -> Self {
        let home_planet = PLANET_DATA
            .iter()
            .find(|p| p.id == self.info.home_planet_id)
            .expect("There should be a planet.");

        let rng = if let Some(r) = rng {
            r
        } else {
            &mut ChaCha8Rng::from_os_rng()
        };

        let population = home_planet.random_population(rng).unwrap_or_default();
        self.info.population = population;
        self.image = PlayerImage::from_info(&self.info, rng);

        self
    }

    pub fn with_position(mut self, position: Option<GamePosition>) -> Self {
        self.build_data.position = position;

        self
    }

    pub fn with_base_level(mut self, base_level: f32) -> Self {
        self.build_data.base_level = base_level;

        self
    }

    pub fn current_skill_array(&self) -> [Skill; 20] {
        (0..20)
            .map(|idx| self.skill_at_index(idx))
            .collect::<Vec<Skill>>()
            .try_into()
            .expect("There should be 20 skills")
    }

    pub fn current_tiredness(&self, world: &World) -> f32 {
        let mut tiredness = self.tiredness;
        // Check if player is currently playing.
        // In this case, read current tiredness from game.
        if let Some(team_id) = self.team {
            if let Ok(team) = world.teams.get_or_err(&team_id) {
                if let Some(game_id) = team.current_game {
                    if let Ok(game) = world.games.get_or_err(&game_id) {
                        if let Some(p) = if game.home_team_in_game.team_id == team_id {
                            game.home_team_in_game.players.get(&self.id)
                        } else {
                            game.away_team_in_game.players.get(&self.id)
                        } {
                            tiredness = p.tiredness;
                        }
                    }
                }
            }
        }

        tiredness
    }

    pub fn current_morale(&self, world: &World) -> f32 {
        let mut morale = self.morale;
        // Check if player is currently playing.
        // In this case, read current morale from game.
        if let Some(team_id) = self.team {
            if let Ok(team) = world.teams.get_or_err(&team_id) {
                if let Some(game_id) = team.current_game {
                    if let Ok(game) = world.games.get_or_err(&game_id) {
                        if let Some(p) = if game.home_team_in_game.team_id == team_id {
                            game.home_team_in_game.players.get(&self.id)
                        } else {
                            game.away_team_in_game.players.get(&self.id)
                        } {
                            morale = p.morale;
                        }
                    }
                }
            }
        }

        morale
    }

    pub fn can_drink(&self, world: &World) -> AppResult<()> {
        if self.team.is_none() {
            return Err(anyhow!("Player has no team, so no rum to drink"));
        }

        let team = world
            .teams
            .get_or_err(&self.team.expect("Player should have team"))?;

        if team.current_game.is_some() {
            return Err(anyhow!("Can't drink during game"));
        }

        // Spugna can drink ad libitum
        if self.morale == MAX_SKILL && !matches!(self.special_trait, Some(Trait::Spugna)) {
            return Err(anyhow!("No need to drink"));
        }

        if self.tiredness == MAX_SKILL {
            return Err(anyhow!("No energy to drink"));
        }

        if team.resources.value(&Resource::RUM) == 0 {
            return Err(anyhow!("No rum to drink"));
        }

        Ok(())
    }

    pub fn bare_hiring_value(&self) -> f32 {
        // Age modifier decrease linearly from (0,1.5) to (PEAK_PERFORMANCE_RELATIVE_AGE, 1.0),
        // then decreases linearly from (PEAK_PERFORMANCE_RELATIVE_AGE, 1.0) to (1.0, 0.5).
        let age_modifier = if PEAK_PERFORMANCE_RELATIVE_AGE >= self.info.relative_age() {
            1.5 - self.info.relative_age() / (2.0 * PEAK_PERFORMANCE_RELATIVE_AGE)
        } else {
            (1.0 - 0.5 * (self.info.relative_age() + PEAK_PERFORMANCE_RELATIVE_AGE))
                / (1.0 - PEAK_PERFORMANCE_RELATIVE_AGE)
        };

        let special_trait_extra = if self.special_trait.is_some() {
            SPECIAL_TRAIT_VALUE_BONUS * self.reputation.powf(1.0 / 3.0)
        } else {
            1.0
        };

        (self.average_skill() * age_modifier * special_trait_extra).max(0.0)
    }

    pub fn hire_cost(&self, team_reputation: f32) -> u32 {
        (COST_PER_VALUE * self.bare_hiring_value() * (5.0 * self.reputation - team_reputation))
            .max(1.0) as u32
    }

    pub fn release_cost(&self) -> u32 {
        0
    }

    fn apply_population_skill_modifiers(&mut self) {
        match self.info.population {
            Population::Human { region } => match region {
                Region::Italy => self.info.height = (self.info.height * 1.02).min(225.0),
                Region::Germany => self.info.height = (self.info.height * 1.05).min(225.0),
                Region::Spain => self.info.height = (self.info.height * 1.00).min(225.0),
                Region::Greece => self.info.height = (self.info.height * 0.96).min(225.0),
                Region::Nigeria => self.info.height = (self.info.height * 1.05).min(225.0),
                Region::India => self.info.height = (self.info.height * 0.95).min(225.0),
                Region::Euskadi => self.info.height = (self.info.height * 0.98).min(225.0),
                Region::Kurdistan => self.info.height = (self.info.height * 0.96).min(225.0),
                Region::Palestine => self.info.height = (self.info.height * 0.96).min(225.0),
                Region::Japan => self.info.height = (self.info.height * 0.94).min(225.0),
            },
            Population::Yardalaim => {
                self.info.weight = (self.info.weight * 1.5).min(255.0);
                self.offense.brawl = (self.offense.brawl * 1.2).bound();
                self.athletics.strength = (self.athletics.strength * 1.35).bound();
            }
            Population::Polpett => {
                self.info.height *= 0.95;
                self.mental.aggression = (self.mental.aggression * 1.35).bound();
                self.defense.steal = (self.defense.steal * 1.2).bound();
            }
            Population::Juppa => {
                self.info.height = (self.info.height * 1.09).min(225.0);
                self.offense.long_range = (self.offense.long_range * 1.23).bound();
            }
            Population::Galdari => {
                self.info.height = (self.info.height * 1.02).min(225.0);
                self.mental.charisma = (self.mental.charisma * 1.15).bound();
                self.mental.vision = (self.mental.vision * 1.35).bound();
                self.defense.steal = (self.defense.steal * 1.2).bound();
            }
            Population::Pupparoll => {
                self.athletics.quickness = (self.athletics.quickness * 0.95).bound();
                self.athletics.vertical = (self.athletics.vertical * 1.25).bound();
                self.technical.rebounds = (self.technical.rebounds * 1.25).bound();
                self.mental.aggression = (self.mental.aggression * 0.85).bound();
                self.offense.brawl = (self.offense.brawl * 1.15).bound();
            }
            Population::Octopulp => {
                self.athletics.quickness = (self.athletics.quickness * 0.95).bound();
                self.mental.vision = (self.mental.vision * 0.75).bound();
                self.defense.steal = (self.defense.steal * 1.2).bound();
                self.offense.brawl = (self.offense.brawl * 1.1).bound();
                self.offense.close_range = (self.offense.close_range * 1.35).bound();
                self.info.weight = (self.info.weight * 1.3).min(255.0);
            }
        }
    }

    fn apply_info_skill_modifiers(&mut self) {
        self.athletics.quickness = skill_linear_interpolation(
            self.athletics.quickness,
            self.info.weight,
            [90.0, 1.0, 130.0, 0.5],
        );
        self.athletics.vertical = skill_linear_interpolation(
            self.athletics.vertical,
            self.info.weight,
            [90.0, 1.0, 130.0, 0.5],
        );
        self.athletics.strength = skill_linear_interpolation(
            self.athletics.strength,
            self.info.weight,
            [75.0, 0.75, 135.0, 1.25],
        );
        self.athletics.stamina = skill_linear_interpolation(
            self.athletics.stamina,
            self.info.weight,
            [90.0, 1.0, 130.0, 0.8],
        );
        self.technical.rebounds = skill_linear_interpolation(
            self.technical.rebounds,
            self.info.height,
            [190.0, 0.75, 215.0, 1.25],
        );
        self.defense.block = skill_linear_interpolation(
            self.defense.block,
            self.info.height,
            [190.0, 0.75, 215.0, 1.25],
        );

        self.mental.vision = skill_linear_interpolation(
            self.mental.vision,
            self.info.relative_age(),
            [0.0, 0.75, 1.0, 1.25],
        );
        self.mental.charisma = skill_linear_interpolation(
            self.mental.charisma,
            self.info.relative_age(),
            [0.0, 0.75, 1.0, 1.25],
        );

        self.athletics.stamina = skill_linear_interpolation(
            self.athletics.stamina,
            self.info.relative_age(),
            [0.0, 1.1, 1.0, 0.75],
        );
    }

    fn skill_at_index(&self, idx: usize) -> Skill {
        match idx {
            0 => self.athletics.quickness,
            1 => self.athletics.vertical,
            2 => self.athletics.strength,
            3 => self.athletics.stamina,
            4 => self.offense.brawl,
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
            15 => self.technical.rebounds,
            16 => self.mental.vision,
            17 => self.mental.aggression,
            18 => self.mental.intuition,
            19 => self.mental.charisma,
            _ => panic!("Invalid skill index"),
        }
    }

    pub fn set_jersey(&mut self, jersey: &Jersey) {
        self.image.set_jersey(jersey, &self.info);
        self.version += 1;
    }

    pub fn compose_image(&self) -> AppResult<Gif> {
        self.image.compose(&self.info)
    }

    pub fn is_on_planet(&self) -> Option<PlanetId> {
        match self.current_location {
            PlayerLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        }
    }

    pub fn average_skill(&self) -> Skill {
        (0..20).map(|idx| self.skill_at_index(idx)).sum::<Skill>() / 20.0
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
        self.tiredness == MAX_SKILL
    }

    pub fn add_tiredness(&mut self, tiredness: f32) {
        let max_tiredness = if self.special_trait == Some(Trait::Relentless) {
            0.8 * MAX_SKILL
        } else if self.special_trait == Some(Trait::Crumiro) {
            0.85 * MAX_SKILL
        } else {
            MAX_SKILL
        };

        self.tiredness = (self.tiredness + tiredness / (1.0 + self.athletics.stamina / MAX_SKILL))
            .min(max_tiredness)
            .bound();

        if self.is_knocked_out() {
            self.morale = MIN_SKILL;
        }
    }

    pub fn add_morale(&mut self, morale: f32) {
        let min_morale = if self.special_trait == Some(Trait::Crumiro) {
            0.15 * MAX_SKILL
        } else {
            MIN_SKILL
        };

        let mod_morale = if morale >= 0.0 {
            morale
        } else {
            // If morale is a malus, the player charisma reduces the malus (up to a factor 2).
            morale / (1.0 + self.mental.charisma / MAX_SKILL)
        };

        self.morale = (self.morale + mod_morale).max(min_morale).bound();
    }

    pub fn modify_skill(&mut self, idx: usize, mut value: f32) {
        // Quickness cannot improve beyond WOODEN_LEG_MAX_QUICKNESS if player has a wooden leg
        if self.has_wooden_leg() && idx == 0 && self.athletics.quickness >= WOODEN_LEG_MAX_QUICKNESS
        {
            return;
        }
        // Vision cannot improve beyond EYE_PATCH_MAX_VISION if player has an eye patch
        if self.has_eye_patch() && idx == 16 && self.mental.vision >= EYE_PATCH_MAX_VISION {
            return;
        }
        // Charisma improves quicker if player has an eye patch
        if self.has_eye_patch() && idx == 19 && value > 0.0 {
            value *= 1.5;
        }

        // Ball handling cannot improve beyond HOOK_MAX_BALL_HANDLING if player has a hook
        if self.has_hook() && idx == 13 && self.technical.ball_handling >= HOOK_MAX_BALL_HANDLING {
            return;
        }
        // Strength improves quicker if player has a hook
        if self.has_hook() && idx == 2 && value > 0.0 {
            value *= 1.5;
        }

        let new_value = (self.skill_at_index(idx) + value).bound();
        match idx {
            0 => self.athletics.quickness = new_value,
            1 => self.athletics.vertical = new_value,
            2 => self.athletics.strength = new_value,
            3 => self.athletics.stamina = new_value,
            4 => self.offense.brawl = new_value,
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
            15 => self.technical.rebounds = new_value,
            16 => self.mental.vision = new_value,
            17 => self.mental.aggression = new_value,
            18 => self.mental.intuition = new_value,
            19 => self.mental.charisma = new_value,
            _ => unreachable!("Invalid skill index {idx}"),
        }
    }

    pub fn update_skills_training(
        &mut self,
        experience_at_position: [u32; MAX_GAME_POSITION as usize],
        training_bonus: f32,
        training_focus: Option<TrainingFocus>,
    ) {
        // potential_modifier has a value ranging from 0.0 to 2.0.
        // Players with skills below their potential improve faster, above their potential improve slower.
        let potential_modifier = if self.average_skill() > self.potential {
            (1.0 + (self.potential - self.average_skill()) / MAX_SKILL).powf(10.0)
        } else {
            1.0 + (self.potential - self.average_skill()) / MAX_SKILL
        };
        for p in 0..MAX_GAME_POSITION {
            if experience_at_position[p as usize] == 0 {
                continue;
            }

            for (idx, &w) in p.weights().iter().enumerate() {
                let training_focus_bonus = match training_focus {
                    Some(focus) => {
                        if focus.is_focus(idx) {
                            2.0
                        } else {
                            0.5
                        }
                    }
                    None => 1.0,
                };
                self.skills_training[idx] += experience_at_position[p as usize] as f32
                    * w
                    * EXPERIENCE_PER_SKILL_MULTIPLIER
                    * training_bonus
                    * training_focus_bonus
                    * potential_modifier;

                log::debug!(
                    "Experience increase: {:.3}={}x{}x{}x{}x{}x{:.2}",
                    experience_at_position[p as usize] as f32
                        * w
                        * EXPERIENCE_PER_SKILL_MULTIPLIER
                        * training_bonus
                        * training_focus_bonus
                        * potential_modifier,
                    experience_at_position[p as usize] as f32,
                    w,
                    EXPERIENCE_PER_SKILL_MULTIPLIER,
                    training_bonus,
                    training_focus_bonus,
                    potential_modifier
                );

                // Cap to maximum skill increase per day.
                self.skills_training[idx] =
                    self.skills_training[idx].min(MAX_SKILL_INCREASE_PER_LONG_TICK);
            }
        }

        log::debug!("Total Experience increase: {:#?}", self.skills_training);
    }

    pub fn tiredness_weighted_rating(&self) -> f32 {
        if self.is_knocked_out() {
            return 0.0;
        }
        self.average_skill() * (MAX_SKILL - self.tiredness / 2.0)
    }
}

impl Rated for Player {
    fn rating(&self) -> Skill {
        self.average_skill()
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

impl Default for InfoStats {
    fn default() -> Self {
        let population = Population::default();
        Self {
            first_name: "Defaulto".to_string(),
            last_name: "Faultonio".to_string(),
            crew_role: CrewRole::default(),
            home_planet_id: PlanetId::default(),
            population,
            age: population.min_age(),
            pronouns: Pronoun::default(),
            height: 183.2,
            weight: 73.0,
        }
    }
}

impl InfoStats {
    pub fn short_name(&self) -> String {
        format!(
            "{}.{}",
            self.first_name.chars().next().unwrap_or_default(),
            self.last_name
        )
    }

    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    pub fn relative_age(&self) -> f32 {
        self.population.relative_age(self.age)
    }
    pub fn randomize_for_position(&mut self, position: GamePosition, rng: &mut ChaCha8Rng) {
        let p_data = PLAYER_DATA
            .get(&self.population)
            .unwrap_or_else(|| panic!("Player data should exist for {}", self.population));
        let pronouns =
            if self.population == Population::Polpett || self.population == Population::Octopulp {
                Pronoun::They
            } else {
                Pronoun::random(rng)
            };
        self.first_name = match pronouns {
            Pronoun::He => p_data
                .first_names_he
                .choose(rng)
                .expect("No available name")
                .to_string(),
            Pronoun::She => p_data
                .first_names_she
                .choose(rng)
                .expect("No available name")
                .to_string(),
            Pronoun::They => match rng.random_bool(0.5) {
                true => p_data
                    .first_names_he
                    .choose(rng)
                    .expect("No available name")
                    .to_string(),
                false => p_data
                    .first_names_she
                    .choose(rng)
                    .expect("No available name")
                    .to_string(),
            },
        };
        self.last_name = p_data
            .last_names
            .choose(rng)
            .expect("No available name")
            .to_string();
        self.age = self.population.min_age()
            + rng.random_range(0.0..0.55) * (self.population.max_age() - self.population.min_age());
        self.height = Normal::new(192.0 + 3.5 * position as f32, 5.0)
            .unwrap()
            .sample(rng);
        let bmi = rng.random_range(12..22) as f32 + self.height / 20.0;
        self.weight = bmi * self.height * self.height / 10000.0;
    }
}

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Display)]
#[repr(u8)]
pub enum Trait {
    Killer,
    Relentless,
    Showpirate,
    Spugna,
    Crumiro,
}

impl Trait {
    pub fn description(&self, player: &Player) -> String {
        match self {
            Trait::Killer => format!(
                "Better at brawling during games based on reputation (+{}), +25% team bonus to Weapons.",
                player.reputation.value()
            ),
            Trait::Relentless => "Cannot get exhausted, +25% team bonus to Recovery.".to_string(),
            Trait::Showpirate => {
                format!(
                    "Increase games attendance based on reputation (+{}%), more likely to dunk!",
                    player.reputation.value()
                )
            }
            Trait::Spugna => "Immediately maximizes morale when drinking. It is said that a drunk pilot could bring you somewhere unexpected...".to_string(),
            Trait::Crumiro => "Legendary trait of the emperor's crew members".to_string(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        app::App,
        core::skill::Rated,
        types::{AppResult, HashMapWithResult},
    };
    use itertools::Itertools;
    use serde::{Deserialize, Serialize};

    #[test]
    fn test_bare_value() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        let player_id = world
            .players
            .values()
            .next()
            .expect("There should be at least one player")
            .id;

        let player = world.players.get_mut_or_err(&player_id)?;
        player.info.age = player.info.population.min_age();

        for _ in 0..20 {
            println!(
                "Relative age {:02} - Overall {:02} {} - Bare value {:02}",
                player.info.relative_age(),
                player.average_skill(),
                player.average_skill().stars(),
                player.bare_hiring_value()
            );
            player.info.age += 0.025 * player.info.population.max_age();
        }

        Ok(())
    }

    #[ignore]
    #[test]
    fn test_players_generation() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;

        let players = world
            .players
            .values()
            .sorted_by(|a, b| a.average_skill().partial_cmp(&b.average_skill()).unwrap())
            .collect_vec();

        let skills = players.iter().map(|p| p.average_skill()).collect_vec();
        let potentials = players.iter().map(|p| p.potential).collect_vec();

        #[derive(Serialize, Deserialize)]
        struct GenerationData {
            skills: Vec<f32>,
            potentials: Vec<f32>,
        }

        let data = GenerationData { skills, potentials };

        std::fs::write(
            "./pytests/player_generation.json",
            serde_json::to_vec(&data)?,
        )?;
        Ok(())
    }
}
