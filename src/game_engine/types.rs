use super::{action::Action, constants::MIN_TIREDNESS_FOR_ROLL_DECLINE, tactic::Tactic};
use crate::{
    core::{
        constants::MAX_PLAYERS_PER_GAME,
        player::Player,
        position::{GamePosition, NUM_GAME_POSITIONS},
        resources::Resource,
        skill::{MAX_SKILL, MIN_SKILL},
        team::Team,
        utils::is_default,
        GameRating, GameSkill, Rated, Skill,
    },
    game_engine::constants::{FITNESS_ROLL_MALUS, NUMBER_OF_ROLLS},
    image::game::PitchImage,
    types::{AppResult, GameId, PlayerId, PlayerMap, StorableResourceMap, TeamId, TeamMap},
};
use anyhow::anyhow;
use itertools::Itertools;

use libp2p::PeerId;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::sync::LazyLock;
use std::{collections::HashMap, ops::Not};
use strum::Display;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameStats {
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub games: [u16; 3],
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub position: Option<GamePosition>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub seconds_played: u32,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub points: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub brawls: [u16; 3], // brawls as wins/losses/draws
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub attempted_2pt: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub made_2pt: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub attempted_3pt: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub made_3pt: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub offensive_rebounds: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub defensive_rebounds: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub assists: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub steals: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub blocks: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub turnovers: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub rum_drunk: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub plus_minus: i32,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub extra_morale: f32,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub extra_tiredness: f32,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    // Contains all the shots made by the player as a tuple (x, y, is_made)
    pub shots: Vec<(u8, u8, bool)>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    // Indicates whether the player shot in the last action
    pub last_action_shot: Option<(u8, u8, bool)>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub experience_at_position: [u32; 5],
}

impl GameStats {
    pub fn update(&mut self, stats: &GameStats) {
        //don't update is_playing and position because otherwise we would have a lot of non-default gamestats to write.
        // We instead update them by hand after the Substitution action.
        self.seconds_played += stats.seconds_played;
        self.points += stats.points;
        for i in 0..self.brawls.len() {
            self.brawls[i] += stats.brawls[i];
        }
        self.attempted_2pt += stats.attempted_2pt;
        self.made_2pt += stats.made_2pt;
        self.attempted_3pt += stats.attempted_3pt;
        self.made_3pt += stats.made_3pt;
        self.offensive_rebounds += stats.offensive_rebounds;
        self.defensive_rebounds += stats.defensive_rebounds;
        self.assists += stats.assists;
        self.steals += stats.steals;
        self.blocks += stats.blocks;
        self.turnovers += stats.turnovers;
        self.rum_drunk += stats.rum_drunk;
        if let Some(shot) = stats.last_action_shot {
            self.shots.push(shot);
            assert!(!self.shots.is_empty());
        }
        self.last_action_shot = stats.last_action_shot;
        for (idx, exp) in stats.experience_at_position.iter().enumerate() {
            // This loop is used only for historical stats update, as we don't give extra experience during actions at the moment.
            self.experience_at_position[idx] += exp;
        }
    }

