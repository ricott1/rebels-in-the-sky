use std::{borrow::Cow, collections::HashMap};

use super::constants::POLOSIUS_TEAM_ID;
use crate::{
    core::{
        Planet, Player, SpaceCoveState, Team, LIGHT_YEAR, MAX_NUM_ASTEROID_PER_TEAM,
        MAX_PLAYERS_PER_GAME, SATOSHI_PER_BITCOIN, WEEKS,
    },
    game_engine::game::GameSummary,
    types::{GameId, PlanetId, PlayerId, SystemTimeTick, Tick},
};
use itertools::Itertools;
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::{Display, EnumIter};

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize_repr, Deserialize_repr, EnumIter, Display,
)]
#[repr(u8)]
pub enum Honour {
    Defiant,
    Galactic,
    Maximalist,
    MultiKulti,
    Pirate,
    Traveller,
    Veteran,
}

impl Honour {
    pub fn conditions_met(
        self,
        team: &Team,
        past_games: &HashMap<GameId, GameSummary>,
        players: &HashMap<PlayerId, Player>,
        planets: &HashMap<PlanetId, Planet>,
    ) -> bool {
        match self {
            Self::Defiant => {
                past_games
                    .values()
                    .filter(|g| {
                        team.id != POLOSIUS_TEAM_ID
                            && g.is_network
                            && matches!(g.winner, Some(team_id) if team_id == team.id)
                            && (g.home_team_id == POLOSIUS_TEAM_ID
                                || g.away_team_id == POLOSIUS_TEAM_ID)
                    })
                    .count()
                    > 0
            }
            Self::Galactic => {
                let maybe_asteroids = team
                    .asteroid_ids
                    .iter()
                    .map(|id| planets.get(id))
                    .collect::<Option<Vec<&Planet>>>();

                maybe_asteroids
                    .map(|asteroids| {
                        asteroids
                            .iter()
                            .map(|asteroid| {
                                asteroid
                                    .satellite_of
                                    .expect("Asteroid should have a parent.")
                            })
                            .unique()
                            .count()
                            == MAX_NUM_ASTEROID_PER_TEAM
                    })
                    .unwrap_or_default()
            }
            Self::Maximalist => team.balance() >= SATOSHI_PER_BITCOIN,
            Self::MultiKulti => {
                let players = team
                    .player_ids
                    .iter()
                    .map(|id| players.get(id))
                    .collect::<Option<Vec<&Player>>>()
                    .unwrap_or_default();

                players
                    .iter()
                    .map(
                        |p| // Discriminant disregards internal fields (Humans have a region internal field)
                        std::mem::discriminant(&p.info.population),
                    )
                    .unique()
                    .count()
                    >= MAX_PLAYERS_PER_GAME
            }
            Self::Pirate => team
                .space_cove
                .as_ref()
                .filter(|cove| cove.state == SpaceCoveState::Ready)
                .is_some(),
            Self::Traveller => team.total_travelled >= LIGHT_YEAR,
            Self::Veteran => {
                team.creation_time != Tick::default()
                    && (Tick::now() - team.creation_time) >= 52 * WEEKS
            }
        }
    }

    pub fn description(&self) -> Cow<'static, str> {
        match self {
            Self::Defiant => Cow::Borrowed("Defiant: Defeat the imperial team Polosius III."),
            Self::Galactic => Cow::Owned(format!(
                "Galactic: Control an asteroid around {MAX_NUM_ASTEROID_PER_TEAM} different planets."
            )),
            Self::Maximalist => {
                Cow::Borrowed("Bitcoin Maximalist: Held at least 1 BTC at some point in time.")
            }
            Self::MultiKulti => {
                Cow::Borrowed("MultiKulti: Have pirates from 7 different populations in the crew.")
            }
            Self::Pirate => Cow::Borrowed("Pirate: Build the space cove on an asteroid."),
            Self::Traveller => {
                Cow::Borrowed("Traveller: Travel through the galaxy for at least 1 light year.")
            }
            Self::Veteran => Cow::Borrowed("Veteran: Played for a year."),
        }
    }

    pub fn symbol(&self) -> char {
        match self {
            Self::Defiant => 'D',
            Self::Galactic => 'G',
            Self::Maximalist => 'B',
            Self::MultiKulti => 'M',
            Self::Pirate => 'P',
            Self::Traveller => 'T',
            Self::Veteran => 'V',
        }
    }
}

