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
        constants::{MoraleModifier, TirednessCost},
        planet::Planet,
        player::{Player, Trait},
        position::MAX_POSITION,
        role::CrewRole,
        skill::{GameSkill, MAX_SKILL},
    },
};
use itertools::Itertools;
use rand::{seq::IndexedRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameSummary {
    pub id: GameId,
    pub home_team_id: TeamId,
    pub away_team_id: TeamId,
    pub home_team_name: String,
    pub away_team_name: String,
    #[serde(default)]
    pub home_team_knocked_out: bool,
    #[serde(default)]
    pub away_team_knocked_out: bool,
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
            // We need to loop over every action to cover the case in which the game ends abrutly because one team is knocked out.
            // For quarters>1, we need to remove previous quarters score to get only the partial score of the quarter.
            match action.start_at.period() {
                Period::Q1 => {
                    home_quarters_score[0] = action.home_score;
                    away_quarters_score[0] = action.away_score;
                }
                Period::Q2 => {
                    home_quarters_score[1] = action.home_score - home_quarters_score[0];
                    away_quarters_score[1] = action.away_score - away_quarters_score[0];
                }
                Period::Q3 => {
                    home_quarters_score[2] =
                        action.home_score - home_quarters_score[0] - home_quarters_score[1];
                    away_quarters_score[2] =
                        action.away_score - away_quarters_score[0] - away_quarters_score[1];
                }
                Period::Q4 => {
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
            home_team_knocked_out: game.is_team_knocked_out(Possession::Home),
            away_team_knocked_out: game.is_team_knocked_out(Possession::Away),
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

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameMVPSummary {
    pub name: String,
    pub score: u32,
    pub best_stats: [(String, u16, u32); 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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
    next_step: u16,
    pub current_action: Action,
    pub winner: Option<TeamId>,
    pub home_team_mvps: Option<Vec<GameMVPSummary>>,
    pub away_team_mvps: Option<Vec<GameMVPSummary>>,
}

impl<'game> Game {
    pub fn is_network(&self) -> bool {
        self.home_team_in_game.peer_id.is_some() && self.away_team_in_game.peer_id.is_some()
    }

    pub fn new(
        id: GameId,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
        starting_at: Tick,
        planet: &Planet,
    ) -> Self {
        let total_reputation = home_team_in_game.reputation + away_team_in_game.reputation;
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

        let attendance = (BASE_ATTENDANCE as f32
            + (total_reputation.value() as f32).powf(2.0) * planet.total_population() as f32)
            * rng.random_range(0.75..1.25)
            * (1.0 + bonus_attendance);
        game.attendance = attendance as u32;
        let mut default_output = ActionOutput::default();

        let opening_text = [
    format!(
        "{} vs {}. The intergalactic showdown is kicking off on {}! {} fans have packed the arena{}.",
        home_name,
        away_name,
        planet.name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "It's {} against {}! We're live here on {} where {} spectators{} are buzzing with excitement.",
        home_name,
        away_name,
        planet.name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "The stage is set on {} for {} vs {}. A crowd of {}{} fans is ready for the action to unfold!",
        planet.name,
        home_name,
        away_name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "{} and {} clash today on {}! An electric atmosphere fills the stadium with {} fans{} watching closely.",
        home_name,
        away_name,
        planet.name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "Welcome to {} for an epic battle: {} vs {}. The crowd of {} fans{} is ready to witness greatness!",
        planet.name,
        home_name,
        away_name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "Tonight on {}, it's {} taking on {}. With {} passionate fans{} in attendance, the game is about to ignite!",
        planet.name,
        home_name,
        away_name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "Game night on {}! {} faces off against {} before {} eager fans{} under the starry skies.",
        planet.name,
        home_name,
        away_name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "The rivalry continues on {}: {} vs {}. The crowd of {} fans{} is fired up for this clash!",
        planet.name,
        home_name,
        away_name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "All eyes are on {} as {} battles {}. An audience of {}{} is here to cheer for their team!",
        planet.name,
        home_name,
        away_name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
    format!(
        "Here on {}, it's {} vs {}. A roaring crowd of {} fans{} awaits the start of the showdown!",
        planet.name,
        home_name,
        away_name,
        game.attendance,
        if game.attendance == 69 { " (nice)" } else { "" }
    ),
].choose(&mut rng).expect("There should be one option").clone();

        default_output.description = opening_text;
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
            (
                "Brw",
                if stats.brawls[0] > stats.brawls[1] {
                    stats.brawls[0] - stats.brawls[1]
                } else {
                    stats.brawls[1] - stats.brawls[0]
                },
                if stats.brawls[0] > stats.brawls[1] {
                    3.0
                } else {
                    -3.0
                },
            ),
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
        let name = player.info.short_name();

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
            ActionSituation::AfterSubstitution | ActionSituation::BallInBackcourt => {
                let brawl_probability = BRAWL_ACTION_PROBABILITY
                    * (self.home_team_in_game.tactic.brawl_probability_modifier()
                        + self.away_team_in_game.tactic.brawl_probability_modifier());
                if rng.random_bool(brawl_probability as f64) {
                    Action::Brawl
                } else {
                    match self.possession {
                        Possession::Home => self
                            .home_team_in_game
                            .pick_action(rng)
                            .unwrap_or(Action::Isolation),
                        Possession::Away => self
                            .away_team_in_game
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
        score_change: u16,
    ) {
        let (mut home_stats, mut away_stats) = match self.possession {
            Possession::Home => (attack_stats, defense_stats),
            Possession::Away => (defense_stats, attack_stats),
        };

        // Conditions for morale boost:
        // shot success, team is losing at most by a margin equal to the captain charisma.
        let attacking_player = self.attacking_players();
        let team_captain = attacking_player
            .iter()
            .find(|&p| p.info.crew_role == CrewRole::Captain);

        let mut losing_margin = 4;

        if let Some(captain) = team_captain {
            losing_margin += (captain.mental.charisma / 4.0) as u16
        };

        let score = self.get_score();

        let is_losing_by_margin = match self.possession {
            Possession::Home => score.0 < score.1 && score.1 - score.0 <= losing_margin,
            Possession::Away => score.1 < score.0 && score.0 - score.1 <= losing_margin,
        };

        if let Some(updates) = &mut home_stats {
            for (id, player_stats) in self.home_team_in_game.stats.iter_mut() {
                let player = self.home_team_in_game.players.get_mut(&id).unwrap();
                if let Some(stats) = updates.get_mut(&id) {
                    player_stats.update(stats);
                    player.add_tiredness(stats.extra_tiredness);
                    player.add_morale(stats.extra_morale);
                }
                // Add morale if team scored
                if score_change > 0 {
                    if self.possession == Possession::Home {
                        player.add_morale(MoraleModifier::SMALL_BONUS);
                    } else {
                        player.add_morale(
                            MoraleModifier::SMALL_MALUS
                                / (1.0 + player.mental.charisma / MAX_SKILL),
                        );
                    }

                    if is_losing_by_margin {
                        player
                            .add_morale(MoraleModifier::SMALL_BONUS + player.mental.charisma / 8.0);
                    }
                }
            }
        }
        if let Some(updates) = &mut away_stats {
            for (id, player_stats) in self.away_team_in_game.stats.iter_mut() {
                let player = self.away_team_in_game.players.get_mut(&id).unwrap();

                if let Some(stats) = updates.get_mut(&id) {
                    player_stats.update(stats);
                    player.add_tiredness(stats.extra_tiredness);
                    player.add_morale(stats.extra_morale);
                }
                // Add morale if team scored
                if score_change > 0 {
                    if self.possession == Possession::Away {
                        player.add_morale(MoraleModifier::SMALL_BONUS);
                    } else {
                        player.add_morale(
                            MoraleModifier::SMALL_MALUS
                                / (1.0 + player.mental.charisma / MAX_SKILL),
                        );
                    }

                    if is_losing_by_margin {
                        player
                            .add_morale(MoraleModifier::SMALL_BONUS + player.mental.charisma / 8.0);
                    }
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
                let stats = team.stats.get_mut(&id).expect("Player should have stats");
                if stats.is_playing() && !self.timer.is_break() {
                    stats.seconds_played += 1;
                    if !player.is_knocked_out() {
                        stats.experience_at_position[stats
                            .position
                            .expect("Playing player should have a position")
                            as usize] += 1;
                        player.add_tiredness(TirednessCost::LOW);
                    }
                } else if player.tiredness > RECOVERING_TIREDNESS_PER_SHORT_TICK
                    && !player.is_knocked_out()
                {
                    // We don't use add_tiredness here because otherwise the stamina would have an effect.
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

    pub fn attacking_team(&self) -> &TeamInGame {
        match self.possession {
            Possession::Home => &self.home_team_in_game,
            Possession::Away => &self.away_team_in_game,
        }
    }

    pub fn defending_team(&self) -> &TeamInGame {
        match self.possession {
            Possession::Home => &self.away_team_in_game,
            Possession::Away => &self.home_team_in_game,
        }
    }

    fn get_rng_seed(&self) -> [u8; 32] {
        let mut seed = [0; 32];
        seed[0..16].copy_from_slice(self.id.as_bytes());
        seed[16..24].copy_from_slice(self.starting_at.to_be_bytes().as_ref());
        seed[24..26].copy_from_slice(self.timer.value.to_be_bytes().as_ref());

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

    pub fn tick(&mut self, current_tick: Tick) {
        if self.has_ended() {
            return;
        }

        self.timer.tick();

        if self.timer.has_ended() {
            self.ended_at = Some(current_tick);
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
                self.next_step = self.timer.period().next().start();
                self.action_results.push(eoq);
                return;
            }
        }

        self.current_action = self.pick_action(rng);

        if let Some(mut result) = self.current_action.execute(action_input, self, rng) {
            self.apply_game_stats_update(
                result.attack_stats_update.clone(),
                result.defense_stats_update.clone(),
                result.score_change,
            );

            if result.score_change > 0 {
                let home_plus_minus = if self.possession == Possession::Home {
                    result.score_change as i32
                } else {
                    -(result.score_change as i32)
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
                        self.ended_at = Some(current_tick);
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
                        self.ended_at = Some(current_tick);
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
                        self.ended_at = Some(current_tick);
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
                    (false, false) =>
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
    use crate::game_engine::types::TeamInGame;
    use crate::types::{AppResult, GameId};
    use crate::types::{SystemTimeTick, Tick};
    use crate::world::constants::DEFAULT_PLANET_ID;
    use crate::world::world::World;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[ignore]
    #[test]
    fn test_game() -> AppResult<()> {
        let mut world = World::new(None);
        let rng = &mut ChaCha8Rng::seed_from_u64(world.seed);

        let id0 = world.generate_random_team(
            rng,
            DEFAULT_PLANET_ID.clone(),
            "Testen".to_string(),
            "Tosten".to_string(),
        )?;
        let id1 = world.generate_random_team(
            rng,
            DEFAULT_PLANET_ID.clone(),
            "Holalo".to_string(),
            "Halley".to_string(),
        )?;

        let home_team = world.get_team(&id0).unwrap().clone();

        let checked_player_id = home_team.player_ids[0];
        let quickness_before = world
            .get_player_or_err(&checked_player_id)?
            .athletics
            .quickness
            .clone();

        let home_team_in_game = TeamInGame::from_team_id(&id0, &world.teams, &world.players);
        let away_team_in_game = TeamInGame::from_team_id(&id1, &world.teams, &world.players);

        let mut game = Game::new(
            GameId::new_v4(),
            home_team_in_game.unwrap(),
            away_team_in_game.unwrap(),
            Tick::now(),
            &world.get_planet(&DEFAULT_PLANET_ID).unwrap(),
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
            let _ = world.handle_tick_events(Tick::now());
        }
        let quickness_after = world
            .get_player_or_err(&checked_player_id)?
            .athletics
            .quickness
            .clone();
        println!("{} {}", quickness_before, quickness_after);

        Ok(())
    }
}