    pub fn is_playing(&self) -> bool {
        self.position.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TeamInGame {
    pub team_id: TeamId,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub peer_id: Option<PeerId>,
    pub reputation: Skill,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub version: u64,
    pub name: String,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub initial_positions: Vec<PlayerId>,
    // This is necessary for NetworkGame and in general to be able to simulate a game from the start
    // because the player tiredness is updated during the game.
    // The order is the same as initial_positions
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub initial_tiredness: Vec<Skill>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub initial_morale: Vec<Skill>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub initial_drunkenness: Vec<Skill>,
    // Rum brought to the game, see Team::enter_game.
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub initial_rum: u32,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub rum: u32,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub players: PlayerMap,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub stats: GameStatsMap,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub tactic: Tactic,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub substitution_tendency: SubstitutionTendency,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub game_position_fluidity: GamePositionFluidity,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub in_game_drinking: InGameDrinking,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub score_run: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub network_game_rating: GameRating,
}

impl TeamInGame {
    pub fn new(team: &Team, players: PlayerMap) -> Self {
        let mut stats = HashMap::new();

        for (idx, &player_id) in players.keys().enumerate() {
            let mut player_stats = GameStats::default();
            if (idx as GamePosition) < NUM_GAME_POSITIONS {
                player_stats.position = Some(idx as GamePosition);
            }
            stats.insert(player_id, player_stats.clone());
        }

        let initial_tiredness = players.values().map(|p| p.tiredness).collect();
        let initial_morale = players.values().map(|p| p.morale).collect();
        let initial_drunkenness = players.values().map(|p| p.drunkenness).collect();

        // Rum brought to the game, depending on the team in-game drinking setting.
        let initial_rum = team.resources.value(&Resource::RUM).min(
            team.in_game_drinking.bottles_per_player() * players.len() as u32,
        );

        let network_game_rating = team.network_game_rating.clone();
        Self {
            team_id: team.id,
            peer_id: team.peer_id,
            reputation: team.reputation,
            name: team.name.clone(),
            initial_positions: players.keys().copied().collect_vec(),
            initial_tiredness,
            initial_morale,
            initial_drunkenness,
            initial_rum,
            rum: initial_rum,
            version: team.version,
            players,
            stats,
            tactic: team.game_tactic,
            network_game_rating,
            substitution_tendency: team.substitution_tendency,
            game_position_fluidity: team.game_position_fluidity,
            in_game_drinking: team.in_game_drinking,
            ..Default::default()
        }
    }

    pub fn test() -> Self {
        let team = Team {
            id: TeamId::new_v4(),
            ..Default::default()
        };

        let mut players = PlayerMap::new();
        for _ in 0..MAX_PLAYERS_PER_GAME {
            let player = Player::default().randomize(None);
            players.insert(player.id, player);
        }

        Self::new(&team, players)
    }

    // We expose this function rather than from_team because we need to get the players anyway.
    pub fn from_team_id(team_id: &TeamId, teams: &TeamMap, players: &PlayerMap) -> AppResult<Self> {
        let team = if let Some(team) = teams.get(team_id) {
            team
        } else {
            return Err(anyhow!("Could not find team {team_id}"));
        };
        let mut team_players = PlayerMap::new();
        for &player_id in team.player_ids.iter().take(MAX_PLAYERS_PER_GAME) {
            let player = if let Some(player) = players.get(&player_id) {
                player
            } else {
                return Err(anyhow!("Could not find player {player_id}"));
            };
            team_players.insert(player_id, player.clone());
        }

        Ok(TeamInGame::new(team, team_players))
    }

    // Total drunkenness of the players currently on the pitch.
    pub(crate) fn playing_drunkenness(&self) -> Skill {
        self.stats
            .iter()
            .filter(|(_, stats)| stats.is_playing())
            .filter_map(|(id, _)| self.players.get(id))
            .map(|p| p.drunkenness)
            .sum()
    }

    pub fn pick_action(&self, rng: &mut ChaCha8Rng) -> Option<Action> {
        let num_active_players = self
            .players
            .values()
            .filter(|p| !p.is_knocked_out())
            .count();

        self.tactic.pick_action(rng, num_active_players)
    }
}

impl Rated for TeamInGame {
    fn rating(&self) -> Skill {
        if self.players.is_empty() {
            return MIN_SKILL;
        }

        self.players
            .values()
            .map(|p| p.average_skill())
            .sum::<Skill>()
            / self.players.len() as Skill
    }
}

#[derive(Debug, Clone, Copy, Default, Display, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SubstitutionTendency {
    Low,
    #[default]
    Normal,
    High,
}

impl SubstitutionTendency {
    pub const fn next(&self) -> Self {
        match self {
            Self::Low => Self::Normal,
            Self::Normal => Self::High,
            Self::High => Self::Low,
        }
    }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::Low => "Tend to substitute players less frequently during games.",
            Self::Normal => "Tend to substitute players with default frequency during games.",
            Self::High => "Tend to substitute players more often during games.",
        }
    }

    pub const fn substitution_probability(&self) -> f64 {
        match self {
            Self::Low => 0.125,
            Self::Normal => 0.25,
            Self::High => 0.425,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Display, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum GamePositionFluidity {
    Low,
    #[default]
    Normal,
    High,
}

impl GamePositionFluidity {
    pub const fn next(&self) -> Self {
        match self {
            Self::Low => Self::Normal,
            Self::Normal => Self::High,
            Self::High => Self::Low,
        }
    }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::Low => "Tend to put players in their best position as much as possible.",
            Self::Normal => "Tend to put players in their best position with default frequency.",
            Self::High => "Tend to allow players to play in more positions.",
        }
    }

    pub const fn fitness_exponent(&self) -> f32 {
        match self {
            Self::Low => 1.5,
            Self::Normal => 1.0,
            Self::High => 0.6,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Display, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum InGameDrinking {
    None,
    #[default]
    Normal,
    High,
}

impl InGameDrinking {
    pub const fn next(&self) -> Self {
        match self {
            Self::None => Self::Normal,
            Self::Normal => Self::High,
            Self::High => Self::None,
        }
    }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::None => "Bring no rum to games: pirates never drink on the bench.",
            Self::Normal => "Bring 1 liter of rum per pirate to games for the occasional bench swig.",
            Self::High => "Bring 2 liters of rum per pirate to games. The bench drinks often.",
        }
    }

    // How many liters of rum per pirate are brought to a game.
    pub const fn bottles_per_player(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::Normal => 1,
            Self::High => 2,
        }
    }

    // Multiplies the morale-based probability of drinking when substituted out.
    pub const fn drink_probability_modifier(&self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Normal => 1.0,
            Self::High => 1.5,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Possession {
    #[default]
    Home,
    Away,
}

impl Not for Possession {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Home => Self::Away,
            Self::Away => Self::Home,
        }
    }
}

