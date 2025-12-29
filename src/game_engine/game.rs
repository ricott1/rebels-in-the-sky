use super::{
    action::{Action, ActionOutput, ActionSituation},
    constants::*,
    timer::{Period, Timer},
    types::{GameStatsMap, Possession, TeamInGame},
};
use crate::{
    app_version,
    core::{
        constants::TirednessCost,
        player::{Player, Trait},
        position::MAX_GAME_POSITION,
        skill::GameSkill,
        utils::is_default,
        DEFAULT_PLANET_ID,
    },
    game_engine::{end_of_quarter, substitution},
    types::*,
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
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub is_network: bool,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    app_version: [usize; 3],
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
            is_network: game.is_network(),
            app_version: game.app_version,
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
    pub winner: Option<TeamId>,
    pub home_team_mvps: Option<Vec<GameMVPSummary>>,
    pub away_team_mvps: Option<Vec<GameMVPSummary>>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    app_version: [usize; 3],
}

impl Game {
    pub fn is_network(&self) -> bool {
        self.home_team_in_game.peer_id.is_some() && self.away_team_in_game.peer_id.is_some()
    }

    pub fn test(home_team_in_game: TeamInGame, away_team_in_game: TeamInGame) -> Self {
        Game::new(
            GameId::new_v4(),
            home_team_in_game,
            away_team_in_game,
            Tick::now(),
            DEFAULT_PLANET_ID.clone(),
            0,
            "Test arena",
        )
    }

    pub fn new(
        id: GameId,
        home_team_in_game: TeamInGame,
        away_team_in_game: TeamInGame,
        starting_at: Tick,
        planet_id: PlanetId,
        planet_total_population: u32,
        planet_name: &str,
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
            location: planet_id,
            attendance: 0,
            starting_at,
            ended_at: None,
            action_results: vec![], // We start from default empty output
            won_jump_ball: Possession::default(),
            possession: Possession::default(),
            timer: Timer::default(),
            next_step: 0,
            winner: None,
            home_team_mvps: None,
            away_team_mvps: None,
            app_version: app_version(),
        };
        let seed = game.get_rng_seed();
        let mut rng = ChaCha8Rng::from_seed(seed);

        let attendance = (BASE_ATTENDANCE
            + (total_reputation.value() as u32).pow(2) * planet_total_population)
            as f32
            * rng.random_range(0.75..=1.25)
            * (1.0 + bonus_attendance);
        game.attendance = attendance as u32;
        let mut default_output = ActionOutput::default();

