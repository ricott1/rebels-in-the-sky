use super::{
    action::{Action, ActionOutput, ActionSituation, EngineAction},
    constants::*,
    end_of_quarter::EndOfQuarter,
    substitution::Substitution,
    timer::{Period, Timer},
    types::{GameStatsMap, Possession, TeamInGame},
};
use crate::{
    types::{GameId, PlanetId, PlayerId, SortablePlayerMap, TeamId, Tick},
    world::{
        planet::Planet,
        player::{Player, Trait},
        position::MAX_POSITION,
        skill::GameSkill,
    },
};
use itertools::Itertools;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummary {
    pub id: GameId,
    pub home_team_id: TeamId,
    pub away_team_id: TeamId,
    pub home_team_name: String,
    pub away_team_name: String,
    pub home_quarters_score: [u16; 4],
    pub away_quarters_score: [u16; 4],
    pub location: PlanetId,
    pub attendance: u32,
    pub starting_at: Tick,
    pub ended_at: Option<Tick>,
    pub winner: Option<TeamId>,
}

impl GameSummary {
    pub fn from_game(game: &Game) -> GameSummary {
        let mut home_quarters_score = [0 as u16; 4];
        let mut away_quarters_score = [0 as u16; 4];
        for action in game.action_results.iter() {
            // The first action of the break period will update only the correct element of the partial score.
            // For quarters>1, we need to remove previous quarters score to get only the partial score of the quarter.
            match action.start_at.period() {
                Period::B1 => {
                    home_quarters_score[0] = action.home_score;
                    away_quarters_score[0] = action.away_score;
                }
                Period::B2 => {
                    home_quarters_score[1] = action.home_score - home_quarters_score[0];
                    away_quarters_score[1] = action.away_score - away_quarters_score[0];
                }
                Period::B3 => {
                    home_quarters_score[2] =
                        action.home_score - home_quarters_score[0] - home_quarters_score[1];
                    away_quarters_score[2] =
                        action.away_score - away_quarters_score[0] - away_quarters_score[1];
                }
                Period::B4 => {
                    home_quarters_score[3] = action.home_score
                        - home_quarters_score[0]
                        - home_quarters_score[1]
                        - home_quarters_score[2];
                    away_quarters_score[3] = action.away_score
                        - away_quarters_score[0]
                        - away_quarters_score[1]
                        - away_quarters_score[2];
                }
                _ => continue,
            }
        }

        Self {
            id: game.id.clone(),
            home_team_id: game.home_team_in_game.team_id,
            away_team_id: game.away_team_in_game.team_id,
            home_team_name: game.home_team_in_game.name.clone(),
            away_team_name: game.away_team_in_game.name.clone(),
            home_quarters_score,
            away_quarters_score,
            location: game.location,
            attendance: game.attendance,
            starting_at: game.starting_at,
            ended_at: game.ended_at,
            winner: game.winner,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GameMVPSummary {
    pub name: String,
    pub score: u32,
    pub best_stats: [(String, u8, u32); 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Game {
    pub id: GameId,
    pub home_team_in_game: TeamInGame,
    pub away_team_in_game: TeamInGame,
    pub location: PlanetId,
    pub attendance: u32,
    pub action_results: Vec<ActionOutput>,
    pub won_jump_ball: Possession,
    pub starting_at: Tick,
    pub ended_at: Option<Tick>,
    pub possession: Possession,
    pub timer: Timer,
    pub next_step: u16,
    pub current_action: Action,
    pub winner: Option<TeamId>,
    pub home_team_mvps: Option<Vec<GameMVPSummary>>,
    pub away_team_mvps: Option<Vec<GameMVPSummary>>,
}

impl<'game> Game {
    pub fn new(
        id: GameId,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
        starting_at: Tick,
        planet: &Planet,
    ) -> Self {
        let total_reputation = home_team_in_game.reputation + away_team_in_game.reputation;
        let total_population = planet
            .populations
            .iter()
            .map(|(_, population)| population)
            .sum::<u32>();

        let home_name = home_team_in_game.name.clone();
        let away_name = away_team_in_game.name.clone();

        let bonus_attendance = home_team_in_game
            .players
            .iter()
            .map(|(_, player)| {
                if player.special_trait == Some(Trait::Showpirate) {
                    player.reputation.value()
                } else {
                    0
                }
            })
            .sum::<u8>() as f32
            / 100.0
            + away_team_in_game
                .players
                .iter()
                .map(|(_, player)| {
                    if player.special_trait == Some(Trait::Showpirate) {
                        player.reputation.value()
                    } else {
                        0
                    }
                })
                .sum::<u8>() as f32
                / 100.0;

        let mut game = Self {
            id,
            home_team_in_game,
            away_team_in_game,
            location: planet.id,
            attendance: 0,
            starting_at,
            ended_at: None,
            action_results: vec![], // We start from default empty output
            won_jump_ball: Possession::default(),
            possession: Possession::default(),
            timer: Timer::default(),
            next_step: 0,
            current_action: Action::JumpBall,
            winner: None,
            home_team_mvps: None,
            away_team_mvps: None,
        };
        let seed = game.get_rng_seed();
        let mut rng = ChaCha8Rng::from_seed(seed);

        let attendance = (BASE_ATTENDANCE + total_reputation.value() as u32 * total_population)
            as f32
            * rng.gen_range(0.75..1.25)
            * (1.0 + bonus_attendance);
        game.attendance = attendance as u32;
        let mut default_output = ActionOutput::default();
        default_output.description = format!(
            "{} vs {}. Game is about to start here on {}! There are {} people in the stadium.",
            home_name, away_name, planet.name, game.attendance
        );
        default_output.random_seed = seed;
        game.action_results.push(default_output);
        game
    }

    fn player_mvp_summary(&self, player_id: PlayerId) -> Option<GameMVPSummary> {
        let stats = if let Some(s) = self.home_team_in_game.stats.get(&player_id) {
            s
        } else {
            self.away_team_in_game.stats.get(&player_id)?
        };

        let best_stats = vec![
            ("Pts", stats.points, 100.0), //We want points to show as number 1
            (
                "Reb",
                stats.defensive_rebounds + stats.offensive_rebounds,
                1.5,
            ),
            ("Stl", stats.steals, 2.5),
            ("Blk", stats.blocks, 3.0),
            ("Ast", stats.assists, 2.0),
            ("TO", stats.turnovers, -1.5),
            (
                "Acc",
                stats.attempted_2pt - stats.made_2pt + stats.attempted_3pt - stats.made_3pt,
                -0.5,
            ),
        ];

        let score = best_stats
            .iter()
            .map(|(_, s, m)| s.clone() as f32 * m.clone())
            .sum::<f32>() as u32;

        let player = if let Some(p) = self.home_team_in_game.players.get(&player_id) {
            p
        } else {
            self.away_team_in_game.players.get(&player_id)?
        };
        let name = format!(
            "{}. {} ",
            player.info.first_name.chars().next().unwrap_or_default(),
            player.info.last_name,
        );

        Some(GameMVPSummary {
            name,
            score,
            best_stats: best_stats
                .iter()
                .map(|(t, s, m)| {
                    (
                        t.to_string(),
                        s.clone(),
                        (s.clone() as f32 * m.clone()) as u32,
                    )
                })
                .sorted_by(|(_, _, a), (_, _, b)| b.cmp(a))
                .take(3)
                .collect_vec()
                .try_into()
                .ok()?,
        })
    }

    pub fn team_mvps(&self, possession: Possession) -> Vec<GameMVPSummary> {
        let players = match possession {
            Possession::Home => &self.home_team_in_game.players,
            Possession::Away => &self.away_team_in_game.players,
        };
        players
            .keys()
            .map(|&id| self.player_mvp_summary(id).unwrap_or_default())
            .sorted_by(|a, b| b.score.cmp(&a.score))
            .take(3)
            .collect()
    }

    fn pick_action(&self, rng: &mut ChaCha8Rng) -> Action {
        //FIXME: Actions should be picked based on the team tactic/players
        let situation = self.action_results[self.action_results.len() - 1]
            .situation
            .clone();

        match situation {
            ActionSituation::JumpBall => Action::JumpBall,
            ActionSituation::AfterOffensiveRebound => Action::CloseShot,
            ActionSituation::CloseShot => Action::CloseShot,
            ActionSituation::MediumShot => Action::MediumShot,
            ActionSituation::LongShot => Action::LongShot,
            ActionSituation::MissedShot => Action::Rebound,
            ActionSituation::EndOfQuarter => Action::StartOfQuarter,
            ActionSituation::BallInBackcourt => {
                let brawl_probability = BRAWL_ACTION_PROBABILITY
                    * (self.home_team_in_game.tactic.brawl_probability_modifier()
                        + self.away_team_in_game.tactic.brawl_probability_modifier());
                if rng.gen_bool(brawl_probability as f64) {
                    Action::Brawl
                } else {
                    match self.possession {
                        Possession::Home => self
                            .home_team_in_game
                            .tactic
                            .pick_action(rng)
                            .unwrap_or(Action::Isolation),
                        Possession::Away => self
                            .away_team_in_game
                            .tactic
                            .pick_action(rng)
                            .unwrap_or(Action::Isolation),
                    }
                }
            }
            ActionSituation::BallInMidcourt
            | ActionSituation::AfterDefensiveRebound
            | ActionSituation::AfterLongOffensiveRebound
            | ActionSituation::Turnover => match self.possession {
                Possession::Home => self
                    .home_team_in_game
                    .pick_action(rng)
                    .unwrap_or(Action::Isolation),
                Possession::Away => self
                    .away_team_in_game
                    .pick_action(rng)
                    .unwrap_or(Action::Isolation),
            },
        }
    }

    fn apply_game_stats_update(
        &mut self,
        attack_stats: Option<GameStatsMap>,
        defense_stats: Option<GameStatsMap>,
    ) {
        let (home_stats, away_stats) = match self.possession {
            Possession::Home => (attack_stats, defense_stats),
            Possession::Away => (defense_stats, attack_stats),
        };

        if let Some(updates) = home_stats {
            for (id, player_stats) in self.home_team_in_game.stats.iter_mut() {
                if let Some(update) = updates.get(&id) {
                    player_stats.update(update);
                    let player = self.home_team_in_game.players.get_mut(&id).unwrap();
                    player.add_tiredness(update.extra_tiredness);
                }
            }
        }
        if let Some(updates) = away_stats {
            for (id, player_stats) in self.away_team_in_game.stats.iter_mut() {
                if let Some(update) = updates.get(&id) {
                    player_stats.update(update);
                    let player = self.away_team_in_game.players.get_mut(&id).unwrap();
                    player.add_tiredness(update.extra_tiredness);
                }
            }
        }
    }

    fn apply_sub_update(
        &mut self,
        attack_stats: Option<GameStatsMap>,
        defense_stats: Option<GameStatsMap>,
    ) {
        let (home_stats, away_stats) = match self.possession {
            Possession::Home => (attack_stats, defense_stats),
            Possession::Away => (defense_stats, attack_stats),
        };

        if let Some(updates) = home_stats {
            for (id, player_stats) in self.home_team_in_game.stats.iter_mut() {
                if let Some(update) = updates.get(&id) {
                    player_stats.position = update.position;
                }
            }
        }
        if let Some(updates) = away_stats {
            for (id, player_stats) in self.away_team_in_game.stats.iter_mut() {
                if let Some(update) = updates.get(&id) {
                    player_stats.position = update.position;
                }
            }
        }

        assert!(self.home_team_in_game.stats.len() == self.home_team_in_game.players.len());
    }

    fn apply_tiredness_update(&mut self) {
        for team in [&mut self.home_team_in_game, &mut self.away_team_in_game] {
            for (id, player) in team.players.iter_mut() {
                let stats = team.stats.get_mut(&id).unwrap();
                if stats.is_playing() && !self.timer.is_break() {
                    stats.seconds_played += 1;
                    if !player.is_knocked_out() {
                        stats.experience_at_position[stats.position.unwrap() as usize] += 1;
                        player.add_tiredness(TirednessCost::LOW);
                    }
                } else if player.tiredness > RECOVERING_TIREDNESS_PER_SHORT_TICK
                    && !player.is_knocked_out()
                {
                    player.tiredness -= RECOVERING_TIREDNESS_PER_SHORT_TICK;
                }
            }
        }
    }

    pub fn attacking_players(&self) -> Vec<&Player> {
        match self.possession {
            Possession::Home => self
                .home_team_in_game
                .players
                .by_position(&self.home_team_in_game.stats)
                .iter()
                .take(MAX_POSITION as usize)
                .map(|&p| p)
                .collect::<Vec<&Player>>(),
            Possession::Away => self
                .away_team_in_game
                .players
                .by_position(&self.away_team_in_game.stats)
                .iter()
                .take(MAX_POSITION as usize)
                .map(|&p| p)
                .collect::<Vec<&Player>>(),
        }
    }

    pub fn defending_players(&self) -> Vec<&Player> {
        match self.possession {
            Possession::Home => self
                .away_team_in_game
                .players
                .by_position(&self.away_team_in_game.stats)
                .iter()
                .take(5)
                .map(|&p| p)
                .collect::<Vec<&Player>>(),
            Possession::Away => self
                .home_team_in_game
                .players
                .by_position(&self.home_team_in_game.stats)
                .iter()
                .take(5)
                .map(|&p| p)
                .collect::<Vec<&Player>>(),
        }
    }

    pub fn attacking_stats(&self) -> &GameStatsMap {
        match self.possession {
            Possession::Home => &self.home_team_in_game.stats,
            Possession::Away => &self.away_team_in_game.stats,
        }
    }

    pub fn defending_stats(&self) -> &GameStatsMap {
        match self.possession {
            Possession::Home => &self.away_team_in_game.stats,
            Possession::Away => &self.home_team_in_game.stats,
        }
    }

    fn get_rng_seed(&self) -> [u8; 32] {
        let mut seed = [0; 32];
        seed[0..16].copy_from_slice(self.id.as_bytes());
        seed[16..32].copy_from_slice(self.starting_at.to_be_bytes().as_ref());
        // Overwrite first two bytes with timer value
        seed[0..2].copy_from_slice(self.timer.value.to_be_bytes().as_ref());

        seed
    }

    pub fn get_score(&self) -> (u16, u16) {
        if let Some(result) = self.action_results.last() {
            (result.home_score, result.away_score)
        } else {
            (0, 0)
        }
    }

    pub fn is_team_knocked_out(&self, side: Possession) -> bool {
        match side {
            Possession::Home => self
                .home_team_in_game
                .players
                .iter()
                .all(|(_, p)| p.is_knocked_out()),
            Possession::Away => self
                .away_team_in_game
                .players
                .iter()
                .all(|(_, p)| p.is_knocked_out()),
        }
    }

    fn game_end_description(&self, winner: Option<&str>) -> String {
        let (home, away) = self.get_score();
        if let Some(winner_name) = winner {
            let loser_name = if winner_name.to_string() == self.home_team_in_game.name {
                self.away_team_in_game.name.clone()
            } else {
                self.home_team_in_game.name.clone()
            };
            format!(
                "{} won this nice game over {}. The final score is {} {}-{} {}.",
                winner_name,
                loser_name,
                self.home_team_in_game.name,
                home,
                away,
                self.away_team_in_game.name,
            )
        } else {
            format!(
                "It's a tie! The final score is {} {}-{} {}.",
                self.home_team_in_game.name, home, away, self.away_team_in_game.name
            )
        }
    }

    pub fn has_started(&self, timestamp: Tick) -> bool {
        self.starting_at <= timestamp
    }

    pub fn has_ended(&self) -> bool {
        self.ended_at.is_some()
    }

    pub fn tick(&mut self, current_timestamp: Tick) {
        if self.has_ended() {
            return;
        }

        self.timer.tick();

        if self.timer.has_ended() {
            self.ended_at = Some(current_timestamp);
            self.home_team_mvps = Some(self.team_mvps(Possession::Home));
            self.away_team_mvps = Some(self.team_mvps(Possession::Away));

            let description = match self.get_score() {
                (home, away) if home > away => {
                    self.winner = Some(self.home_team_in_game.team_id);
                    self.game_end_description(Some(&self.home_team_in_game.name))
                }
                (home, away) if home < away => {
                    self.winner = Some(self.away_team_in_game.team_id);
                    self.game_end_description(Some(&self.away_team_in_game.name))
                }
                _ => {
                    self.winner = None;
                    self.game_end_description(None)
                }
            };

            self.action_results.push(ActionOutput {
                description,
                start_at: self.timer,
                end_at: self.timer,
                home_score: self.get_score().0,
                away_score: self.get_score().1,
                ..Default::default()
            });

            return;
        }

        self.apply_tiredness_update();

        let seed = self.get_rng_seed();
        let rng = &mut ChaCha8Rng::from_seed(seed);
        let action_input = &self.action_results[self.action_results.len() - 1];

        if !self.timer.reached(self.next_step) {
            return;
        }

        // If next tick is at a break, we are at the end of the quarter and should stop.
        if self.timer.is_break() {
            if let Some(eoq) = EndOfQuarter::execute(action_input, self, rng) {
                self.next_step = eoq.end_at.value;
                self.action_results.push(eoq);
                return;
            }
        }

        self.current_action = self.pick_action(rng);

        if let Some(mut result) = self.current_action.execute(action_input, self, rng) {
            self.apply_game_stats_update(
                result.attack_stats_update.clone(),
                result.defense_stats_update.clone(),
            );

            if result.score_change > 0 {
                let home_plus_minus: i16 = if self.possession == Possession::Home {
                    result.score_change as i16
                } else {
                    -(result.score_change as i16)
                };
                for (_, stats) in self.home_team_in_game.stats.iter_mut() {
                    if stats.is_playing() {
                        stats.plus_minus += home_plus_minus;
                    }
                }
                for (_, stats) in self.away_team_in_game.stats.iter_mut() {
                    if stats.is_playing() {
                        stats.plus_minus -= home_plus_minus;
                    }
                }
                result.description = format!(
                    "{} [{}-{}]",
                    result.description.clone(),
                    result.home_score,
                    result.away_score,
                );
            }

            self.possession = result.possession;

            // If this was the first action (JumpBall),
            // assigns the value of won_jump_ball to possession
            if self.next_step == 0 {
                self.won_jump_ball = self.possession;
            }
            self.next_step = result.end_at.value.min(self.timer.period().next().start());

            self.action_results.push(result);

            let action_input = &self.action_results[self.action_results.len() - 1];
            if action_input.situation == ActionSituation::BallInBackcourt {
                // If home team is completely knocked out, end the game.
                // Check that each player is knocked out
                let home_knocked_out = self.is_team_knocked_out(Possession::Home);
                let away_knocked_out = self.is_team_knocked_out(Possession::Away);

                match (home_knocked_out, away_knocked_out) {
                    (true, true) => {
                        self.ended_at = Some(current_timestamp);
                        self.home_team_mvps = Some(self.team_mvps(Possession::Home));
                        self.away_team_mvps = Some(self.team_mvps(Possession::Away));

                        let description = match self.get_score() {
                            (home, away) if home > away => {
                                self.winner = Some(self.home_team_in_game.team_id);
                                self.game_end_description(Some(&self.home_team_in_game.name))
                            }
                            (home, away) if home < away => {
                                self.winner = Some(self.away_team_in_game.team_id);
                                self.game_end_description(Some(&self.away_team_in_game.name))
                            }
                            _ => {
                                self.winner = None;
                                self.game_end_description(None)
                            }
                        };

                        self.action_results.push(ActionOutput {
                            description: format!(
                    "Both team are completely done! {} They should get some rest now...",
                    description
                ),
                            start_at: self.timer,
                            end_at: self.timer,
                            home_score: self.get_score().0,
                            away_score: self.get_score().1,
                            ..Default::default()
                        });
                    }
                    (true, false) => {
                        self.ended_at = Some(current_timestamp);
                        self.home_team_mvps = Some(self.team_mvps(Possession::Home));
                        self.away_team_mvps = Some(self.team_mvps(Possession::Away));

                        self.winner = Some(self.away_team_in_game.team_id);
                        let description = format!(
                            "The home team is completely wasted and lost! {}",
                            self.game_end_description(Some(&self.away_team_in_game.name))
                        );

                        self.action_results.push(ActionOutput {
                            description,
                            start_at: self.timer,
                            end_at: self.timer,
                            home_score: self.get_score().0,
                            away_score: self.get_score().1,
                            ..Default::default()
                        });
                    }
                    (false, true) => {
                        self.ended_at = Some(current_timestamp);
                        self.home_team_mvps = Some(self.team_mvps(Possession::Home));
                        self.away_team_mvps = Some(self.team_mvps(Possession::Away));

                        self.winner = Some(self.home_team_in_game.team_id);
                        let description = format!(
                            "The away team is completely wasted and lost! {}",
                            self.game_end_description(Some(&self.away_team_in_game.name))
                        );

                        self.action_results.push(ActionOutput {
                            description,
                            start_at: self.timer,
                            end_at: self.timer,
                            home_score: self.get_score().0,
                            away_score: self.get_score().1,
                            ..Default::default()
                        });
                    }
                    _ =>
                    // Check if teams make substitutions. Only if ball is out
                    {
                        if let Some(sub) = Substitution::execute(action_input, self, rng) {
                            self.apply_sub_update(
                                sub.attack_stats_update.clone(),
                                sub.defense_stats_update.clone(),
                            );
                            self.action_results.push(sub);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Game;
    use crate::engine::types::TeamInGame;
    use crate::types::{GameId, IdSystem};
    use crate::types::{SystemTimeTick, Tick};
    use crate::world::constants::DEFAULT_PLANET_ID;
    use crate::world::world::World;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[ignore]
    #[test]
    fn test_game() {
        let mut world = World::new(None);
        let rng = &mut ChaCha8Rng::seed_from_u64(world.seed);

        // world.initialize(true);

        // let home_planet = world.planets.get(&DEFAULT_PLANET_ID).unwrap();
        let id0 = world
            .generate_random_team(
                rng,
                DEFAULT_PLANET_ID.clone(),
                "Testen".to_string(),
                "Tosten".to_string(),
            )
            .unwrap();
        let id1 = world
            .generate_random_team(
                rng,
                DEFAULT_PLANET_ID.clone(),
                "Holalo".to_string(),
                "Halley".to_string(),
            )
            .unwrap();

        let home_team = world.get_team(id0).unwrap().clone();

        let checked_player_id = home_team.player_ids[0];
        let quickness_before = world
            .get_player(checked_player_id)
            .unwrap()
            .athletics
            .quickness
            .clone();

        let home_team_in_game = TeamInGame::from_team_id(id0, &world.teams, &world.players);
        let away_team_in_game = TeamInGame::from_team_id(id1, &world.teams, &world.players);

        let mut game = Game::new(
            GameId::new(),
            home_team_in_game.unwrap(),
            away_team_in_game.unwrap(),
            Tick::now(),
            &world.get_planet(DEFAULT_PLANET_ID.clone()).unwrap(),
        );

        game.home_team_in_game
            .players
            .remove(&home_team.player_ids[1]);
        game.home_team_in_game
            .players
            .remove(&home_team.player_ids[2]);
        game.home_team_in_game
            .players
            .remove(&home_team.player_ids[3]);

        game.home_team_in_game
            .stats
            .remove(&home_team.player_ids[1]);
        game.home_team_in_game
            .stats
            .remove(&home_team.player_ids[2]);
        game.home_team_in_game
            .stats
            .remove(&home_team.player_ids[3]);

        println!("{:?}", game.home_team_in_game.players.len());

        world.games.insert(game.id, game);
        while world.games.len() > 0 {
            let _ = world.handle_tick_events(Tick::now(), false);
        }
        let quickness_after = world
            .get_player(checked_player_id)
            .unwrap()
            .athletics
            .quickness
            .clone();
        println!("{} {}", quickness_before, quickness_after);
    }
}