#[cfg(test)]
mod tests {

    use itertools::Itertools;

    use crate::{
        app::App,
        core::{
            Honour, Planet, Player, Population, Region, Resource, Team, MAX_NUM_ASTEROID_PER_TEAM,
        },
        types::AppResult,
    };

    #[test]
    fn test_conditions_not_met_multikulti() -> AppResult<()> {
        let app = &mut App::test_default()?;

        let mut team = Team::random(None);
        let team_id = team.id;
        team.add_resource(Resource::SATOSHI, 1_000_000)?;
        assert!(team.player_ids.len() == 0);
        app.world.teams.insert(team_id, team);

        let mut player_ids = vec![];

        let test_populations = vec![
            Population::Human {
                region: Region::Italy,
            },
            Population::Human {
                region: Region::Kurdistan,
            },
            Population::Human {
                region: Region::India,
            },
            Population::Pupparoll,
            Population::Galdari,
            Population::Yardalaim,
            Population::Polpett,
        ];
        for &population in test_populations.iter() {
            let player = Player::default().with_population(population);
            let player_id = player.id;
            app.world.players.insert(player_id, player);
            player_ids.push(player_id);
        }

        for player_id in player_ids.iter() {
            app.world.hire_player_for_team(player_id, &team_id)?;
        }

        let team = app.world.teams.get(&team_id).unwrap();
        assert!(team.player_ids.len() == 7);

        assert!(!Honour::MultiKulti.conditions_met(
            &team,
            &app.world.past_games,
            &app.world.players,
            &app.world.planets
        ));

        Ok(())
    }

    #[test]
    fn test_conditions_met_multikulti() -> AppResult<()> {
        let app = &mut App::test_default()?;

        let mut team = Team::random(None);
        let team_id = team.id;
        team.add_resource(Resource::SATOSHI, 1_000_000)?;
        assert!(team.player_ids.len() == 0);
        app.world.teams.insert(team_id, team);

        let mut player_ids = vec![];

        let test_populations = vec![
            Population::Human {
                region: Region::Italy,
            },
            Population::Juppa,
            Population::Octopulp,
            Population::Pupparoll,
            Population::Galdari,
            Population::Yardalaim,
            Population::Polpett,
        ];
        for &population in test_populations.iter() {
            let player = Player::default().with_population(population);
            let player_id = player.id;
            app.world.players.insert(player_id, player);
            player_ids.push(player_id);
        }

        for player_id in player_ids.iter() {
            app.world.hire_player_for_team(player_id, &team_id)?;
        }

        let team = app.world.teams.get(&team_id).unwrap();
        assert!(team.player_ids.len() == 7);

        assert!(Honour::MultiKulti.conditions_met(
            &team,
            &app.world.past_games,
            &app.world.players,
            &app.world.planets
        ));

        Ok(())
    }

    #[test]
    fn test_conditions_met_galactic() -> AppResult<()> {
        let app = &mut App::test_default()?;

        let mut team = Team::random(None);

        let parent_planets = app
            .world
            .planets
            .values()
            .filter(|p| p.total_population() > 0)
            .map(|p| p.id)
            .collect_vec();

        for idx in 0..MAX_NUM_ASTEROID_PER_TEAM {
            let asteroid = Planet::asteroid(
                "name".to_string(),
                "filename".to_string(),
                parent_planets[idx],
            );
            team.asteroid_ids.push(asteroid.id);
            app.world.planets.insert(asteroid.id, asteroid);
        }

        assert!(Honour::Galactic.conditions_met(
            &team,
            &app.world.past_games,
            &app.world.players,
            &app.world.planets
        ));

        Ok(())
    }
}
