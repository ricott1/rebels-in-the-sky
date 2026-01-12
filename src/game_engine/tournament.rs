use std::collections::HashMap;

use crate::{
    app_version,
    core::{Planet, Team, HOURS, SECONDS},
    game_engine::{game::Game, types::TeamInGame},
    types::{AppResult, GameId, KartoffelId, PlanetId, PlayerMap, SystemTimeTick, TeamId, Tick},
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
    organizer_id: TeamId,
    max_participants: usize,
    participants: HashMap<TeamId, TeamInGame>,
    game_ids: Vec<GameId>,
    planet_id: PlanetId,
    planet_name: String,
    planet_total_population: u32,
    pub starting_at: Tick,
    ended_at: Option<Tick>,
    pub winner: Option<TeamId>,
    app_version: [usize; 3],
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
            starting_at: tournament.starting_at,
            ended_at: tournament.ended_at,
            winner: tournament.winner,
            app_version: tournament.app_version,
        }
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
    initialized: bool,
    canceled: bool,
    pub registered_teams: HashMap<TeamId, TeamInGame>,
    pub participants: HashMap<TeamId, TeamInGame>,
    game_ids: Vec<GameId>,
    pending_team_for_next_game: Option<TeamId>,
    pub planet_id: PlanetId,
    pub planet_name: String,
    planet_total_population: u32,
    pub registrations_closing_at: Tick,
    pub starting_at: Tick,
    game_time_interval: Tick,
    ended_at: Option<Tick>,
    pub winner: Option<TeamId>,
    app_version: [usize; 3],
}

impl Tournament {
    const CONFIRMATION_STATE_DURATION: Tick = 5 * SECONDS;
    fn get_rng_seed(&self, timestamp: Tick) -> [u8; 32] {
        let mut seed = [0; 32];
        seed[0..16].copy_from_slice(self.id.as_bytes());
        seed[16..24].copy_from_slice(&timestamp.to_be_bytes());

        seed
    }

    fn get_rng(&self, timestamp: Tick) -> ChaCha8Rng {
        ChaCha8Rng::from_seed(self.get_rng_seed(timestamp))
    }

