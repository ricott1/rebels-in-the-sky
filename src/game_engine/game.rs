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
    #[serde(default)]
    pub is_network: bool,
}

impl GameSummary {
    pub fn from_game(game: &Game) -> GameSummary {
        let mut home_quarters_score = [0_u16; 4];
        let mut away_quarters_score = [0_u16; 4];
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
            id: game.id,
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
            is_network: game.home_team_in_game.peer_id.is_some()
                || game.away_team_in_game.peer_id.is_some(),
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
            .values()
            .map(|player| {
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
                .values()
                .map(|player| {
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
        let mut action_rng = ChaCha8Rng::from_seed(seed);

        let attendance = (BASE_ATTENDANCE as f32
            + (total_reputation.value() as f32).powf(2.0) * planet.total_population() as f32)
            * action_rng.random_range(0.75..1.25)
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
].choose(&mut action_rng).expect("There should be one option").clone();

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

        let best_stats = [
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
                stats.brawls[0].abs_diff(stats.brawls[1]),
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
            .map(|(_, s, m)| *s as f32 * *m)
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
                .map(|(t, s, m)| (t.to_string(), *s, (*s as f32 * *m) as u32))
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

    fn pick_action(&self, action_rng: &mut ChaCha8Rng) -> Action {
        let situation = self.action_results[self.action_results.len() - 1].situation;

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
                if action_rng.random_bool(brawl_probability as f64) {
                    Action::Brawl
                } else {
                    match self.possession {
                        Possession::Home => self
                            .home_team_in_game
                            .pick_action(action_rng)
                            .unwrap_or(Action::Isolation),
                        Possession::Away => self
                            .away_team_in_game
                            .pick_action(action_rng)
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
                    .pick_action(action_rng)
                    .unwrap_or(Action::Isolation),
                Possession::Away => self
                    .away_team_in_game
                    .pick_action(action_rng)
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
                let player = self.home_team_in_game.players.get_mut(id).unwrap();
                if let Some(stats) = updates.get_mut(id) {
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
                let player = self.away_team_in_game.players.get_mut(id).unwrap();

                if let Some(stats) = updates.get_mut(id) {
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
                if let Some(update) = updates.get(id) {
                    player_stats.position = update.position;
                }
            }
        }
        if let Some(updates) = away_stats {
            for (id, player_stats) in self.away_team_in_game.stats.iter_mut() {
                if let Some(update) = updates.get(id) {
                    player_stats.position = update.position;
                }
            }
        }

        assert!(self.home_team_in_game.stats.len() == self.home_team_in_game.players.len());
    }

    fn apply_tiredness_update(&mut self) {
        for team in [&mut self.home_team_in_game, &mut self.away_team_in_game] {
            for (id, player) in team.players.iter_mut() {
                let stats = team.stats.get_mut(id).expect("Player should have stats");
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
                .copied()
                .collect::<Vec<&Player>>(),
            Possession::Away => self
                .away_team_in_game
                .players
                .by_position(&self.away_team_in_game.stats)
                .iter()
                .take(MAX_POSITION as usize)
                .copied()
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
                .copied()
                .collect::<Vec<&Player>>(),
            Possession::Away => self
                .home_team_in_game
                .players
                .by_position(&self.home_team_in_game.stats)
                .iter()
                .take(5)
                .copied()
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
            let loser_name = if winner_name == self.home_team_in_game.name {
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

        let mut seed = self.get_rng_seed();
        let action_rng = &mut ChaCha8Rng::from_seed(seed);

        // Reverse seed just to get a different rng generator.
        seed.reverse();
        let description_rng = &mut ChaCha8Rng::from_seed(seed);
        let action_input = &self.action_results[self.action_results.len() - 1];

        if !self.timer.reached(self.next_step) {
            return;
        }

        // If next tick is at a break, we are at the end of the quarter and should stop.
        if self.timer.is_break() {
            if let Some(eoq) =
                EndOfQuarter::execute(action_input, self, action_rng, description_rng)
            {
                self.next_step = self.timer.period().next().start();
                self.action_results.push(eoq);
                return;
            }
        }

        self.current_action = self.pick_action(action_rng);

        if let Some(mut result) =
            self.current_action
                .execute(action_input, self, action_rng, description_rng)
        {
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
                    "Both team are completely done! {description} They should get some rest now..."
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
                        if let Some(sub) =
                            Substitution::execute(action_input, self, action_rng, description_rng)
                        {
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
    use crate::game_engine::action::Advantage;
    use crate::game_engine::constants::NUMBER_OF_ROLLS;
    use crate::game_engine::game::GameSummary;
    use crate::game_engine::types::{GameStats, GameStatsMap, TeamInGame};
    use crate::types::{AppResult, GameId};
    use crate::types::{SystemTimeTick, Tick};
    use crate::world::constants::DEFAULT_PLANET_ID;
    use crate::world::world::World;
    use itertools::Itertools;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use std::collections::{BTreeMap, HashMap};

    #[test]
    fn test_game() -> AppResult<()> {
        let mut world = World::new(None);
        let action_rng = &mut ChaCha8Rng::seed_from_u64(world.seed);
        let id0 = world.generate_random_team(
            action_rng,
            DEFAULT_PLANET_ID.clone(),
            "Testen".to_string(),
            "Tosten".to_string(),
            Some(0.0),
        )?;
        let id1 = world.generate_random_team(
            action_rng,
            DEFAULT_PLANET_ID.clone(),
            "Holalo".to_string(),
            "Halley".to_string(),
            Some(0.0),
        )?;

        let home_team_in_game = TeamInGame::from_team_id(&id0, &world.teams, &world.players);
        let away_team_in_game = TeamInGame::from_team_id(&id1, &world.teams, &world.players);

        let home_rating = world.team_rating(&id0).unwrap_or_default();
        let away_rating = world.team_rating(&id1).unwrap_or_default();

        let game = Game::new(
            GameId::new_v4(),
            home_team_in_game.unwrap(),
            away_team_in_game.unwrap(),
            Tick::now(),
            &world.get_planet(&DEFAULT_PLANET_ID).unwrap(),
        );

        let game_id = game.id;
        let home_tactic = game.home_team_in_game.tactic;
        let away_tactic = game.away_team_in_game.tactic;

        world.games.insert(game.id, game);

        // Call parts of the internal handle_tick loop by hand, to make the simulation go faster.
        let mut game_action_results = vec![];
        let mut game_stats = GameStatsMap::new();
        while world.games.len() > 0 {
            let current_tick = Tick::now();
            for game in world.games.values_mut() {
                if game.has_started(current_tick) && !game.has_ended() {
                    game.tick(current_tick);
                }

                if game.has_ended() {
                    let game_summary = GameSummary::from_game(&game);
                    game_action_results = game.action_results.clone();
                    game_stats = game.home_team_in_game.stats.clone();
                    game_stats.extend(game.away_team_in_game.stats.clone());
                    world.past_games.insert(game_summary.id, game_summary);
                }
            }

            world.games.retain(|_, g| !g.has_ended());
        }

        let result = world.past_games.get(&game_id).unwrap();
        let home_score: u16 = result.home_quarters_score.iter().sum();
        let away_score: u16 = result.away_quarters_score.iter().sum();

        println!(
            "{:.2} vs {:.2} --> {}:{}\n{} -- {}",
            home_rating, away_rating, home_score, away_score, home_tactic, away_tactic
        );

        let num_attack_advantages = game_action_results
            .iter()
            .filter(|a| a.advantage == Advantage::Attack)
            .count();

        let num_no_advantages = game_action_results
            .iter()
            .filter(|a| a.advantage == Advantage::Neutral)
            .count();

        let num_defense_advantages = game_action_results
            .iter()
            .filter(|a| a.advantage == Advantage::Defense)
            .count();

        println!(
            "Advantages: {} {} {}",
            num_attack_advantages, num_no_advantages, num_defense_advantages
        );

        let num_offensive_rebounds: u16 = game_stats
            .iter()
            .map(|(_, stat)| stat.offensive_rebounds)
            .sum();

        let num_defense_rebounds: u16 = game_stats
            .iter()
            .map(|(_, stat)| stat.defensive_rebounds)
            .sum();
        println!(
            "Rebounds: off:{} def:{}",
            num_offensive_rebounds, num_defense_rebounds,
        );

        let attempted_2pt: u16 = game_stats.iter().map(|(_, stat)| stat.attempted_2pt).sum();
        let made_2pt: u16 = game_stats.iter().map(|(_, stat)| stat.made_2pt).sum();
        let attempted_3pt: u16 = game_stats.iter().map(|(_, stat)| stat.attempted_3pt).sum();
        let made_3pt: u16 = game_stats.iter().map(|(_, stat)| stat.made_3pt).sum();

        println!(
            "Shots: 2pt:{}/{} 3pt:{}/{}",
            made_2pt, attempted_2pt, made_3pt, attempted_3pt
        );

        Ok(())
    }

    /// Sum the provided selector over a team's GameStatsMap.
    /// `stats` is a GameStatsMap (player_id -> GameStats).
    fn team_stat_sum<F>(stats: &GameStatsMap, selector: F) -> f32
    where
        F: Fn(&GameStats) -> f32,
    {
        stats.values().map(|stat| selector(stat)).sum()
    }

    fn get_simulated_game_stats(
        world: &mut World,
        action_rng: &mut ChaCha8Rng,
        n_games: usize,
    ) -> AppResult<Vec<(f32, GameStatsMap, GameStatsMap)>> {
        let mut samples = Vec::with_capacity(n_games);
        const DELTA: f32 = 6.0;
        for i in 0..n_games {
            let id0 = world.generate_random_team(
                action_rng,
                DEFAULT_PLANET_ID.clone(),
                "Testen".to_string(),
                "Tosten".to_string(),
                Some(DELTA * i as f32 / n_games as f32),
            )?;
            let id1 = world.generate_random_team(
                action_rng,
                DEFAULT_PLANET_ID.clone(),
                "Holalo".to_string(),
                "Halley".to_string(),
                Some(-DELTA * i as f32 / n_games as f32),
            )?;

            let home_team_in_game = TeamInGame::from_team_id(&id0, &world.teams, &world.players);
            let away_team_in_game = TeamInGame::from_team_id(&id1, &world.teams, &world.players);

            let home_rating = world.team_rating(&id0).unwrap_or_default();
            let away_rating = world.team_rating(&id1).unwrap_or_default();

            let mut game = Game::new(
                GameId::new_v4(),
                home_team_in_game.unwrap(),
                away_team_in_game.unwrap(),
                Tick::now(),
                &world.get_planet(&DEFAULT_PLANET_ID).unwrap(),
            );

            // Simulate until finished
            while !game.has_ended() {
                let current_tick = Tick::now();
                if game.has_started(current_tick) {
                    game.tick(current_tick);
                }
            }

            // Reorder so home team is always higher rated one.
            if home_rating >= away_rating {
                samples.push((
                    home_rating - away_rating,
                    game.home_team_in_game.stats.clone(),
                    game.away_team_in_game.stats.clone(),
                ));
            } else {
                samples.push((
                    away_rating - home_rating,
                    game.away_team_in_game.stats.clone(),
                    game.home_team_in_game.stats.clone(),
                ));
            }
        }

        Ok(samples)
    }

    /// Returns (mean, stddev, count) per bin_center (int)
    fn compute_binned_stats<F>(
        samples: &Vec<(f32, GameStatsMap, GameStatsMap)>, // (rating_diff, home_stas, away_stats)
        bin_size: f32,
        selectors: Vec<F>,
    ) -> BTreeMap<i32, ((Vec<f32>, Vec<f32>), (Vec<f32>, Vec<f32>), usize)>
    where
        F: Fn(&GameStats) -> f32,
    {
        // First pass: sum and count for each selector
        let default_entry = (
            vec![0.0f32].repeat(selectors.len()), // away avg/stddev for each selector
            vec![0.0f32].repeat(selectors.len()), // home avg/stddev for each selector
            0usize,
        );
        let mut sums: BTreeMap<i32, (Vec<f32>, Vec<f32>, usize)> = BTreeMap::new();
        for (rating_diff, home_stats, away_stats) in samples {
            let bin = ((*rating_diff) / bin_size).round() as i32;
            let entry = sums.entry(bin).or_insert(default_entry.clone());
            for (idx, selector) in selectors.iter().enumerate() {
                entry.0[idx] += team_stat_sum(home_stats, selector);
                entry.1[idx] += team_stat_sum(away_stats, selector);
            }
            entry.2 += 1;
        }

        // Means
        let mut means: BTreeMap<i32, (Vec<f32>, Vec<f32>)> = BTreeMap::new();
        for (bin, (home_sums, away_sums, count)) in &sums {
            let home_means = (0..selectors.len())
                .map(|idx| home_sums[idx] / *count as f32)
                .collect();

            let away_means = (0..selectors.len())
                .map(|idx| away_sums[idx] / *count as f32)
                .collect();

            means.insert(*bin, (home_means, away_means));
        }

        // Second pass: sum squared deviations
        let default_entry = (
            vec![0.0f32].repeat(selectors.len()), // away avg/stddev for each selector
            vec![0.0f32].repeat(selectors.len()), // home avg/stddev for each selector
        );
        let mut sqdevs: BTreeMap<i32, (Vec<f32>, Vec<f32>)> = BTreeMap::new();
        for (rating_diff, home_stats, away_stats) in samples {
            let bin = ((*rating_diff) / bin_size).round() as i32;
            let (home_means, away_means) = means[&bin].clone();
            let entry = sqdevs.entry(bin).or_insert(default_entry.clone());
            for (idx, selector) in selectors.iter().enumerate() {
                entry.0[idx] += (team_stat_sum(home_stats, selector) - home_means[idx]).powi(2);
                entry.1[idx] += (team_stat_sum(away_stats, selector) - away_means[idx]).powi(2);
            }
        }

        // Final assembly: compute sample variance (N-1), stddev, and count
        let mut out = BTreeMap::new();
        for (bin, (_, _, count)) in sums {
            let (home_means, away_means) = means[&bin].clone();
            let (home_ss, away_ss) = sqdevs.get(&bin).unwrap();
            let home_variances = home_ss
                .iter()
                .map(|s| {
                    if count > 1 {
                        (s / (count as f32 - 1.0)).sqrt()
                    } else {
                        0.0
                    }
                })
                .collect_vec();
            let away_variances = away_ss
                .iter()
                .map(|s| {
                    if count > 1 {
                        (s / (count as f32 - 1.0)).sqrt()
                    } else {
                        0.0
                    }
                })
                .collect_vec();

            let bin_center = (bin as f32 * bin_size) as i32;
            out.insert(
                bin_center,
                (
                    (home_means, home_variances),
                    (away_means, away_variances),
                    count,
                ),
            );
        }
        out
    }

    #[ignore]
    #[test]
    fn test_multiple_games() -> AppResult<()> {
        // Example usage: compute stat for made_2pt (you can change to any selector)
        let mut world = World::new(None);
        let mut action_rng = ChaCha8Rng::seed_from_u64(world.seed);

        const N: usize = 10_000;
        let samples = get_simulated_game_stats(&mut world, &mut action_rng, N)?;
        let bin_size = 1.0;

        let point_selector = |s: &GameStats| 2.0 * s.made_2pt as f32 + 3.0 * s.made_3pt as f32;
        let win_samples = samples
            .iter()
            .filter(|(_, home_stats, away_stats)| {
                team_stat_sum(home_stats, point_selector)
                    > team_stat_sum(away_stats, point_selector)
            })
            .map(|(rating_diff, _, _)| ((*rating_diff) / bin_size).round() as i32)
            .collect_vec();

        let mut win_counts = HashMap::new();
        for bin in win_samples {
            *win_counts.entry(bin).or_insert(0) += 1;
        }

        println!("N={}", NUMBER_OF_ROLLS);

        let selectors = vec![
            |s: &GameStats| 2.0 * s.made_2pt as f32 + 3.0 * s.made_3pt as f32, // points
            |s: &GameStats| s.made_2pt as f32,
            |s: &GameStats| s.attempted_2pt as f32,
            |s: &GameStats| s.made_3pt as f32,
            |s: &GameStats| s.attempted_3pt as f32,
            |s: &GameStats| s.defensive_rebounds as f32,
            |s: &GameStats| s.offensive_rebounds as f32,
            |s: &GameStats| s.assists as f32,
            |s: &GameStats| s.turnovers as f32,
            |s: &GameStats| s.steals as f32,
            |s: &GameStats| s.blocks as f32,
            |s: &GameStats| s.brawls[0] as f32 + 0.5 * s.brawls[1] as f32,
        ];
        let bin_stats = compute_binned_stats(&samples, bin_size, selectors);

        for (bin_center, ((home_avg, home_stddev), (away_avg, away_stddev), count)) in bin_stats {
            println!("Δrating={:+2} ({} samples)", bin_center, count);

            let bin_win_counts = win_counts.get(&bin_center).copied().unwrap_or_default();

            // The following formulas are not exact cause we consider draws a loss.
            println!(
                "  Win% = {:3.1} ± {:3.1} ({}/{})",
                100.0 * (bin_win_counts + 1) as f32 / (count + 2) as f32,
                100.0
                    * (((bin_win_counts + 1) * (count - bin_win_counts + 1)) as f32
                        / ((count + 2).pow(2) * (count + 3)) as f32)
                        .sqrt(),
                bin_win_counts,
                count
            );
            println!(
                "  points = {:3.1} ± {:3.1} vs {:3.1} ± {:3.1}",
                home_avg[0], home_stddev[0], away_avg[0], away_stddev[0],
            );
            println!(
                "  2pt = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                home_avg[1],
                home_avg[2],
                home_stddev[1],
                home_stddev[2],
                away_avg[1],
                away_avg[2],
                away_stddev[1],
                away_stddev[2],
            );
            println!(
                "  3pt = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                home_avg[3],
                home_avg[4],
                home_stddev[3],
                home_stddev[4],
                away_avg[3],
                away_avg[4],
                away_stddev[3],
                away_stddev[4],
            );

            println!(
                "  Def/Off Rebounds = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                home_avg[5],
                home_avg[6],
                home_stddev[5],
                home_stddev[6],
                away_avg[5],
                away_avg[6],
                away_stddev[5],
                away_stddev[6],
            );

            println!(
                "  Assists/Turnovers = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                home_avg[7],
                home_avg[8],
                home_stddev[7],
                home_stddev[8],
                away_avg[7],
                away_avg[8],
                away_stddev[7],
                away_stddev[8],
            );

            println!(
                "  Steals/Blocks = {:3.1}/{:3.1} ± {:3.1}/{:3.1} vs {:3.1}/{:3.1} ± {:3.1}/{:3.1}",
                home_avg[9],
                home_avg[10],
                home_stddev[9],
                home_stddev[10],
                away_avg[9],
                away_avg[10],
                away_stddev[9],
                away_stddev[10],
            );

            println!(
                "  Brawls = {:3.1} ± {:3.1} vs {:3.1} ± {:3.1}",
                home_avg[11], home_stddev[11], away_avg[11], away_stddev[11],
            );

            println!("");
        }

        Ok(())
    }
}
