use super::{action::Action, constants::MIN_TIREDNESS_FOR_ROLL_DECLINE, tactic::Tactic};
use crate::{
    image::game::PitchImage,
    types::{AppResult, GameId, PlayerId, PlayerMap, TeamId, TeamMap},
    world::{
        constants::MAX_PLAYERS_PER_GAME,
        player::{InfoStats, Player},
        position::{Position, MAX_POSITION},
        skill::{Athletics, Defense, Mental, Offense, Technical, MAX_SKILL, MIN_SKILL},
        team::Team,
        types::TrainingFocus,
        utils::is_default,
    },
};
use itertools::Itertools;
use libp2p::PeerId;
use once_cell::sync::Lazy;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Not};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameStats {
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub games: [u16; 3],
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub position: Option<Position>,
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
        if let Some(shot) = stats.last_action_shot {
            self.shots.push(shot);
            assert!(self.shots.len() > 0);
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
    pub peer_id: Option<PeerId>,
    pub reputation: f32,
    pub version: u64,
    pub name: String,
    pub initial_positions: Vec<PlayerId>,
    // this is necessary for NetworkGame and in general to be able to simulate a game from the start
    // because the player tiredness is updated during the game.
    // The order is the same as initial_positions
    pub initial_tiredness: Vec<f32>,
    pub initial_morale: Vec<f32>,
    pub players: PlayerMap,
    pub stats: GameStatsMap,
    pub tactic: Tactic,
    pub training_focus: Option<TrainingFocus>,
    pub momentum: u8,
}

impl<'game> TeamInGame {
    fn new(team: &Team, players: PlayerMap) -> Self {
        let mut stats = HashMap::new();

        for (idx, &player_id) in players.keys().enumerate() {
            let mut player_stats = GameStats::default();
            if (idx as Position) < MAX_POSITION {
                player_stats.position = Some(idx as Position);
            }
            stats.insert(player_id, player_stats.clone());
        }

        let initial_tiredness = players
            .keys()
            .map(|id| players.get(id).unwrap().tiredness)
            .collect();
        let initial_morale = players
            .keys()
            .map(|id| players.get(id).unwrap().morale)
            .collect();
        Self {
            team_id: team.id,
            peer_id: team.peer_id,
            reputation: team.reputation,
            name: team.name.clone(),
            initial_positions: players.keys().map(|id| id.clone()).collect_vec(),
            initial_tiredness,
            initial_morale,
            version: team.version,
            players,
            stats,
            tactic: team.game_tactic,
            training_focus: team.training_focus,
            ..Default::default()
        }
    }

    pub fn from_team_id(team_id: &TeamId, teams: &TeamMap, players: &PlayerMap) -> Option<Self> {
        let team = teams.get(team_id)?;
        let mut team_players = PlayerMap::new();
        for &player_id in team.player_ids.iter().take(MAX_PLAYERS_PER_GAME) {
            let player = players.get(&player_id)?;
            team_players.insert(player_id, player.clone());
        }

        Some(TeamInGame::new(team, team_players))
    }

    pub fn pick_action(&self, rng: &mut ChaCha8Rng) -> AppResult<Action> {
        self.tactic.pick_action(rng)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTeamInGame {
    pub team_id: TeamId,
    pub version: u64,
    pub name: String,
    pub initial_positions: Vec<PlayerId>,
    pub players: PersistedPlayerMap,
    pub stats: GameStatsMap,
    pub tactic: Tactic,
}

impl PersistedTeamInGame {
    pub fn from_team_in_game(team_in_game: &TeamInGame) -> Self {
        let mut players = HashMap::new();
        for (player_id, player) in team_in_game.players.iter() {
            players.insert(player_id.clone(), PersistedPlayer::from_player(player));
        }
        Self {
            team_id: team_in_game.team_id,
            version: team_in_game.version,
            name: team_in_game.name.clone(),
            initial_positions: team_in_game.initial_positions.clone(),
            players,
            stats: team_in_game.stats.clone(),
            tactic: team_in_game.tactic,
        }
    }
}

type PersistedPlayerMap = HashMap<PlayerId, PersistedPlayer>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedPlayer {
    pub id: PlayerId,
    pub version: u64,
    pub info: InfoStats,
    pub athletics: Athletics,
    pub offense: Offense,
    pub technical: Technical,
    pub defense: Defense,
    pub mental: Mental,
}

impl PersistedPlayer {
    pub fn from_player(player: &Player) -> Self {
        Self {
            id: player.id,
            version: player.version,
            info: player.info.clone(),
            athletics: player.athletics,
            offense: player.offense,
            technical: player.technical,
            defense: player.defense,
            mental: player.mental,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq)]
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

pub static HOME_CLOSE_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchImage::HomeCloseShotMask));
pub static AWAY_CLOSE_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchImage::AwayCloseShotMask));
pub static HOME_MEDIUM_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchImage::HomeMediumShotMask));
pub static AWAY_MEDIUM_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchImage::AwayMediumShotMask));
pub static HOME_LONG_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchImage::HomeLongShotMask));
pub static AWAY_LONG_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchImage::AwayLongShotMask));
pub static HOME_IMPOSSIBLE_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchImage::HomeImpossibleShotMask));
pub static AWAY_IMPOSSIBLE_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchImage::AwayImpossibleShotMask));

fn get_shot_positions(mask: PitchImage) -> Vec<(u8, u8)> {
    let img = mask.image().unwrap();
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
    fn min_roll(&self) -> u8;
    fn max_roll(&self) -> u8;
    fn roll(&self, rng: &mut ChaCha8Rng) -> u8;
}

impl EnginePlayer for Player {
    fn min_roll(&self) -> u8 {
        (self.morale / 2.5) as u8
    }

    fn max_roll(&self) -> u8 {
        if self.tiredness == MAX_SKILL {
            return 0;
        }

        if self.tiredness <= MIN_TIREDNESS_FOR_ROLL_DECLINE {
            return 2 * MAX_SKILL as u8;
        }

        2 * (MAX_SKILL - (self.tiredness - MIN_TIREDNESS_FOR_ROLL_DECLINE)) as u8
    }

    fn roll(&self, rng: &mut ChaCha8Rng) -> u8 {
        rng.gen_range(MIN_SKILL as u8..=2 * MAX_SKILL as u8)
            .max(self.min_roll())
            .min(self.max_roll())
    }
}

#[cfg(test)]
#[test]
fn test_roll() {
    use rand::SeedableRng;

    use crate::{types::PlanetId, world::types::Population};

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
    let population = Population::default();
    let mut player = Player::random(
        rng,
        PlayerId::new_v4(),
        None,
        population,
        &PlanetId::default(),
        0.0,
    );

    print_player_rolls(&player, rng);

    player.tiredness = MAX_SKILL;
    print_player_rolls(&player, rng);

    player.morale = MIN_SKILL;
    print_player_rolls(&player, rng);

    player.tiredness = MIN_SKILL;
    player.morale = MIN_SKILL;
    print_player_rolls(&player, rng);
}
