use std::collections::HashMap;

use super::constants::POLOSIUS_TEAM_ID;
use crate::{
    core::{
        Player, SpaceCoveState, Team, LIGHT_YEAR, MAX_PLAYERS_PER_GAME, SATOSHI_PER_BITCOIN, WEEKS,
    },
    game_engine::game::GameSummary,
    types::{GameId, PlayerId, SystemTimeTick, Tick},
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
            Self::Pirate => matches!(team.space_cove, SpaceCoveState::Ready { .. }),
            Self::Traveller => team.total_travelled >= LIGHT_YEAR,
            Self::Veteran => {
                team.creation_time != Tick::default()
                    && (Tick::now() - team.creation_time) >= 52 * WEEKS
            }
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Defiant => "Defiant: Defeat the emeperial team Polosius III.",
            Self::Maximalist => "Bitcoin Maximalist: Held at least 1 BTC at some point in time.",
            Self::MultiKulti => {
                "MultiKulti: Have pirates from 7 different populations in the crew."
            }
            Self::Pirate => "Pirate: Build the space cove on an asteroid.",
            Self::Traveller => "Traveller: Travel through the galaxy for at least 1 light year.",
            Self::Veteran => "Veteran: Played for a year.",
        }
    }

    pub fn symbol(&self) -> char {
        match self {
            Self::Defiant => 'D',
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
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    use crate::{
        app::App,
        core::{Honour, Player, Population, Region, Resource, Team},
        types::AppResult,
    };

    #[test]
    fn test_conditions_not_met_multikulti() -> AppResult<()> {
        let app = &mut App::test_default()?;
        let rng = &mut ChaCha8Rng::from_os_rng();

        let mut team = Team::random(rng);
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
            &app.world.players
        ));

        Ok(())
    }

    #[test]
    fn test_conditions_met_multikulti() -> AppResult<()> {
        let app = &mut App::test_default()?;
        let rng = &mut ChaCha8Rng::from_os_rng();

        let mut team = Team::random(rng);
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
            &app.world.players
        ));

        Ok(())
    }
}