        let opening_text = [
            format!(
                "{} vs {}. The intergalactic showdown is kicking off on {}! {} fans have packed the arena{}.",
                home_name,
                away_name,
                planet_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "It's {} against {}! We're live here on {} where {} spectators{} are buzzing with excitement.",
                home_name,
                away_name,
                planet_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "The stage is set on {} for {} vs {}. A crowd of {}{} fans is ready for the action to unfold!",
                planet_name,
                home_name,
                away_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "{} and {} clash today on {}! An electric atmosphere fills the stadium with {} fans{} watching closely.",
                home_name,
                away_name,
                planet_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "Welcome to {} for an epic battle: {} vs {}. The crowd of {} fans{} is ready to witness greatness!",
                planet_name,
                home_name,
                away_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "Tonight on {}, it's {} taking on {}. With {} passionate fans{} in attendance, the game is about to ignite!",
                planet_name,
                home_name,
                away_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "Game night on {}! {} faces off against {} before {} eager fans{} under the starry skies.",
                planet_name,
                home_name,
                away_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "The rivalry continues on {}: {} vs {}. The crowd of {} fans{} is fired up for this clash!",
                planet_name,
                home_name,
                away_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "All eyes are on {} as {} battles {}. An audience of {}{} is here to cheer for their team!",
                planet_name,
                home_name,
                away_name,
                game.attendance,
                if game.attendance == 69 { " (nice)" } else { "" }
            ),
            format!(
                "Here on {}, it's {} vs {}. A roaring crowd of {} fans{} awaits the start of the showdown!",
                planet_name,
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

    fn pick_action(&self, action_rng: &mut ChaCha8Rng) -> Option<Action> {
        let situation = self.action_results[self.action_results.len() - 1].situation;
        let action = match situation {
            ActionSituation::JumpBall => Action::JumpBall,
            ActionSituation::AfterOffensiveRebound => Action::CloseShot,
            ActionSituation::CloseShot => Action::CloseShot,
            ActionSituation::MediumShot => Action::MediumShot,
            ActionSituation::LongShot => Action::LongShot,
            ActionSituation::ForcedOffTheScreenAction => Action::OffTheScreen,
            ActionSituation::Fastbreak => Action::Fastbreak,
            ActionSituation::MissedShot => Action::Rebound,
            ActionSituation::EndOfQuarter => Action::StartOfQuarter,
            ActionSituation::AfterSubstitution | ActionSituation::BallInBackcourt => {
                let brawl_probability = BRAWL_ACTION_PROBABILITY
                    * (self.home_team_in_game.tactic.brawl_probability_modifier()
                        + self.away_team_in_game.tactic.brawl_probability_modifier());
                if action_rng.random_bool(brawl_probability) {
                    Action::Brawl
                } else {
                    match self.possession {
                        Possession::Home => self.home_team_in_game.pick_action(action_rng)?,
                        Possession::Away => self.away_team_in_game.pick_action(action_rng)?,
                    }
                }
            }
            ActionSituation::BallInMidcourt
            | ActionSituation::AfterDefensiveRebound
            | ActionSituation::AfterLongOffensiveRebound
            | ActionSituation::Turnover => match self.possession {
                Possession::Home => self.home_team_in_game.pick_action(action_rng)?,
                Possession::Away => self.away_team_in_game.pick_action(action_rng)?,
            },
        };

        Some(action)
    }

    fn apply_game_stats_update(
        &mut self,
        attack_stats_update: Option<&GameStatsMap>,
        defense_stats_update: Option<&GameStatsMap>,
        score_change: u16,
    ) {
        let (attacking_team, defending_team) = match self.possession {
            Possession::Home => (&mut self.home_team_in_game, &mut self.away_team_in_game),
            Possession::Away => (&mut self.away_team_in_game, &mut self.home_team_in_game),
        };

        let attacking_stats = &mut attacking_team.stats;
        let attacking_players = &mut attacking_team.players;
        let defending_stats = &mut defending_team.stats;
        let defending_players = &mut defending_team.players;

        for (stats_update, stats, players) in [
            (attack_stats_update, attacking_stats, attacking_players),
            (defense_stats_update, defending_stats, defending_players),
        ] {
            let updates = if let Some(updates) = stats_update {
                updates
            } else {
                continue;
            };

            for (id, player_stats) in stats.iter_mut() {
                let player = players.get_mut(id).expect("Player should exist.");
                if let Some(stats) = updates.get(id) {
                    player_stats.update(stats);
                    player.add_tiredness(stats.extra_tiredness);
                    player.add_morale(stats.extra_morale);
                }
            }
        }

        for stats in defending_stats.values_mut() {
            if stats.is_playing() {
                stats.plus_minus += score_change as i32;
            }
        }
        for stats in defending_stats.values_mut() {
            if stats.is_playing() {
                stats.plus_minus -= score_change as i32;
            }
        }
    }

    fn apply_sub_update(
        &mut self,
        attack_stats_update: Option<&GameStatsMap>,
        defense_stats_update: Option<&GameStatsMap>,
    ) {
        let (home_stats_update, away_stats_update) = match self.possession {
            Possession::Home => (attack_stats_update, defense_stats_update),
            Possession::Away => (defense_stats_update, attack_stats_update),
        };

        if let Some(updates) = home_stats_update {
            for (id, player_stats) in self.home_team_in_game.stats.iter_mut() {
                if let Some(update) = updates.get(id) {
                    player_stats.position = update.position;
                }
            }
        }
        if let Some(updates) = away_stats_update {
            for (id, player_stats) in self.away_team_in_game.stats.iter_mut() {
                if let Some(update) = updates.get(id) {
                    player_stats.position = update.position;
                }
            }
        }
    }

    fn apply_tiredness_update(&mut self) {
        // Apply low generic tiredness to all playing players and recovery for bench players.
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
                        player.add_tiredness(
                            TirednessCost::LOW * team.tactic.playing_tiredness_modifier(),
                        );
                    }
                } else if !player.is_knocked_out() {
                    // We don't use add_tiredness here because otherwise the stamina would have an effect.
                    player.tiredness =
                        (player.tiredness - RECOVERING_TIREDNESS_PER_SHORT_TICK).bound();
                }
            }
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

    fn attacking_stats(&self) -> &GameStatsMap {
        &self.attacking_team().stats
    }

