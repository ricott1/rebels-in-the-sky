use crate::{
    app_version,
    core::{utils::is_default, Planet, Rated, Skill, Team, MINUTES, MIN_SKILL, SECONDS},
    game_engine::{
        game::{Game, GameSummary},
        timer,
        types::TeamInGame,
    },
    types::{
        AppResult, GameId, GameMap, GameSummaryMap, KartoffelId, PlanetId, PlayerMap,
        SystemTimeTick, TeamId, Tick,
    },
};
use anyhow::anyhow;
use itertools::Itertools;
use rand::{seq::SliceRandom, RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;
use strum::Display;

pub type TournamentId = uuid::Uuid;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentSummary {
    pub id: TournamentId,
    tournament_type: TournamentType,
    kartoffel_id: KartoffelId,
    pub organizer_id: TeamId,
    max_participants: usize,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub participant_ids: Vec<TeamId>,
    pub game_ids: Vec<GameId>,
    pub planet_id: PlanetId,
    planet_name: String,
    planet_total_population: u32,
    registrations_closing_at: Tick,
    pub ended_at: Option<Tick>,
    pub winner: Option<TeamId>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub winner_name: String,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    tournament_rating: Skill,
    app_version: [usize; 3],
}

impl Rated for TournamentSummary {
    fn rating(&self) -> Skill {
        self.tournament_rating
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
            participant_ids: tournament.participants.keys().copied().collect(),
            game_ids: tournament.games.iter().map(|g| g.id).collect(),
            planet_id: tournament.planet_id,
            planet_name: tournament.planet_name.clone(),
            planet_total_population: tournament.planet_total_population,
            registrations_closing_at: tournament.registrations_closing_at,
            ended_at: tournament.ended_at,
            winner: tournament.winner,
            winner_name: tournament
                .winner
                .map(|id| {
                    tournament
                        .participants
                        .get(&id)
                        .expect("Winner should be a participant")
                        .name
                        .clone()
                })
                .expect("Ended tournament should have a winner"),
            tournament_rating: tournament.rating(),
            app_version: tournament.app_version,
        }
    }

    pub fn starting_at(&self) -> Tick {
        self.registrations_closing_at
            + Tournament::CONFIRMATION_STATE_DURATION
            + Tournament::SYNCING_STATE_DURATION
    }

    pub fn name(&self) -> String {
        format!("{} {}", self.planet_name, self.tournament_type)
    }
}

#[derive(Debug, Default, Display, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum TournamentType {
    #[default]
    Cup,
    Supercup,
}

impl TournamentType {
    pub fn max_participants(&self) -> usize {
        match self {
            Self::Cup => 4,
            Self::Supercup => 8,
        }
    }

    pub fn registration_duration(&self) -> Tick {
        match self {
            Self::Cup => 5 * MINUTES,
            Self::Supercup => 45 * MINUTES,
        }
    }
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

// Note: all clients will run the same tournament deterministically, but teams can be registered
// only with a network message sent to the organizer which will respond with the updated tournament state.
// This means that clients are responsible for updating their team state to reflect the fact that they will be playing in the tournament.

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
    pub games: Vec<Game>,
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

    pub fn new(organizer: &Team, tournament_type: TournamentType) -> AppResult<Self> {
        organizer.can_organize_tournament()?;

        let now = Tick::now();
        let registrations_closing_at = Tick::now() + tournament_type.registration_duration();
        if registrations_closing_at <= now {
            return Err(anyhow!("Tournament is closing registrations in the past!"));
        }

        let max_participants = tournament_type.max_participants();

        let tournament = Self {
            organizer_id: organizer.id,
            id: TournamentId::new_v4(),
            max_participants,
            registrations_closing_at,
            app_version: app_version(),
            game_time_interval: 30 * MINUTES,
            tournament_type,
            ..Default::default()
        };

        Ok(tournament)
    }

