use super::{
    constants::{COST_PER_VALUE, EXPERIENCE_PER_SKILL_MULTIPLIER, SPECIAL_TRAIT_VALUE_BONUS},
    jersey::Jersey,
    planet::Planet,
    position::{GamePosition, MAX_POSITION},
    resources::Resource,
    role::CrewRole,
    skill::{GameSkill, Skill, MAX_SKILL, MIN_SKILL},
    types::{PlayerLocation, Pronoun, Region, TrainingFocus},
    utils::PLAYER_DATA,
    world::World,
};
use crate::{
    game_engine::constants::MIN_TIREDNESS_FOR_ROLL_DECLINE,
    image::{player::PlayerImage, types::Gif},
    types::{AppResult, PlanetId, PlayerId, StorableResourceMap, TeamId},
    world::{
        constants::*,
        position::Position,
        skill::{Athletics, Defense, Mental, Offense, Rated, Technical},
        types::Population,
        utils::skill_linear_interpolation,
    },
};
use anyhow::anyhow;
use libp2p::PeerId;
use rand::{seq::SliceRandom, Rng};
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal};
use serde::{de::Visitor, ser::SerializeStruct, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
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
    pub tiredness: f32,
    pub morale: f32,
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

        const FIELDS: &'static [&'static str] = &[
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
    pub fn current_skill_array(&self) -> [Skill; 20] {
        (0..20)
            .map(|idx| self.skill_at_index(idx))
            .collect::<Vec<Skill>>()
            .try_into()
            .expect("There should be 20 skills")
    }

    pub fn can_drink(&self, world: &World) -> AppResult<()> {
        if self.team.is_none() {
            return Err(anyhow!("Player has no team, so no rum to drink"));
        }

        let team = world.get_team_or_err(&self.team.expect("Player should have team"))?;

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

    pub fn bare_value(&self) -> f32 {
        // Age modifier decrease linearly from (0,1.5) to (PEAK_PERFORMANCE_RELATIVE_AGE, 1.0),
        // then decreases linearly from (PEAK_PERFORMANCE_RELATIVE_AGE, 1.0) to (1.0, 0.5).
        let age_modifier = if PEAK_PERFORMANCE_RELATIVE_AGE >= self.info.relative_age() {
            -self.info.relative_age() / (2.0 * PEAK_PERFORMANCE_RELATIVE_AGE) + 1.5
        } else {
            (self.info.relative_age() + PEAK_PERFORMANCE_RELATIVE_AGE - 2.0)
                / (2.0 * PEAK_PERFORMANCE_RELATIVE_AGE - 2.0)
        };

        let special_trait_extra = if self.special_trait.is_some() {
            SPECIAL_TRAIT_VALUE_BONUS * self.reputation.powf(1.0 / 3.0)
        } else {
            1.0
        };

        (self.average_skill() * age_modifier * special_trait_extra).max(0.0)
    }

    pub fn hire_cost(&self, team_reputation: f32) -> u32 {
        (COST_PER_VALUE * self.bare_value() * (5.0 * self.reputation - team_reputation)).max(1.0)
            as u32
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

        // Base level modifier increases linearly from (0,0) to (PEAK_PERFORMANCE_RELATIVE_AGE, 1.0),
        // then decreases linearly from (PEAK_PERFORMANCE_RELATIVE_AGE, 1.0) to (1.0, 0.0).
        let base_level_modifier = if PEAK_PERFORMANCE_RELATIVE_AGE >= info.relative_age() {
            info.relative_age() / PEAK_PERFORMANCE_RELATIVE_AGE
        } else {
            (info.relative_age() - 1.0) / (PEAK_PERFORMANCE_RELATIVE_AGE - 1.0)
        };
        base_level *= base_level_modifier;

        let athletics = Athletics::for_position(position.unwrap(), rng, base_level);
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
            potential: 0.0,
            athletics,
            offense,
            technical,
            defense,
            mental,
            current_location: PlayerLocation::OnPlanet {
                planet_id: home_planet.id,
            },
            image,
            skills_training: [Skill::default(); 20],
            previous_skills: [Skill::default(); 20],
            tiredness: 0.0,
            morale: MAX_MORALE,
        };

        player.apply_info_modifiers();
        player.apply_skill_modifiers();

        if athletics.quickness < WOODEN_LEG_MAX_QUICKNESS {
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

        if athletics.strength > 15.0 && rng.gen_bool(TRAIT_PROBABILITY) {
            player.special_trait = Some(Trait::Killer);
        } else if mental.charisma > 15.0 && rng.gen_bool(TRAIT_PROBABILITY) {
            player.special_trait = Some(Trait::Showpirate);
        } else if mental.intuition > 10.0 && rng.gen_bool(TRAIT_PROBABILITY) {
            player.special_trait = Some(Trait::Spugna);
        } else if athletics.stamina > 15.0 && rng.gen_bool(TRAIT_PROBABILITY) {
            player.special_trait = Some(Trait::Relentless);
        }

        player.previous_skills = player.current_skill_array();

        let normal = Normal::new(0.0, 5.75).expect("Should create valid normal distribution");
        let extra_potential = (normal.sample(rng) as f32).abs();
        player.potential = (player.average_skill() + extra_potential).bound();
        player.reputation =
            (player.average_skill() as f32 / 5.0 + player.info.relative_age() * 5.0).bound();

        player
    }

    fn apply_skill_modifiers(&mut self) {
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
                self.info.height = self.info.height * 0.95;
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
                self.mental.vision = (self.mental.vision * 1.5).bound();
                self.defense.steal = (self.defense.steal * 1.2).bound();
            }
            Population::Pupparoll => {
                self.athletics.quickness = (self.athletics.quickness * 0.95).bound();
                self.athletics.vertical = (self.athletics.vertical * 1.25).bound();
                self.technical.rebounds = (self.technical.rebounds * 1.25).bound();
                self.mental.aggression = (self.mental.aggression * 0.85).bound();
                self.offense.brawl = (self.offense.brawl * 1.25).bound();
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

    pub fn is_on_planet(&self) -> Option<PlanetId> {
        match self.current_location {
            PlayerLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        }
    }

    fn apply_info_modifiers(&mut self) {
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
            [75.0, 0.5, 135.0, 1.3],
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

        self.mental.vision =
            skill_linear_interpolation(self.mental.vision, self.info.age, [16.0, 0.5, 38.0, 1.75]);
        self.mental.charisma = skill_linear_interpolation(
            self.mental.charisma,
            self.info.age,
            [16.0, 0.75, 38.0, 1.25],
        );

        self.athletics.stamina = skill_linear_interpolation(
            self.athletics.stamina,
            self.info.age,
            [16.0, 1.3, 44.0, 0.7],
        );
    }

    pub fn set_jersey(&mut self, jersey: &Jersey) {
        self.image.set_jersey(jersey, &self.info);
        self.version += 1;
    }

    pub fn compose_image(&self) -> AppResult<Gif> {
        self.image.compose(&self.info)
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
        self.tiredness == MAX_TIREDNESS
    }

    pub fn add_tiredness(&mut self, tiredness: f32) {
        let max_tiredness = if self.special_trait == Some(Trait::Relentless) {
            MAX_TIREDNESS - 1.0
        } else {
            MAX_TIREDNESS
        };
        self.tiredness = (self.tiredness
            + tiredness / (1.0 + self.athletics.stamina / MAX_TIREDNESS))
            .min(max_tiredness)
            .bound();

        if self.is_knocked_out() {
            self.morale = 0.0;
        }
    }

    pub fn add_morale(&mut self, morale: f32) {
        self.morale = (self.morale + morale).bound();
    }

    fn min_roll(&self) -> u8 {
        (self.morale / 2.0) as u8
    }

    fn max_roll(&self) -> u8 {
        if self.tiredness == MAX_TIREDNESS {
            return 0;
        }

        if self.tiredness <= MIN_TIREDNESS_FOR_ROLL_DECLINE {
            return 2 * MAX_SKILL as u8;
        }

        2 * (MAX_TIREDNESS - (self.tiredness - MIN_TIREDNESS_FOR_ROLL_DECLINE)) as u8
    }

    pub fn roll(&self, rng: &mut ChaCha8Rng) -> u8 {
        rng.gen_range(MIN_SKILL as u8..=2 * MAX_SKILL as u8)
            .max(self.min_roll())
            .min(self.max_roll())
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
            _ => panic!("Invalid skill index {}", idx),
        }
    }

    pub fn update_skills_training(
        &mut self,
        experience_at_position: [u16; MAX_POSITION as usize],
        training_bonus: f32,
        training_focus: Option<TrainingFocus>,
    ) {
        // potential_modifier has a value ranging from 0.0 to 2.0.
        // Players with skills below their potential improve faster, above their potential improve slower.
        let potential_modifier = 1.0 + (self.potential - self.average_skill()) / 20.0;
        for p in 0..MAX_POSITION {
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

                log::info!(
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

        log::info!("Total Experience increase: {:#?}", self.skills_training);
    }

    pub fn tiredness_weighted_rating_at_position(&self, position: Position) -> f32 {
        if self.is_knocked_out() {
            return 0.0;
        }
        position.player_rating(self.current_skill_array()) * (MAX_TIREDNESS - self.tiredness / 2.0)
    }
}

impl Rated for Player {
    fn rating(&self) -> u8 {
        self.average_skill().value()
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
    pub fn shortened_name(&self) -> String {
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
    pub fn for_position(
        position: Option<Position>,
        population: Option<Population>,
        rng: &mut ChaCha8Rng,
        home_planet: &Planet,
    ) -> Self {
        let population = match population {
            Some(p) => p,
            None => home_planet.random_population(rng).unwrap_or_default(),
        };
        let p_data = PLAYER_DATA.get(&population).unwrap();
        let pronouns = if population == Population::Polpett || population == Population::Octopulp {
            Pronoun::They
        } else {
            Pronoun::random(rng)
        };
        let first_name = match pronouns {
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
            Pronoun::They => match rng.gen_range(0..2) {
                0 => p_data
                    .first_names_he
                    .choose(rng)
                    .expect("No available name")
                    .to_string(),
                _ => p_data
                    .first_names_she
                    .choose(rng)
                    .expect("No available name")
                    .to_string(),
            },
        };
        let last_name = p_data
            .last_names
            .choose(rng)
            .expect("No available name")
            .to_string();
        let age = population.min_age()
            + rng.gen_range(0.0..0.55) * (population.max_age() - population.min_age());
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

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq, Display)]
#[repr(u8)]
pub enum Trait {
    Killer,
    Relentless,
    Showpirate,
    Spugna,
}

impl Trait {
    pub fn description(&self, player: &Player) -> String {
        match self {
            Trait::Killer => format!(
                "Better at brawling during games. Bonus is based on reputation (+{}).",
                player.reputation.value()
            ),
            Trait::Relentless => format!("Cannot get exhausted"),
            Trait::Showpirate => {
                format!(
                    "Increase games attendance based on reputation (+{}%)",
                    player.reputation.value()
                )
            }
            Trait::Spugna => format!("Immediately maximizes morale when drinking. It is said that a drunk pilot could bring you somewhere unexpected...",),
        }
    }
}

#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    use crate::{
        app::App,
        types::PlayerId,
        world::{
            planet::Planet,
            skill::{Rated, MAX_SKILL, MIN_SKILL},
        },
    };

    use super::Player;

    #[test]
    fn test_bare_value() {
        let mut app = App::new(None, true, true, true, false, None, None, None);
        app.new_world();

        let world = &mut app.world;

        let player_id = world
            .players
            .values()
            .next()
            .expect("There should be at least one player")
            .id;

        let player = world.players.get_mut(&player_id).unwrap();
        player.info.age = player.info.population.min_age();

        for _ in 0..20 {
            println!(
                "Relative age {:02} - Overall {:02} {} - Bare value {:02}",
                player.info.relative_age(),
                player.average_skill(),
                player.average_skill().stars(),
                player.bare_value()
            );
            player.info.age += 0.025 * player.info.population.max_age();
        }
    }

    #[test]
    fn test_roll() {
        fn print_player_rolls(player: &Player, rng: &mut ChaCha8Rng) {
            let roll = player.roll(rng);
            println!(
                "Tiredness={} Morale={} => Min={:2} Max={:2} Roll={:2}",
                player.tiredness,
                player.morale,
                player.min_roll(),
                player.max_roll(),
                roll
            );
            assert!(player.max_roll() >= roll);
            if player.max_roll() >= player.min_roll() {
                assert!(player.min_roll() <= roll);
            }
        }
        let rng = &mut ChaCha8Rng::from_entropy();
        let planet = Planet::default();
        let mut player = Player::random(rng, PlayerId::new_v4(), None, &planet, 0.0);

        print_player_rolls(&player, rng);

        player.tiredness = MAX_SKILL;
        print_player_rolls(&player, rng);

        player.morale = MIN_SKILL;
        print_player_rolls(&player, rng);

        player.tiredness = MIN_SKILL;
        player.morale = MIN_SKILL;
        print_player_rolls(&player, rng);
    }
}
