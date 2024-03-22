use super::{action::Action, tactic::Tactic};
use crate::{
    image::pitch::PitchStyle,
    types::{AppResult, GameId, PlayerId, PlayerMap, TeamId, TeamMap},
    world::{
        player::{InfoStats, Player},
        position::{Position, MAX_POSITION},
        skill::{Athleticism, Defense, Mental, Offense, Technical},
        team::Team,
    },
};
use libp2p::PeerId;
use once_cell::sync::Lazy;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Not};

fn is_default<T: Default + PartialOrd>(v: &T) -> bool {
    *v == T::default()
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameStats {
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub position: Option<Position>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub seconds_played: u16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub fouls: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub technical_fouls: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub points: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub attempted_ft: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub made_ft: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub attempted_2pt: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub made_2pt: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub attempted_3pt: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub made_3pt: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub offensive_rebounds: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub defensive_rebounds: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub assists: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub steals: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub blocks: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub turnovers: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub plus_minus: i16,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub morale: u8,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub extra_tiredness: f32,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub shot_positions: Vec<(u8, u8, bool)>, //x, y, is_made
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub experience_at_position: [u16; 5],
}

impl GameStats {
    pub fn update(&mut self, stats: &GameStats) {
        //don't update is_playing and position because otherwise we would have a lot of non-default gamestats to write.
        // We instead update them by hand after the Substitution action.
        self.seconds_played += stats.seconds_played;
        self.fouls += stats.fouls;
        self.technical_fouls += stats.technical_fouls;
        self.points += stats.points;
        self.attempted_ft += stats.attempted_ft;
        self.made_ft += stats.made_ft;
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
        self.morale += stats.morale;
        self.shot_positions
            .append(&mut stats.shot_positions.clone());
        for (idx, exp) in stats.experience_at_position.iter().enumerate() {
            self.experience_at_position[idx] += exp;
        }
    }

    pub fn is_playing(&self) -> bool {
        self.position.is_some()
    }

    // pub fn is_knocked_out(&self) -> bool {
    //     self.tiredness == MAX_TIREDNESS
    // }

    // pub fn add_tiredness(&mut self, tiredness: f32, stamina: f32) {
    //     self.tiredness = (self.tiredness + tiredness / (1.0 + stamina / 20.0)).min(MAX_TIREDNESS);
    // }
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
    pub players: PlayerMap,
    pub stats: GameStatsMap,
    pub tactic: Tactic,
    pub momentum: u8,
}

impl<'game> TeamInGame {
    pub fn new(team: &Team, players: PlayerMap) -> Self {
        let mut stats = HashMap::new();

        for (idx, player_id) in team.player_ids.iter().enumerate() {
            let mut player_stats = GameStats::default();
            if (idx as Position) < MAX_POSITION {
                player_stats.position = Some(idx as Position);
            }
            stats.insert(player_id.clone(), player_stats.clone());
        }

        let initial_tiredness = team
            .player_ids
            .iter()
            .map(|id| players.get(id).unwrap().tiredness)
            .collect();
        Self {
            team_id: team.id,
            peer_id: team.peer_id,
            reputation: team.reputation,
            name: team.name.clone(),
            initial_positions: team.player_ids.clone(),
            initial_tiredness,
            version: team.version,
            players,
            stats,
            tactic: team.game_tactic,
            ..Default::default()
        }
    }

    pub fn from_team_id(team_id: TeamId, teams: &TeamMap, players: &PlayerMap) -> Option<Self> {
        let team = teams.get(&team_id)?;
        let mut team_players = PlayerMap::new();
        for player_id in team.player_ids.iter() {
            let player = players.get(player_id)?;
            team_players.insert(player_id.clone(), player.clone());
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
    pub athleticism: Athleticism,
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
            athleticism: player.athleticism,
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
    Lazy::new(|| get_shot_positions(PitchStyle::HomeCloseShotMask));
pub static AWAY_CLOSE_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchStyle::AwayCloseShotMask));
pub static HOME_MEDIUM_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchStyle::HomeMediumShotMask));
pub static AWAY_MEDIUM_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchStyle::AwayMediumShotMask));
pub static HOME_LONG_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchStyle::HomeLongShotMask));
pub static AWAY_LONG_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchStyle::AwayLongShotMask));
pub static HOME_IMPOSSIBLE_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchStyle::HomeImpossibleShotMask));
pub static AWAY_IMPOSSIBLE_SHOT_POSITIONS: Lazy<Vec<(u8, u8)>> =
    Lazy::new(|| get_shot_positions(PitchStyle::AwayImpossibleShotMask));

fn get_shot_positions(mask: PitchStyle) -> Vec<(u8, u8)> {
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
    stats.fouls = 2;
    stats.technical_fouls = 3;
    stats.points = 4;
    stats.attempted_ft = 5;
    stats.made_ft = 6;
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
    stats.morale = 18;
    stats.extra_tiredness = 19.0;
    stats.shot_positions = vec![(1, 2, true), (3, 4, false)];
    stats.experience_at_position = [1, 2, 3, 4, 5];

    let serialized = serde_json::to_string(&stats).unwrap();
    let deserialized: GameStats = serde_json::from_str(&serialized).unwrap();
    assert_eq!(stats, deserialized);
}