    pub fn test(participants: usize, max_participants: usize) -> Self {
        let mut t = Self {
            id: TournamentId::from_u128(1),
            max_participants,
            registrations_closing_at: Tick::now() + SECONDS,
            game_time_interval: 30 * MINUTES,
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
        format!("{} {}", self.planet_name, self.tournament_type)
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
        !self.games.is_empty()
    }

    fn shuffled_participant_ids(&self, rng: &mut ChaCha8Rng) -> Vec<TeamId> {
        let mut ids = self.participants.keys().copied().sorted().collect_vec();
        ids.shuffle(rng);
        ids
    }

    pub fn initialize(&mut self) -> Vec<Game> {
        let mut new_games = vec![];
        assert!(!self.has_ended());
        assert!(self.winner.is_none());
        assert!(self.pending_team_for_next_game.is_none());

        let rng = &mut self.get_rng(self.starting_at());

        // Shuffle the participants and pair them up; an odd one out is left as pending_team_for_next_game (a bye).
        let pairings = self.shuffled_participant_ids(rng);
        assert!(pairings.len() == self.participants.len());

        for (idx, &team_id) in pairings.iter().enumerate() {
            if let Some(pending_team_id) = self.pending_team_for_next_game.take() {
                let pending_team = self
                    .participants
                    .get(&pending_team_id)
                    .expect("Team should be a participant")
                    .clone();
                let team_in_game = self
                    .participants
                    .get(&team_id)
                    .expect("Team should be a participant")
                    .clone();

                let game = self.new_game(
                    rng,
                    pending_team,
                    team_in_game,
                    self.starting_at()
                        + self.game_time_interval * (idx + 1) as u64 / pairings.len() as u64,
                );
                self.games.push(game.clone());
                new_games.push(game);
            } else {
                self.pending_team_for_next_game = Some(team_id);
            }
        }

        if self.participants.len().is_multiple_of(2) {
            assert!(self.pending_team_for_next_game.is_none());
        } else {
            assert!(self.pending_team_for_next_game.is_some());
        }

        new_games
    }

    fn initial_bye(&self) -> Option<TeamId> {
        if self.participants.len() % 2 == 0 {
            return None;
        }
        let rng = &mut self.get_rng(self.starting_at());
        self.shuffled_participant_ids(rng).last().copied()
    }

    pub fn generate_next_games(
        &mut self,
        current_tick: Tick,
        games: &GameMap,
        past_games: &GameSummaryMap,
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

        // Winner of every finished game, taken from `games` or `past_games` alike:
        // advancement must not depend on which one holds it, since a spectator receives finished games straight into past_games.
        let mut completed: Vec<(TeamId, Tick, GameId)> = vec![];
        for game in self.games.iter() {
            // A result may sit in `games` (just ended, not yet archived) or `past_games`;
            // a game with no result yet is skipped (we wait, we don't cancel).
            let result = match games.get(&game.id) {
                Some(active) => {
                    active
                        .has_ended()
                        .then_some((active.winner, active.starting_at, active.id))
                }
                None => past_games
                    .get(&game.id)
                    .map(|s| (s.winner, s.starting_at, s.id)),
            };
            if let Some((winner, starting_at, id)) = result {
                let winner =
                    winner.ok_or_else(|| anyhow!("Tournament game should have a winner."))?;
                completed.push((winner, starting_at, id));
            }
        }

        // Rebuild the pairing queue from scratch each call so a repeated call can't advance it twice.
        // Winners pair up in the order games finish.
        completed.sort_by_key(|&(_, tick, id)| (tick, id));
        let mut pending = self.initial_bye();
        let mut required: Vec<(TeamId, TeamId, Tick)> = vec![];
        for &(winner, starting_at, _) in &completed {
            if let Some(other) = pending.take() {
                required.push((other, winner, starting_at));
            } else {
                pending = Some(winner);
            }
        }

        // Every game played: the team left without an opponent is the champion.
        if completed.len() == self.participants.len() - 1 {
            self.winner =
                Some(pending.ok_or_else(|| anyhow!("Finished tournament should have a winner."))?);
            self.ended_at = Some(current_tick);
            log::info!("Tournament {} is over.", self.id);
            return Ok(vec![]);
        }

        // Schedule only the next-round games that don't exist yet: two teams meet at most once,
        // so an existing matchup was already created on an earlier tick.
        let mut new_games = vec![];
        for (home_id, away_id, after_starting_at) in required {
            let exists = self.games.iter().any(|g| {
                let pair = [g.home_team_in_game.team_id, g.away_team_in_game.team_id];
                pair.contains(&home_id) && pair.contains(&away_id)
            });
            if exists {
                continue;
            }

            let home = self
                .participants
                .get(&home_id)
                .expect("Team should be a participant")
                .clone();
            let away = self
                .participants
                .get(&away_id)
                .expect("Team should be a participant")
                .clone();
            let rng = &mut self.get_rng(self.games.len() as u64 + 1);
            let new_game = self.new_game(
                rng,
                home,
                away,
                after_starting_at
                    + timer::MAX_TIME_IN_SECONDS as Tick * SECONDS
                    + self.game_time_interval,
            );
            self.games.push(new_game.clone());
            new_games.push(new_game);
        }

        Ok(new_games)
    }

    pub fn active_games<'a>(&'a self, games: &'a GameMap) -> Vec<&'a Game> {
        self.games
            .iter()
            .filter_map(|game| games.get(&game.id))
            .collect::<Vec<&Game>>()
    }

    pub fn past_game_summaries<'a>(
        &'a self,
        past_games: &'a GameSummaryMap,
    ) -> Vec<&'a GameSummary> {
        self.games
            .iter()
            .filter_map(|game| past_games.get(&game.id))
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
    use crate::game_engine::game::{Game, GameSummary};
    use crate::game_engine::{Tournament, TournamentState, TournamentType};
    use crate::types::{
        AppResult, GameMap, GameSummaryMap, PlanetId, PlayerMap, SystemTimeTick, TeamId, Tick,
    };
    use itertools::Itertools;
    use libp2p::PeerId;

