use crate::{
    app_version,
    core::{Team, World},
    game_engine::{game::Game, types::TeamInGame},
    types::{AppResult, GameId, KartoffelId, PlanetId, SystemTimeTick, TeamId, Tick},
};
use anyhow::anyhow;
use itertools::Itertools;
use rand::{seq::IndexedRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

type TournamentId = uuid::Uuid;

// Note: all clients will run the same tournament deterministically,
// but teams can be registered only with a network message sent to the organizer,
// which will respond with the updated tournament.
// This means that clients are responsible for updating their team state
// to reflect the fact that they will be playing in the tournament.

#[derive(Debug, Default)]
pub struct Tournament {
    kartoffel_id: KartoffelId,
    organizer_id: TeamId,
    id: TournamentId,
    max_participants: usize,
    participants: Vec<TeamInGame>,
    current_round_participants: Vec<TeamInGame>,
    next_round_participants: Vec<TeamInGame>,
    location: PlanetId,
    starting_at: Tick,
    ended_at: Option<Tick>,
    winner: Option<TeamId>,
    app_version: [usize; 3],
}

impl Tournament {
    fn get_rng_seed(&self) -> [u8; 32] {
        let mut seed = [0; 32];
        seed[0..16].copy_from_slice(self.id.as_bytes());
        seed[16..32].copy_from_slice(self.organizer_id.as_bytes());

        seed
    }

    fn get_rng(&self) -> ChaCha8Rng {
        ChaCha8Rng::from_seed(self.get_rng_seed())
    }

    pub fn new(organizer_id: TeamId, max_participants: usize, location: PlanetId) -> Self {
        Self {
            organizer_id,
            id: TournamentId::new_v4(),
            max_participants,
            location,
            starting_at: Tick::now(),
            app_version: app_version(),
            ..Default::default()
        }
    }

    pub fn register_team(&mut self, team: &mut Team, world: &World) -> AppResult<()> {
        if self.has_started(Tick::now()) {
            return Err(anyhow!("Tournament has already started."));
        }
        if world.own_team_id != self.organizer_id {
            return Err(anyhow!(
                "Teams can be registered only by the tournament organizer."
            ));
        }

        if self.participants.len() == self.max_participants {
            return Err(anyhow!("Tournament is already full."));
        }

        let team_in_game = TeamInGame::from_team_id(&team.id, &world.teams, &world.players)?;
        self.participants.push(team_in_game);

        Ok(())
    }

    pub fn current_game(&self) -> Option<Game> {
        let game = self.next_game()?;
        if game.has_started(Tick::now()) {
            return Some(game);
        }

        None
    }

    pub fn next_game(&self) -> Option<Game> {
        let rng = &mut self.get_rng();
        let teams = if self.current_round_participants.len() >= 2 {
            self.current_round_participants
                .choose_multiple(rng, 2)
                .collect_vec()
        } else {
            return None;
        };

        let id = GameId::from_u128(rng.random());
        Some(Game::new(
            id,
            teams[0].clone(),
            teams[1].clone(),
            Tick::now(),
            self.location,
            0,
            "Tournamentolo",
        ))
    }

    pub fn has_started(&self, timestamp: Tick) -> bool {
        self.starting_at <= timestamp
    }

    pub fn has_ended(&self) -> bool {
        self.ended_at.is_some()
    }

    pub fn tick(&mut self, current_tick: Tick) {
        if self.has_ended() {
            return;
        }
    }
}