pub type GameStatsMap = HashMap<GameId, GameStats>;

pub static HOME_CLOSE_SHOT_POSITIONS: LazyLock<Vec<(u8, u8)>> =
    LazyLock::new(|| get_shot_positions(PitchImage::HomeCloseShotMask));
pub static AWAY_CLOSE_SHOT_POSITIONS: LazyLock<Vec<(u8, u8)>> =
    LazyLock::new(|| get_shot_positions(PitchImage::AwayCloseShotMask));
pub static HOME_MEDIUM_SHOT_POSITIONS: LazyLock<Vec<(u8, u8)>> =
    LazyLock::new(|| get_shot_positions(PitchImage::HomeMediumShotMask));
pub static AWAY_MEDIUM_SHOT_POSITIONS: LazyLock<Vec<(u8, u8)>> =
    LazyLock::new(|| get_shot_positions(PitchImage::AwayMediumShotMask));
pub static HOME_LONG_SHOT_POSITIONS: LazyLock<Vec<(u8, u8)>> =
    LazyLock::new(|| get_shot_positions(PitchImage::HomeLongShotMask));
pub static AWAY_LONG_SHOT_POSITIONS: LazyLock<Vec<(u8, u8)>> =
    LazyLock::new(|| get_shot_positions(PitchImage::AwayLongShotMask));
pub static HOME_IMPOSSIBLE_SHOT_POSITIONS: LazyLock<Vec<(u8, u8)>> =
    LazyLock::new(|| get_shot_positions(PitchImage::HomeImpossibleShotMask));
pub static AWAY_IMPOSSIBLE_SHOT_POSITIONS: LazyLock<Vec<(u8, u8)>> =
    LazyLock::new(|| get_shot_positions(PitchImage::AwayImpossibleShotMask));

fn get_shot_positions(mask: PitchImage) -> Vec<(u8, u8)> {
    let img = mask.image();
    // select the position of all pixels with positive alpha
    let mut positions = vec![];
    for x in 0..img.width() {
        for y in 0..img.height() {
            let pixel = img.get_pixel(x, y);
            if pixel[3] > 0 {
                positions.push((x as u8, y as u8));
            }
        }
    }
    positions
}

#[cfg(test)]
#[test]
// test GameStats serialization and deserialization
fn test_gamestats_serde() {
    let mut stats = GameStats::default();
    stats.seconds_played = 0;
    stats.points = 4;
    stats.brawls = [5, 5, 6];
    stats.attempted_2pt = 7;
    stats.made_2pt = 8;
    stats.attempted_3pt = 9;
    stats.made_3pt = 10;
    stats.offensive_rebounds = 0;
    stats.defensive_rebounds = 12;
    stats.assists = 13;
    stats.steals = 14;
    stats.blocks = 15;
    stats.turnovers = 0;
    stats.plus_minus = 17;
    stats.extra_morale = 18.0;
    stats.extra_tiredness = 19.0;
    stats.shots = vec![(1, 2, true), (3, 4, false)];
    stats.last_action_shot = None;
    stats.experience_at_position = [1, 2, 3, 4, 5];

    let serialized = serde_json::to_string(&stats).unwrap();
    let deserialized: GameStats = serde_json::from_str(&serialized).unwrap();
    assert_eq!(stats, deserialized);
}