    #[test]
    fn test_tournament_determinism() -> AppResult<()> {
        let mut tournament = Tournament::test(6, 8);
        tournament.registrations_closing_at = Tick::now() + 10 * SECONDS;
        let mut replay_tournament = tournament.clone();

        fn process_tournament(tournament: &mut Tournament) -> AppResult<()> {
            let mut games = GameMap::new();
            let mut past_games = GameSummaryMap::new();

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

                let new_games =
                    tournament.generate_next_games(current_tick, &games, &past_games)?;

                for game in games.values().filter(|g| g.has_ended()) {
                    past_games.insert(game.id, GameSummary::from_game(game));
                }

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
        let mut past_game_summaries = GameSummaryMap::new();
        let mut past_games = GameMap::new();

        for game in tournament.initialize() {
            games.insert(game.id, game);
        }

        let mut current_tick = 0;
        while !tournament.has_ended() {
            for game in games.values_mut() {
                game.tick(current_tick);
            }

            let new_games =
                tournament.generate_next_games(current_tick, &games, &past_game_summaries)?;

            for game in games.values().filter(|g| g.has_ended()) {
                past_game_summaries.insert(game.id, GameSummary::from_game(game));
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
        let mut organizer = Team::random(None);
        let planet_id = PlanetId::default();
        let mut cove = crate::core::SpaceCove::under_construction(planet_id);
        cove.finish_contruction();
        cove.upgrades
            .insert(crate::core::SpaceCoveUpgradeTarget::Stadium);
        organizer.space_cove = Some(cove);
        organizer.current_location = TeamLocation::OnPlanet { planet_id };

        let mut tournament = Tournament::new(&organizer, TournamentType::Cup)?;

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
            tournament.register_team(&team, players, tournament.registrations_closing_at + 1),
            Err(e) if e.to_string() == "Tournament registrations are closed."
        ));
        Ok(())
    }

    #[test]
    fn test_tournament_error_wrong_location() -> AppResult<()> {
        let mut organizer = Team::random(None);
        let planet_id = PlanetId::default();
        let mut cove = crate::core::SpaceCove::under_construction(planet_id);
        cove.finish_contruction();
        cove.upgrades
            .insert(crate::core::SpaceCoveUpgradeTarget::Stadium);
        organizer.space_cove = Some(cove);
        organizer.current_location = TeamLocation::OnPlanet { planet_id };

        let timestamp = Tick::now();
        let mut tournament = Tournament::new(&organizer, TournamentType::Supercup)?;

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

    #[test]
    fn test_cancellation_resets_state() -> AppResult<()> {
        let mut tournament = Tournament::test(4, 4);
        assert!(!tournament.is_canceled());
        assert_eq!(tournament.state(Tick::now()), TournamentState::Registration);

        tournament.cancel();

        assert!(tournament.is_canceled());
        assert_eq!(tournament.state(Tick::now()), TournamentState::Canceled);
        Ok(())
    }

    #[test]
    fn test_generate_next_games_with_past_games_fallback() -> AppResult<()> {
        // This test verifies that generate_next_games works correctly when
        // some completed games have been moved from `games` to `past_games`.
        // The flow is: generate_next_games processes ended games in `games` first,
        // then ended games are moved to past_games. On subsequent ticks, those games
        // are only in past_games but should still be correctly counted.
        let mut tournament = Tournament::test(6, 8);
        tournament.registrations_closing_at = 0;
        let mut games = GameMap::new();
        let mut past_game_summaries = GameSummaryMap::new();

        for game in tournament.initialize() {
            games.insert(game.id, game);
        }

        let mut current_tick: Tick = 0;
        let mut moved_to_past = false;

        while !tournament.has_ended() {
            for game in games.values_mut() {
                game.tick(current_tick);
            }

            // Call generate_next_games with current games + past_games
            let new_games =
                tournament.generate_next_games(current_tick, &games, &past_game_summaries)?;

            // Move ended games to past_game_summaries (simulating world cleanup)
            let ended_ids: Vec<_> = games
                .values()
                .filter(|g| g.has_ended())
                .map(|g| g.id)
                .collect();

            for id in &ended_ids {
                if let Some(game) = games.get(id) {
                    past_game_summaries.insert(*id, GameSummary::from_game(game));
                    if !moved_to_past {
                        moved_to_past = true;
                    }
                }
            }

            games.retain(|_, g| !g.has_ended());

            for game in new_games {
                games.insert(game.id, game);
            }

            current_tick += TickInterval::SHORT;
        }

        assert!(
            moved_to_past,
            "Should have moved at least one game to past_games"
        );
        assert!(tournament.winner.is_some());
        // All tournament games should be accounted for in past_game_summaries
        assert_eq!(
            past_game_summaries.len(),
            tournament.participants.len() - 1,
            "All tournament games should be in past_game_summaries"
        );
        Ok(())
    }

    #[test]
    fn test_game_scheduling_uses_starting_at_plus_max_time() -> AppResult<()> {
        use crate::game_engine::timer;

        let mut tournament = Tournament::test(4, 4);
        tournament.registrations_closing_at = 0;
        let mut games = GameMap::new();
        let mut past_game_summaries = GameSummaryMap::new();

        for game in tournament.initialize() {
            games.insert(game.id, game);
        }

        let mut current_tick: Tick = 0;
        let mut found_new_game = false;

        while !tournament.has_ended() && !found_new_game {
            for game in games.values_mut() {
                game.tick(current_tick);
            }

            // Collect starting_at for games that just ended (before generate_next_games consumes them)
            let ended_starting_ats: Vec<Tick> = games
                .values()
                .filter(|g| g.has_ended())
                .map(|g| g.starting_at)
                .collect();

            let new_games =
                tournament.generate_next_games(current_tick, &games, &past_game_summaries)?;

            for game in games.values().filter(|g| g.has_ended()) {
                past_game_summaries.insert(game.id, GameSummary::from_game(game));
            }

            games.retain(|_, g| !g.has_ended());

            for game in &new_games {
                // The new game should be scheduled at:
                // ended_game.starting_at + MAX_TIME_IN_SECONDS (as Tick) + game_time_interval
                let game_time_interval = 30 * crate::core::MINUTES;
                let matches_any = ended_starting_ats.iter().any(|&ended_start| {
                    let expected = ended_start
                        + timer::MAX_TIME_IN_SECONDS as Tick * SECONDS
                        + game_time_interval;
                    game.starting_at == expected
                });
                assert!(
                    matches_any,
                    "New game starting_at {} should be based on an ended game's starting_at + MAX_TIME_IN_SECONDS + interval",
                    game.starting_at
                );
                found_new_game = true;
            }

            for game in new_games {
                games.insert(game.id, game);
            }

            current_tick += TickInterval::SHORT;
        }

        assert!(found_new_game, "Should have found at least one new game");
        Ok(())
    }

    #[test]
    fn test_tournament_with_2_participants() -> AppResult<()> {
        let mut tournament = Tournament::test(2, 4);
        tournament.registrations_closing_at = 0;
        let mut games = GameMap::new();
        let mut past_game_summaries = GameSummaryMap::new();

        let initial_games = tournament.initialize();
        assert_eq!(
            initial_games.len(),
            1,
            "2 participants should produce 1 initial game"
        );

        for game in initial_games {
            games.insert(game.id, game);
        }

        let mut current_tick: Tick = 0;
        while !tournament.has_ended() {
            for game in games.values_mut() {
                game.tick(current_tick);
            }

            let new_games =
                tournament.generate_next_games(current_tick, &games, &past_game_summaries)?;

            for game in games.values().filter(|g| g.has_ended()) {
                past_game_summaries.insert(game.id, GameSummary::from_game(game));
            }

            games.retain(|_, g| !g.has_ended());

            for game in new_games {
                games.insert(game.id, game);
            }

            current_tick += TickInterval::SHORT;
        }

        assert!(tournament.winner.is_some());
        assert_eq!(tournament.games.len(), 1, "2 participants = 1 total game");
        Ok(())
    }

    #[test]
    fn test_tournament_odd_participants_bye_handling() -> AppResult<()> {
        for num_participants in [3, 5, 7] {
            let mut tournament = Tournament::test(num_participants, 8);
            tournament.registrations_closing_at = 0;
            let mut games = GameMap::new();
            let mut past_game_summaries = GameSummaryMap::new();

            for game in tournament.initialize() {
                games.insert(game.id, game);
            }

            let mut current_tick: Tick = 0;
            while !tournament.has_ended() {
                for game in games.values_mut() {
                    game.tick(current_tick);
                }

                let new_games =
                    tournament.generate_next_games(current_tick, &games, &past_game_summaries)?;

                for game in games.values().filter(|g| g.has_ended()) {
                    past_game_summaries.insert(game.id, GameSummary::from_game(game));
                }

                games.retain(|_, g| !g.has_ended());

                for game in new_games {
                    games.insert(game.id, game);
                }

                current_tick += TickInterval::SHORT;
            }

            assert!(
                tournament.winner.is_some(),
                "Tournament with {num_participants} participants should have a winner"
            );
            assert_eq!(
                tournament.games.len(),
                num_participants - 1,
                "Tournament with {num_participants} participants should have {} total games",
                num_participants - 1
            );
        }
        Ok(())
    }

    fn play_to_end(mut game: Game) -> Game {
        let mut current_tick = game.starting_at;
        let mut ticks = 0;
        while !game.has_ended() {
            if game.has_started(current_tick) {
                game.tick(current_tick);
            }
            current_tick += TickInterval::SHORT;
            ticks += 1;
            assert!(ticks < 100_000, "game did not finish");
        }
        game
    }

    // The bracket must advance identically whether a result is seen via `games` or arrives already-ended via `past_games`.
    #[test]
    fn test_bracket_advances_regardless_of_result_routing() -> AppResult<()> {
        let base = Tournament::test(4, 4);

        let mut proto = base.clone();
        proto.registrations_closing_at = 0;
        let semis = proto.initialize();
        assert_eq!(semis.len(), 2, "4 participants -> 2 semifinals");

        let sf0 = play_to_end(semis[0].clone());
        let sf1 = play_to_end(semis[1].clone());
        let end_tick = sf0.starting_at.max(sf1.starting_at) + 100 * TickInterval::SHORT;

        // Participant path: both ended semis are visible via `games` in one pass.
        let mut part = base.clone();
        part.registrations_closing_at = 0;
        part.initialize();
        {
            let mut games = GameMap::new();
            games.insert(sf0.id, sf0.clone());
            games.insert(sf1.id, sf1.clone());
            let past = GameSummaryMap::new();
            part.generate_next_games(end_tick, &games, &past)?;
        }
        assert_eq!(
            part.games.len(),
            3,
            "participant path should schedule the final (2 semis + 1 final)"
        );

        // Spectator path: sf0 seen via `games`, sf1 arrives already-ended via `past_games`
        // (network_callback routes ended games straight to past_games, bypassing pairing).
        let mut spec = base.clone();
        spec.registrations_closing_at = 0;
        spec.initialize();
        {
            let mut games = GameMap::new();
            let mut past = GameSummaryMap::new();

            games.insert(sf0.id, sf0.clone());
            spec.generate_next_games(end_tick, &games, &past)?;
            past.insert(sf0.id, GameSummary::from_game(&sf0));
            games.retain(|_, g| !g.has_ended());

            past.insert(sf1.id, GameSummary::from_game(&sf1));
            spec.generate_next_games(end_tick, &games, &past)?;
        }

        assert_eq!(
            spec.games.len(),
            part.games.len(),
            "spectator bracket should match the participant bracket (both 3 games)"
        );

        Ok(())
    }

    #[test]
    fn test_generate_next_games_is_idempotent() -> AppResult<()> {
        let mut tournament = Tournament::test(4, 4);
        tournament.registrations_closing_at = 0;
        let semis = tournament.initialize();
        let sf0 = play_to_end(semis[0].clone());
        let sf1 = play_to_end(semis[1].clone());
        let end_tick = sf0.starting_at.max(sf1.starting_at) + 100 * TickInterval::SHORT;

        let mut games = GameMap::new();
        games.insert(sf0.id, sf0.clone());
        games.insert(sf1.id, sf1.clone());
        let past = GameSummaryMap::new();

        tournament.generate_next_games(end_tick, &games, &past)?;
        assert_eq!(tournament.games.len(), 3, "2 semis + 1 final");

        tournament.generate_next_games(end_tick, &games, &past)?;
        assert_eq!(
            tournament.games.len(),
            3,
            "reprocessing the same results must not duplicate the final (now {} games)",
            tournament.games.len()
        );
        Ok(())
    }

    // A spectator whose peer disconnects still finishes the tournament from the results it already has.
    #[test]
    fn test_bracket_completes_with_all_results_via_past_games() -> AppResult<()> {
        let mut tournament = Tournament::test(4, 4);
        tournament.registrations_closing_at = 0;
        let games = GameMap::new();
        let mut past = GameSummaryMap::new();

        for game in tournament.initialize() {
            past.insert(game.id, GameSummary::from_game(&play_to_end(game)));
        }
        let end_tick =
            past.values().map(|g| g.starting_at).max().unwrap() + 100 * TickInterval::SHORT;

        // First pass pairs the two semifinal winners into the final.
        let final_games = tournament.generate_next_games(end_tick, &games, &past)?;
        assert_eq!(final_games.len(), 1, "the final should be scheduled");

        // Play the final (still only via past_games) and finish.
        past.insert(
            final_games[0].id,
            GameSummary::from_game(&play_to_end(final_games[0].clone())),
        );
        tournament.generate_next_games(end_tick, &games, &past)?;

        assert!(tournament.has_ended());
        assert!(tournament.winner.is_some());
        assert_eq!(tournament.games.len(), 3);
        Ok(())
    }

    fn play_tournament_on_schedule(tournament: &mut Tournament) -> AppResult<()> {
        let mut games = GameMap::new();
        let mut past_game_summaries = GameSummaryMap::new();

        for game in tournament.initialize() {
            games.insert(game.id, game);
        }

        let start = tournament.starting_at();
        let mut current_tick = start - start % TickInterval::SHORT;
        let mut ticks = 0;
        while !tournament.has_ended() {
            current_tick += TickInterval::SHORT;
            ticks += 1;
            assert!(ticks < 100_000, "tournament did not finish");

            for game in games.values_mut() {
                if game.has_started(current_tick) {
                    game.tick(current_tick);
                }
            }

            let new_games =
                tournament.generate_next_games(current_tick, &games, &past_game_summaries)?;

            for game in games.values().filter(|g| g.has_ended()) {
                past_game_summaries.insert(game.id, GameSummary::from_game(game));
            }
            games.retain(|_, g| !g.has_ended());

            for game in new_games {
                games.insert(game.id, game);
            }
        }
        Ok(())
    }

    #[test]
    fn test_tournament_max_participants_cup() -> AppResult<()> {
        let max = TournamentType::Cup.max_participants(); // 4
        let mut tournament = Tournament::test(max, max);
        play_tournament_on_schedule(&mut tournament)?;
        assert!(tournament.winner.is_some());
        assert_eq!(tournament.games.len(), max - 1);
        Ok(())
    }

    #[test]
    fn test_tournament_max_participants_supercup() -> AppResult<()> {
        let max = TournamentType::Supercup.max_participants(); // 8
        let mut tournament = Tournament::test(max, max);
        play_tournament_on_schedule(&mut tournament)?;
        assert!(tournament.winner.is_some());
        assert_eq!(tournament.games.len(), max - 1);
        Ok(())
    }

    #[test]
    fn test_canceled_tournament_state() {
        let mut tournament = Tournament::test(4, 4);
        assert!(!tournament.is_canceled());

        tournament.cancel();

        assert!(tournament.is_canceled());
        assert_eq!(tournament.state(Tick::now()), TournamentState::Canceled);
        // Canceled tournament should have no games generated
        assert!(tournament.games.is_empty());
    }
}