    fn new_game(
        &self,
        rng: &mut ChaCha8Rng,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
        starting_at: Tick,
    ) -> Game {
        Game::new(
            GameId::from_u128(rng.random()), // FIXME: check if this is enough to get good randomness
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

        if self.has_ended() {
            return TournamentState::Ended;
        }

        if self.has_started(timestamp) {
            return TournamentState::Started;
        }

        if self.are_registrations_open(timestamp) {
            return TournamentState::Registration;
        }

        TournamentState::Confirmation
    }

    pub fn cancel(&mut self) {
        // Cancel tournament. This can happen if the organizer team is not playing when the confirmation have to be sent.
        self.canceled = true;
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

        let starting_at = registrations_closing_at + Self::CONFIRMATION_STATE_DURATION;

        let tournament = Self {
            organizer_id: organizer.id,
            id: TournamentId::new_v4(),
            max_participants,
            starting_at,
            registrations_closing_at,
            app_version: app_version(),
            game_time_interval: 1 * HOURS,
            ..Default::default()
        };

        Ok(tournament)
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

    pub fn are_registrations_open(&self, timestamp: Tick) -> bool {
        timestamp <= self.registrations_closing_at
    }

    pub fn has_started(&self, timestamp: Tick) -> bool {
        self.starting_at <= timestamp
    }

    pub fn has_ended(&self) -> bool {
        self.ended_at.is_some()
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled
    }

    pub fn generate_next_games(
        &mut self,
        current_tick: Tick,
        games: HashMap<&GameId, &Game>,
    ) -> Vec<Game> {
        let mut new_games = vec![];
        if !self.has_started(current_tick) {
            return new_games;
        }

        if self.has_ended() {
            return new_games;
        }

        if self.participants.is_empty() {
            self.ended_at = Some(current_tick);
            return new_games;
        }

        // If games are empty here, it means they were not initialized yet, since the tournament has not ended.
        if !self.initialized {
            assert!(games.is_empty());
            assert!(!self.has_ended());
            assert!(self.winner.is_none());
            assert!(self.pending_team_for_next_game.is_none());

            let rng = &mut self.get_rng(self.starting_at);

            // Initialize initial games.
            // We shuffle the indecies of the participants, then pair them.
            // If the number of teams is odd, simply set pending_team_for_next_game (it's like a bye).
            let mut pairings = self.participants.values().collect_vec();
            pairings.shuffle(rng);

            assert!(pairings.len() == self.participants.len());

            let iter = pairings.iter();
            for &team_in_game in iter {
                if let Some(pending_team_id) = self.pending_team_for_next_game {
                    let pending_team = self
                        .participants
                        .get(&pending_team_id)
                        .expect("Team should be a participant");

                    let game = self.new_game(
                        rng,
                        team_in_game.clone(),
                        pending_team.clone(),
                        current_tick + self.game_time_interval,
                    );
                    self.game_ids.push(game.id);

                    new_games.push(game);
                    self.pending_team_for_next_game = None;
                } else {
                    self.pending_team_for_next_game = Some(team_in_game.team_id);
                }
            }

            if self.participants.len().is_multiple_of(2) {
                assert!(self.pending_team_for_next_game.is_none());
            } else {
                assert!(self.pending_team_for_next_game.is_some());
            }

            self.initialized = true;
            return new_games;
        }

        // At this point, games can be empty only if the last game was the final,
        // in which case the pending_team_for_next_game is the tournament winner
        if games.is_empty() {
            assert!(self.initialized);
            assert!(self.pending_team_for_next_game.is_some());
            self.winner = self.pending_team_for_next_game;
            self.ended_at = Some(current_tick);
            return vec![];
        }

        let rng = &mut self.get_rng(current_tick);
        let mut new_games = vec![];
        for game in games.values() {
            // Game could have ended because we process tournaments AFTER ticking games and BEFORE removing ended games.
            if !game.has_ended() {
                continue;
            }

            //FIXME: better choice for draws?
            let winner_team_id = if let Some(team_id) = game.winner {
                team_id
            } else if rng.random_bool(0.5) {
                game.home_team_in_game.team_id
            } else {
                game.away_team_in_game.team_id
            };
            if let Some(other_team_id) = self.pending_team_for_next_game {
                let home_team_in_game = self
                    .participants
                    .get(&other_team_id)
                    .expect("Team should be a participant");
                let away_team_in_game = self
                    .participants
                    .get(&winner_team_id)
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

        new_games
    }
}

#[cfg(test)]
mod tests {

    use crate::core::{Player, Team, TeamLocation, TickInterval, MAX_PLAYERS_PER_GAME, SECONDS};
    use crate::game_engine::Tournament;
    use crate::types::{AppResult, GameMap, PlanetId, PlayerMap, SystemTimeTick, TeamId, Tick};
    use libp2p::PeerId;

    #[test]
    fn test_tournament_determinism() -> AppResult<()> {
        const MAX_PARTICIPANTS: usize = 6;

        let mut organizer = Team::random(None);
        let planet_id = PlanetId::default();
        organizer.space_cove = crate::core::SpaceCoveState::Ready { planet_id };
        organizer.current_location = TeamLocation::OnPlanet { planet_id };

        let registrations_closing_at = Tick::now() + 30 * SECONDS;

        let mut tournament =
            Tournament::new(&organizer, MAX_PARTICIPANTS, registrations_closing_at)?;

        for _ in 0..MAX_PARTICIPANTS {
            let mut team = Team {
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

            tournament.register_team(&team, players.clone(), registrations_closing_at)?;
            team.tournament_registration_state =
                crate::core::TournamentRegistrationState::Preconfirmed {
                    tournament_id: tournament.id,
                };

            tournament.confirm_team_registration(&team, players, registrations_closing_at + 1)?;
        }

        let mut replay_tournament = tournament.clone();

        fn process_tournament(tournament: &mut Tournament) {
            let mut games = GameMap::new();
            let mut current_tick = tournament.starting_at;
            while tournament.winner.is_none() {
                for game in games.values_mut() {
                    if game.has_started(current_tick) {
                        game.tick(current_tick);
                    }
                }
                let tournament_games = games.iter().collect();
                let new_games = tournament.generate_next_games(current_tick, tournament_games);
                for game in new_games {
                    games.insert(game.id, game);
                }

                games.retain(|_, game| !game.has_ended());

                current_tick += TickInterval::SHORT;
            }

            assert!(tournament.winner.is_some());

            println!("Winner is {:#?}", tournament.winner);
        }

        process_tournament(&mut tournament);
        process_tournament(&mut replay_tournament);

        assert!(tournament.winner == replay_tournament.winner);

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