    fn defending_stats(&self) -> &GameStatsMap {
        &self.defending_team().stats
    }

    pub fn all_attacking_players(&self) -> &PlayerMap {
        &self.attacking_team().players
    }

    pub fn all_defending_players(&self) -> &PlayerMap {
        &self.defending_team().players
    }

    pub fn attacking_players_array(&self) -> [&Player; MAX_GAME_POSITION as usize] {
        self.all_attacking_players()
            .by_position(self.attacking_stats())
            .iter()
            .take(MAX_GAME_POSITION as usize)
            .copied()
            .collect_vec()
            .try_into()
            .expect(format!("There should be exactly {} players", MAX_GAME_POSITION).as_str())
    }

    pub fn defending_players_array(&self) -> [&Player; MAX_GAME_POSITION as usize] {
        self.all_defending_players()
            .by_position(self.defending_stats())
            .iter()
            .take(MAX_GAME_POSITION as usize)
            .copied()
            .collect_vec()
            .try_into()
            .expect(format!("There should be exactly {} players", MAX_GAME_POSITION).as_str())
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

    fn game_end_description(&self, winner: Option<Possession>) -> String {
        let (home_score, away_score) = self.get_score();
        match winner {
            Some(Possession::Home) => {
                format!(
                    "{} won this nice game over {}. The final score is {} {}-{} {}.",
                    self.home_team_in_game.name,
                    self.away_team_in_game.name,
                    self.home_team_in_game.name,
                    home_score,
                    away_score,
                    self.away_team_in_game.name,
                )
            }

            Some(Possession::Away) => {
                format!(
                    "{} won this nice game over {}. The final score is {} {}-{} {}.",
                    self.away_team_in_game.name,
                    self.home_team_in_game.name,
                    self.home_team_in_game.name,
                    home_score,
                    away_score,
                    self.away_team_in_game.name,
                )
            }

            None => format!(
                "It's a tie! The final score is {} {}-{} {}.",
                self.home_team_in_game.name, home_score, away_score, self.away_team_in_game.name
            ),
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
                    self.game_end_description(Some(Possession::Home))
                }
                (home, away) if home < away => {
                    self.winner = Some(self.away_team_in_game.team_id);
                    self.game_end_description(Some(Possession::Away))
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

        if !self.timer.reached(self.next_step) {
            return;
        }

        let mut seed = self.get_rng_seed();
        let action_rng = &mut ChaCha8Rng::from_seed(seed);

        // Reverse seed just to get a different rng generator.
        seed.reverse();
        let description_rng = &mut ChaCha8Rng::from_seed(seed);
        let action_input = self.action_results[self.action_results.len() - 1].clone();

        // If next tick is at a break, we are at the end of the quarter and should stop.
        if self.timer.is_break() {
            let eoq = end_of_quarter::execute(&action_input, self, action_rng, description_rng);
            self.next_step = self.timer.period().next().start();
            self.action_results.push(eoq);
            return;
        }

        let mut result = if let Some(action) = self.pick_action(action_rng) {
            action.execute(&action_input, self, action_rng, description_rng)
        }
        // If no action can be selected, switch possession and see what happens
        else {
            ActionOutput {
                situation: ActionSituation::Turnover,
                possession: !self.possession,
                description: format!(
                    "Oh no! {}'s players can't decide what to do and turned the ball over like that!",
                    self.attacking_team().name,
                ),
                start_at: action_input.end_at,
                end_at: action_input.end_at.plus(4 + action_rng.random_range(0..=3)),
                home_score: action_input.home_score,
                away_score: action_input.away_score,
                ..Default::default()
            }
        };

        self.apply_game_stats_update(
            result.attack_stats_update.as_ref(),
            result.defense_stats_update.as_ref(),
            result.score_change,
        );

        if result.score_change > 0 {
            result.description = format!(
                "{} [{}-{}]",
                result.description, result.home_score, result.away_score,
            );
        }

        self.possession = result.possession;

        // If this was the first action (JumpBall),
        // assigns the value of won_jump_ball to possession
        if self.next_step == 0 {
            self.won_jump_ball = self.possession;
        }
        self.next_step = result.end_at.value.min(self.timer.period().next().start());

        let situation = result.situation;
        self.action_results.push(result);

        // If home team is completely knocked out, end the game.
        // Check that each player is knocked out
        let home_knocked_out = self.is_team_knocked_out(Possession::Home);
        let away_knocked_out = self.is_team_knocked_out(Possession::Away);

        match (home_knocked_out, away_knocked_out) {
            (true, true) => {
                self.ended_at = Some(current_tick);
                self.home_team_mvps = Some(self.team_mvps(Possession::Home));
                self.away_team_mvps = Some(self.team_mvps(Possession::Away));
                self.winner = None;

                let description = self.game_end_description(None);

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
                    self.game_end_description(Some(Possession::Away))
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
                    self.game_end_description(Some(Possession::Home))
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
                if situation == ActionSituation::BallInBackcourt {
                    let action_input = self.action_results[self.action_results.len() - 1].clone();
                    if let Some(sub) = substitution::should_execute(
                        &action_input,
                        self,
                        action_rng,
                        description_rng,
                    ) {
                        self.apply_sub_update(
                            sub.attack_stats_update.as_ref(),
                            sub.defense_stats_update.as_ref(),
                        );
                        self.action_results.push(sub);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Game;
    use crate::core::constants::DEFAULT_PLANET_ID;
    use crate::core::world::World;
    use crate::core::{Player, Team, TickInterval, MAX_PLAYERS_PER_GAME};
    use crate::game_engine::action::{ActionSituation, Advantage};
    use crate::game_engine::game::GameSummary;
    use crate::game_engine::types::{GameStatsMap, Possession, TeamInGame};
    use crate::types::{AppResult, PlayerMap, TeamId};
    use crate::types::{SystemTimeTick, Tick};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn generate_team_in_game() -> TeamInGame {
        let team = Team {
            id: TeamId::new_v4(),
            ..Default::default()
        };

        let mut players = PlayerMap::new();
        for _ in 0..MAX_PLAYERS_PER_GAME {
            let player = Player::default().randomize(None);
            players.insert(player.id, player);
        }

        TeamInGame::new(&team, players)
    }

    #[test]
    fn test_game_consistency() -> AppResult<()> {
        let home_team_in_game = generate_team_in_game();
        let away_team_in_game = generate_team_in_game();
        let mut game = Game::test(home_team_in_game, away_team_in_game);

        let mut current_tick = game.starting_at;
        while !game.has_ended() {
            game.tick(current_tick);
            current_tick += TickInterval::SHORT;
        }

        let mut home_score = 0;
        let mut away_score = 0;
        for action in game.action_results.iter() {
            if action.possession == Possession::Away {
                assert!(home_score + action.score_change == action.home_score);
                assert!(away_score == action.away_score);
            } else {
                assert!(home_score == action.home_score);
                assert!(away_score + action.score_change == action.away_score);
            }
            println!(
                "+ {} -> {} - {} ",
                action.score_change, home_score, away_score
            );

            home_score = action.home_score;
            away_score = action.away_score;
        }

        Ok(())
    }

    #[test]
    fn test_game_in_world() -> AppResult<()> {
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

        let home_team_in_game = TeamInGame::from_team_id(&id0, &world.teams, &world.players)?;
        let away_team_in_game = TeamInGame::from_team_id(&id1, &world.teams, &world.players)?;

        let home_rating = world.team_rating(&id0).unwrap_or_default();
        let away_rating = world.team_rating(&id1).unwrap_or_default();

        let game = Game::test(home_team_in_game, away_team_in_game);

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

        let gamer_summary = world.past_games.get(&game_id).unwrap();
        let home_score: u16 = gamer_summary.home_quarters_score.iter().sum();
        let away_score: u16 = gamer_summary.away_quarters_score.iter().sum();

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
            "Advantages: ATK={} NTR={} DEF={}",
            num_attack_advantages, num_no_advantages, num_defense_advantages
        );

        for advantage in [Advantage::Attack, Advantage::Neutral, Advantage::Defense] {
            for situation in [
                ActionSituation::CloseShot,
                ActionSituation::MediumShot,
                ActionSituation::LongShot,
            ] {
                let attempted = game_action_results
                    .iter()
                    .filter(|a| a.advantage == advantage && a.situation == situation)
                    .count();

                let made = game_action_results
                    .iter()
                    .enumerate()
                    .filter(|&(idx, _)| {
                        game_action_results[idx].score_change > 0
                            && game_action_results[idx - 1].advantage == advantage
                            && game_action_results[idx - 1].situation == situation
                    })
                    .count();

                println!(
                    "Advantage {:#?} {:#?} {}/{}",
                    advantage, situation, made, attempted
                );
            }
        }

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
}
