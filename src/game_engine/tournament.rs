use std::collections::HashMap;

use crate::{
    app_version,
    core::{Planet, Rated, Skill, Team, HOURS, MIN_SKILL, SECONDS},
    game_engine::{
        game::{Game, GameSummary},
        types::TeamInGame,
    },
    types::{
        AppResult, GameId, GameMap, GameSummaryMap, KartoffelId, PlanetId, PlayerMap,
        SystemTimeTick, TeamId, Tick,
    },
};
use anyhow::anyhow;
use itertools::Itertools;
use rand::{seq::SliceRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::Display;

pub type TournamentId = uuid::Uuid;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentSummary {
    pub id: TournamentId,
    tournament_type: TournamentType,
    kartoffel_id: KartoffelId,
    pub organizer_id: TeamId,
    max_participants: usize,
    pub participants: HashMap<TeamId, TeamInGame>,
    pub game_ids: Vec<GameId>,
    pub planet_id: PlanetId,
    planet_name: String,
    planet_total_population: u32,
    registrations_closing_at: Tick,
    pub ended_at: Option<Tick>,
    pub winner: Option<TeamId>,
    app_version: [usize; 3],
}

impl Rated for TournamentSummary {
    fn rating(&self) -> Skill {
        if self.participants.is_empty() {
            return MIN_SKILL;
        }

        self.participants
            .values()
            .map(|team| team.rating())
            .sum::<Skill>()
            / self.participants.len() as Skill
    }
}

impl TournamentSummary {
    pub fn from_tournament(tournament: &Tournament) -> Self {
        Self {
            id: tournament.id,
            tournament_type: tournament.tournament_type,
            kartoffel_id: tournament.kartoffel_id,
            organizer_id: tournament.organizer_id,
            max_participants: tournament.max_participants,
            participants: tournament.participants.clone(),
            game_ids: tournament.game_ids.clone(),
            planet_id: tournament.planet_id,
            planet_name: tournament.planet_name.clone(),
            planet_total_population: tournament.planet_total_population,
            registrations_closing_at: tournament.registrations_closing_at,
            ended_at: tournament.ended_at,
            winner: tournament.winner,
            app_version: tournament.app_version,
        }
    }

    pub fn starting_at(&self) -> Tick {
        self.registrations_closing_at
            + Tournament::CONFIRMATION_STATE_DURATION
            + Tournament::SYNCING_STATE_DURATION
    }

    pub fn name(&self) -> String {
        let size_name = match self.tournament_type {
            TournamentType::Brackets => match self.max_participants {
                x if x <= 8 => "cup",
                _ => "supercup",
            },
            TournamentType::RoundRobin => match self.max_participants {
                x if x <= 6 => "league",
                _ => "championship",
            },
            TournamentType::Swiss => match self.max_participants {
                x if x <= 6 => "prix",
                _ => "grand prix",
            },
        };

        format!("{} {}", self.planet_name, size_name)
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
enum TournamentType {
    #[default]
    Brackets,
    RoundRobin,
    Swiss,
}

#[derive(Debug, Display, PartialEq)]
pub enum TournamentState {
    // Teams can preregister to the tournament, no limit on number of teams.
    Registration,
    // Tournament has been canceled. At the moment, only if organizer was not playing when moving to Confirmation step.
    Canceled,
    // Teams are sent a confirmation request and are confirmed their participation to the tournament
    // on a first-time-first-serve basis, up to filling spots.
    Confirmation,
    // Tournament is sent to participating teams. This should happene fast (meaning CONFIRMATION_STATE_DURATION is short)
    // to avoid having confirmed teams not receiving the tournament.
    Syncing,
    // Games are played and yadda-yadda.
    Started,
    // Tournament is over
    Ended,
}

// Note: all clients will run the same tournament deterministically,
// but teams can be registered only with a network message sent to the organizer,
// which will respond with the updated tournament state.
// This means that clients are responsible for updating their team state
// to reflect the fact that they will be playing in the tournament.

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tournament {
    pub id: TournamentId,
    tournament_type: TournamentType,
    kartoffel_id: KartoffelId,
    pub organizer_id: TeamId,
    pub max_participants: usize,
    canceled: bool,
    pub registered_teams: HashMap<TeamId, TeamInGame>,
    pub participants: HashMap<TeamId, TeamInGame>,
    pub game_ids: Vec<GameId>,
    pending_team_for_next_game: Option<TeamId>,
    pub planet_id: PlanetId,
    pub planet_name: String,
    planet_total_population: u32,
    pub registrations_closing_at: Tick,
    game_time_interval: Tick,
    ended_at: Option<Tick>,
    pub winner: Option<TeamId>,
    app_version: [usize; 3],
}

impl Tournament {
    const CONFIRMATION_STATE_DURATION: Tick = 6 * SECONDS;
    const SYNCING_STATE_DURATION: Tick = 3 * SECONDS;
    pub fn get_rng_seed(&self, value: u64) -> [u8; 32] {
        let mut seed = [0; 32];
        seed[0..16].copy_from_slice(self.id.as_bytes());
        seed[16..24].copy_from_slice(&value.to_be_bytes());

        seed
    }

    fn get_rng(&self, value: u64) -> ChaCha8Rng {
        ChaCha8Rng::from_seed(self.get_rng_seed(value))
    }

    fn new_game(
        &self,
        rng: &mut ChaCha8Rng,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
        starting_at: Tick,
    ) -> Game {
        Game::new(
            GameId::from_u128(rng.random()),
            home_team_in_game,
            away_team_in_game,
            starting_at,
            self.planet_id,
            self.planet_total_population,
            self.planet_name.as_str(),
            Some(self.id),
        )
    }

    pub fn state(&self, timestamp: Tick) -> TournamentState {
        if self.canceled {
            return TournamentState::Canceled;
        }

        // state:              registration       | confirmation                  | syncing                  | started            | ended
        // timestamp: < registrations_closing_at  | + CONFIRMATION_STATE_DURATION | + SYNCING_STATE_DURATION | ended_at.is_none() | ended_at.is_some()

        if self.has_ended() {
            return TournamentState::Ended;
        }

        if self.has_started(timestamp) {
            return TournamentState::Started;
        }

        if self.registrations_closing_at + Self::CONFIRMATION_STATE_DURATION <= timestamp {
            return TournamentState::Syncing;
        }

        if self.registrations_closing_at <= timestamp {
            return TournamentState::Confirmation;
        }

        TournamentState::Registration
    }

    pub fn cancel(&mut self) {
        // Cancel tournament. This can happen if the organizer team is not playing when the confirmation have to be sent.
        self.canceled = true;
    }

    pub fn starting_at(&self) -> Tick {
        self.registrations_closing_at
            + Self::CONFIRMATION_STATE_DURATION
            + Self::SYNCING_STATE_DURATION
    }

    pub fn new(
        organizer: &Team,
        max_participants: usize,
        registrations_closing_at: Tick,
    ) -> AppResult<Self> {
        organizer.can_organize_tournament()?;

        let now = Tick::now();
        if registrations_closing_at <= now {
            return Err(anyhow!("Tournament is closing registrations in the past!"));
        }

        let tournament = Self {
            organizer_id: organizer.id,
            id: TournamentId::new_v4(),
            max_participants,
            registrations_closing_at,
            app_version: app_version(),
            game_time_interval: 1 * HOURS,
            ..Default::default()
        };

        Ok(tournament)
    }

    pub fn test(participants: usize, max_participants: usize) -> Self {
        let mut t = Self {
            id: TournamentId::from_u128(1),
            max_participants,
            registrations_closing_at: Tick::now() + 1 * SECONDS,
            ..Default::default()
        };

        for idx in 0..participants {
            let mut team_in_game = TeamInGame::test();
            team_in_game.team_id = TeamId::from_u128(idx as u128);
            team_in_game.name = format!("Team {idx}");
            t.registered_teams
                .insert(team_in_game.team_id, team_in_game.clone());
            t.participants.insert(team_in_game.team_id, team_in_game);
        }

        t
    }

    pub fn on_planet(mut self, planet: &Planet) -> Self {
        self.planet_id = planet.id;
        self.planet_name = planet.name.clone();
        self.planet_total_population = planet.total_population();
        self
    }

    pub fn name(&self) -> String {
        let size_name = match self.tournament_type {
            TournamentType::Brackets => match self.max_participants {
                x if x <= 8 => "cup",
                _ => "supercup",
            },
            TournamentType::RoundRobin => match self.max_participants {
                x if x <= 6 => "league",
                _ => "championship",
            },
            TournamentType::Swiss => match self.max_participants {
                x if x <= 6 => "prix",
                _ => "grand prix",
            },
        };

        format!("{} {}", self.planet_name, size_name)
    }

    pub fn register_team(
        &mut self,
        team: &Team,
        players: PlayerMap,
        timestamp: Tick,
    ) -> AppResult<()> {
        team.can_register_to_tournament(self, timestamp)?;
        let team_in_game = TeamInGame::new(team, players);
        self.registered_teams.insert(team.id, team_in_game);

        Ok(())
    }

    pub fn confirm_organizing_team(
        &mut self,
        team: &Team,
        players: PlayerMap,
        timestamp: Tick,
    ) -> AppResult<()> {
        if team.id != self.organizer_id {
            return Err(anyhow!("Only organizing team can be confirmed directly."));
        }
        team.can_confirm_tournament_registration(self, timestamp)?;
        let team_in_game = TeamInGame::new(team, players);
        self.participants.insert(team.id, team_in_game);

        Ok(())
    }

    pub fn is_team_registered(&self, team_id: &TeamId) -> bool {
        self.registered_teams.contains_key(team_id)
    }

    pub fn is_team_participating(&self, team_id: &TeamId) -> bool {
        self.participants.contains_key(team_id)
    }

    pub fn confirm_team_registration(
        &mut self,
        team: &Team,
        players: PlayerMap,
        timestamp: Tick,
    ) -> AppResult<()> {
        team.can_confirm_tournament_registration(self, timestamp)?;
        let team_in_game = TeamInGame::new(team, players);
        self.participants.insert(team.id, team_in_game);

        Ok(())
    }

    pub fn are_registrations_closed(&self, timestamp: Tick) -> bool {
        self.registrations_closing_at <= timestamp
    }

    pub fn are_registrations_open(&self, timestamp: Tick) -> bool {
        !self.are_registrations_closed(timestamp)
    }

    pub fn has_started(&self, timestamp: Tick) -> bool {
        self.starting_at() <= timestamp
    }

    pub fn has_ended(&self) -> bool {
        self.ended_at.is_some()
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled
    }

    pub fn is_initialized(&self) -> bool {
        !self.game_ids.is_empty()
    }

    pub fn initialize(&mut self) -> Vec<Game> {
        let mut new_games = vec![];
        assert!(!self.has_ended());
        assert!(self.winner.is_none());
        assert!(self.pending_team_for_next_game.is_none());

        let rng = &mut self.get_rng(self.starting_at());

        // Initialize initial games.
        // We shuffle the indecies of the participants, then pair them.
        // If the number of teams is odd, simply set pending_team_for_next_game (it's like a bye).
        let mut pairings = self
            .participants
            .values()
            .sorted_by(|a, b| a.team_id.cmp(&b.team_id))
            .collect_vec();
        pairings.shuffle(rng);

        assert!(pairings.len() == self.participants.len());

        for (idx, &team_in_game) in pairings.iter().enumerate() {
            if let Some(pending_team_id) = self.pending_team_for_next_game.take() {
                let pending_team = self
                    .participants
                    .get(&pending_team_id)
                    .expect("Team should be a participant");

                let game = self.new_game(
                    rng,
                    team_in_game.clone(),
                    pending_team.clone(),
                    self.starting_at()
                        + self.game_time_interval * ((idx + 1) / pairings.len()) as u64,
                );
                self.game_ids.push(game.id);
                new_games.push(game);
            } else {
                self.pending_team_for_next_game = Some(team_in_game.team_id);
            }
        }

        if self.participants.len().is_multiple_of(2) {
            assert!(self.pending_team_for_next_game.is_none());
        } else {
            assert!(self.pending_team_for_next_game.is_some());
        }

        return new_games;
    }

    pub fn generate_next_games(
        &mut self,
        current_tick: Tick,
        games: &GameMap,
    ) -> AppResult<Vec<Game>> {
        if !self.is_initialized() {
            return Err(anyhow!("Tournament should have been initialized."));
        }

        if !self.has_started(current_tick) {
            return Ok(vec![]);
        }

        if self.has_ended() {
            return Err(anyhow!(
                "generate_next_games should not be called for ended tournaments."
            ));
        }

        if self.participants.is_empty() {
            unreachable!(
                "Should not be possible to call generate_next_games with empty participants."
            )
        }

        let mut tournament_games = vec![];

        for game_id in self.game_ids.iter() {
            if let Some(game) = games.get(game_id) {
                tournament_games.push(game);
            }
        }

        // At this point, tournament_games can be empty only if the last game was the final,
        // in which case the pending_team_for_next_game is the tournament winner
        if tournament_games.is_empty() {
            if self.pending_team_for_next_game.is_none() {
                return Err(anyhow!(
                    "There should be a pending team if there are no available tournament games."
                ));
            }
            self.winner = self.pending_team_for_next_game;
            self.ended_at = Some(current_tick);
            return Ok(vec![]);
        }

        let rng = &mut self.get_rng(self.game_ids.len() as u64 + 1);
        let mut new_games = vec![];
        for game in tournament_games {
            // Game could have ended because we process tournaments AFTER ticking games and BEFORE removing ended games.
            if !game.has_ended() {
                continue;
            }

            let winner_team_id = if let Some(team_id) = game.winner {
                team_id
            } else {
                return Err(anyhow!("Tournament game should have a winner."));
            };
            if let Some(other_team_id) = self.pending_team_for_next_game {
                let home_team_in_game = self
                    .participants
                    .get(&winner_team_id)
                    .expect("Team should be a participant");
                let away_team_in_game = self
                    .participants
                    .get(&other_team_id)
                    .expect("Team should be a participant");

                let game = self.new_game(
                    rng,
                    home_team_in_game.clone(),
                    away_team_in_game.clone(),
                    current_tick + self.game_time_interval,
                );
                self.game_ids.push(game.id);
                new_games.push(game);

                self.pending_team_for_next_game = None;
            } else {
                self.pending_team_for_next_game = Some(winner_team_id)
            }
        }

        Ok(new_games)
    }

    pub fn active_games<'a>(&'a self, games: &'a GameMap) -> Vec<&'a Game> {
        self.game_ids
            .iter()
            .filter_map(|id| games.get(id))
            .collect::<Vec<&Game>>()
    }
    pub fn past_game_summaries<'a>(
        &'a self,
        past_games: &'a GameSummaryMap,
    ) -> Vec<&'a GameSummary> {
        self.game_ids
            .iter()
            .filter_map(|id| past_games.get(id))
            .collect::<Vec<&GameSummary>>()
    }
}

impl Rated for Tournament {
    fn rating(&self) -> Skill {
        let teams = match self.state(Tick::now()) {
            TournamentState::Registration => &self.registered_teams,
            TournamentState::Confirmation
            | TournamentState::Started
            | TournamentState::Ended
            | TournamentState::Syncing => &self.participants,
            TournamentState::Canceled => &HashMap::default(),
        };

        if teams.is_empty() {
            return MIN_SKILL;
        }

        teams.values().map(|team| team.rating()).sum::<Skill>() / teams.len() as Skill
    }
}

#[cfg(test)]
mod tests {

    use crate::core::{Player, Team, TeamLocation, TickInterval, MAX_PLAYERS_PER_GAME, SECONDS};
    use crate::game_engine::Tournament;
    use crate::types::{AppResult, GameMap, PlanetId, PlayerMap, SystemTimeTick, TeamId, Tick};
    use itertools::Itertools;
    use libp2p::PeerId;

    #[test]
    fn test_tournament_determinism() -> AppResult<()> {
        let mut tournament = Tournament::test(6, 8);
        let mut replay_tournament = tournament.clone();

        fn process_tournament(tournament: &mut Tournament) -> AppResult<()> {
            let mut games = GameMap::new();

            for game in tournament.initialize() {
                games.insert(game.id, game);
            }

            let mut current_tick = tournament.registrations_closing_at;

            while !tournament.has_ended() {
                for game in games.values_mut() {
                    if game.has_started(current_tick) {
                        game.tick(current_tick);
                    }
                }

                let new_games = tournament.generate_next_games(current_tick, &games)?;

                games.retain(|_, g| !g.has_ended());

                for game in new_games {
                    games.insert(game.id, game);
                }

                current_tick += TickInterval::SHORT;
            }
            Ok(())
        }

        process_tournament(&mut tournament)?;
        process_tournament(&mut replay_tournament)?;

        assert!(tournament == replay_tournament);

        Ok(())
    }

    #[test]
    fn test_tournament_game_schedule() -> AppResult<()> {
        let mut tournament = Tournament::test(7, 8);
        tournament.registrations_closing_at = 0;
        println!(
            "{:#?}",
            tournament.get_rng_seed(tournament.starting_at() as u64)
        );
        let mut games = GameMap::new();
        let mut past_games = GameMap::new();

        for game in tournament.initialize() {
            games.insert(game.id, game);
        }

        let mut current_tick = 0;
        while !tournament.has_ended() {
            for game in games.values_mut() {
                game.tick(current_tick);
            }

            let new_games = tournament.generate_next_games(current_tick, &games)?;

            for game in games.values().filter(|g| g.has_ended()) {
                past_games.insert(game.id, game.clone());
            }

            games.retain(|_, g| !g.has_ended());

            for game in new_games {
                games.insert(game.id, game);
            }

            current_tick += TickInterval::SHORT;
        }

        assert!(tournament.winner.is_some());
        assert!(games.is_empty());
        assert!(past_games.len() == tournament.participants.len() - 1);
        println!("{:#?}", tournament.winner,);

        for game in past_games
            .values()
            .sorted_by(|a, b| a.starting_at.cmp(&b.starting_at))
        {
            println!(
                "{} {} {}-{} {} --> {}",
                game.starting_at.formatted_as_time(),
                game.home_team_in_game.name,
                game.get_score().0,
                game.get_score().1,
                game.away_team_in_game.name,
                if matches!(game.winner,  Some(id) if id == game.home_team_in_game.team_id) {
                    game.home_team_in_game.name.as_str()
                } else {
                    game.away_team_in_game.name.as_str()
                }
            )
        }

        Ok(())
    }

    #[test]
    fn test_tournament_error_registrations_closed() -> AppResult<()> {
        const MAX_PARTICIPANTS: usize = 0;

        let mut organizer = Team::random(None);
        let planet_id = PlanetId::default();
        organizer.space_cove = crate::core::SpaceCoveState::Ready { planet_id };
        organizer.current_location = TeamLocation::OnPlanet { planet_id };

        let registrations_closing_at = Tick::now() + 1 * SECONDS;

        let mut tournament =
            Tournament::new(&organizer, MAX_PARTICIPANTS, registrations_closing_at)?;

        let team = Team {
            id: TeamId::new_v4(),
            peer_id: Some(PeerId::random()),
            current_location: TeamLocation::OnPlanet { planet_id },
            ..Default::default()
        };

        let mut players = PlayerMap::new();
        for _ in 0..MAX_PLAYERS_PER_GAME {
            let player = Player::default().randomize(None);
            players.insert(player.id, player);
        }

        assert!(matches!(
            tournament.register_team(&team, players, registrations_closing_at + 1),
            Err(e) if e.to_string() == "Tournament registrations are closed."
        ));
        Ok(())
    }

    #[test]
    fn test_tournament_error_wrong_location() -> AppResult<()> {
        const MAX_PARTICIPANTS: usize = 4;

        let mut organizer = Team::random(None);
        let planet_id = PlanetId::default();
        organizer.space_cove = crate::core::SpaceCoveState::Ready { planet_id };
        organizer.current_location = TeamLocation::OnPlanet { planet_id };

        let timestamp = Tick::now();
        let mut tournament =
            Tournament::new(&organizer, MAX_PARTICIPANTS, timestamp + 30 * SECONDS)?;

        let mut players = PlayerMap::new();
        for _ in 0..MAX_PLAYERS_PER_GAME {
            let player = Player::default().randomize(None);
            players.insert(player.id, player);
        }

        let team = Team {
            id: TeamId::new_v4(),
            peer_id: Some(PeerId::random()),
            current_location: TeamLocation::OnPlanet {
                planet_id: PlanetId::new_v4(),
            },
            ..Default::default()
        };

        assert!(matches!(
            tournament.register_team(&team, players.clone(), timestamp),
            Err(e) if e.to_string() == "Team is not at the tournament location."
        ));

        let team = Team {
            id: TeamId::new_v4(),
            peer_id: Some(PeerId::random()),
            current_location: TeamLocation::Exploring {
                around: PlanetId::default(),
                started: Tick::now(),
                duration: 10000,
            },
            ..Default::default()
        };
        assert!(matches!(
            tournament.register_team(&team, players, timestamp),
            Err(e) if e.to_string() == "Team is not at the tournament location."
        ));
        Ok(())
    }
}