pub trait EnginePlayer {
    fn min_roll(&self) -> i16;
    fn max_roll(&self) -> i16;
    fn roll(&self, rng: &mut ChaCha8Rng, position: Option<GamePosition>) -> i16;
    fn in_game_rating_at_position(
        &self,
        position: GamePosition,
        game_position_fluidity: GamePositionFluidity,
    ) -> f32;
}

impl EnginePlayer for Player {
    fn min_roll(&self) -> i16 {
        self.morale.game_value()
    }

    fn max_roll(&self) -> i16 {
        if self.tiredness == MAX_SKILL {
            return 0;
        }

        if self.tiredness <= MIN_TIREDNESS_FOR_ROLL_DECLINE {
            return MAX_SKILL as i16 * NUMBER_OF_ROLLS as i16;
        }

        const BASE: i16 = 3;
        BASE * MAX_SKILL as i16
            + (NUMBER_OF_ROLLS as i16 - BASE)
                * (MAX_SKILL - (self.tiredness - MIN_TIREDNESS_FOR_ROLL_DECLINE)) as i16
    }

    fn roll(&self, rng: &mut ChaCha8Rng, position: Option<GamePosition>) -> i16 {
        let base = rng
            .random_range(MIN_SKILL as i16..=NUMBER_OF_ROLLS as i16 * MAX_SKILL as i16)
            .max(self.min_roll())
            .min(self.max_roll());

        // Playing out of position caps performance, scaled by how poor the fit is.
        let fitness_malus = if let Some(position) = position {
            let fitness = self
                .game_position_fitness
                .get(position as usize)
                .copied()
                .unwrap_or_default()
                / MAX_SKILL;
            ((1.0 - fitness) * FITNESS_ROLL_MALUS) as i16
        } else {
            0
        };

        (base - fitness_malus).max(MIN_SKILL as i16)
    }

    fn in_game_rating_at_position(
        &self,
        position: GamePosition,
        game_position_fluidity: GamePositionFluidity,
    ) -> f32 {
        if self.is_knocked_out() {
            return 0.0;
        }
        // Follow the general rule: Roll + 2 * skills ( + tactic but it's the same for evey player in the team).
        // This factor takes into account the current tiredness
        let roll = (self.min_roll() + self.max_roll()) / 2;

        // Adjust the skill part by the position fitness, reshaped by the team
        // fluidity setting: the exponent widens (Low) or narrows (High) the gap
        // between good and bad fits.
        let fitness = self
            .game_position_fitness
            .get(position as usize)
            .copied()
            .unwrap_or_default()
            / MAX_SKILL;
        let adjusted_fitness = fitness.powf(game_position_fluidity.fitness_exponent());

        roll as f32 + 2.0 * self.position_skill_rating(position) * adjusted_fitness
    }
}

#[cfg(test)]
#[test]
fn test_roll() {
    use rand::SeedableRng;

    fn print_player_rolls(player: &Player, rng: &mut ChaCha8Rng) {
        let roll = player.roll(rng, None);
        let roll2 = player.roll(rng, None);
        println!(
            "Tiredness={:<4.1} Morale={:<4.1} => Min={:<3} Max={:<3} Roll={:<3}  AdvAtk={:<3} AdvDef={:<3}",
            player.tiredness,
            player.morale,
            player.min_roll(),
            player.max_roll(),
            roll,
            roll.max(roll2),
            roll.min(roll2)
        );

        assert!(player.max_roll() >= roll);
        if player.max_roll() >= player.min_roll() {
            assert!(player.min_roll() <= roll);
        }
    }
    let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());
    let mut player = Player::default().randomize(Some(rng));

    print_player_rolls(&player, rng);

    player.tiredness = MAX_SKILL;
    print_player_rolls(&player, rng);

    player.morale = MIN_SKILL;
    print_player_rolls(&player, rng);

    player.tiredness = MIN_TIREDNESS_FOR_ROLL_DECLINE;
    player.morale = MIN_SKILL;
    print_player_rolls(&player, rng);

    player.tiredness = MIN_TIREDNESS_FOR_ROLL_DECLINE + 2.0;
    player.morale = MIN_SKILL;
    print_player_rolls(&player, rng);

    player.tiredness = MIN_TIREDNESS_FOR_ROLL_DECLINE + 2.0;
    player.morale = MAX_SKILL / 2.0;
    print_player_rolls(&player, rng);

    for _ in 0..10 {
        player.tiredness = rng.random_range(MIN_SKILL..=MAX_SKILL);
        player.morale = rng.random_range(MIN_SKILL..=MAX_SKILL);
        print_player_rolls(&player, rng);
    }
}
