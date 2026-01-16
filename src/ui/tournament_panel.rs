use super::button::Button;
use super::clickable_list::ClickableListState;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::{
    constants::*,
    traits::{Screen, SplitPanel},
    widgets::{default_block, selectable_list},
};
use crate::core::{skill::Rated, world::World};
use crate::game_engine::game::GameSummary;
use crate::game_engine::{Tournament, TournamentId, TournamentState, TournamentSummary};
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::ui::tournament_brackets_lines::{current_round, number_of_rounds};
use crate::ui::{tournament_brackets_lines, ui_key};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::layout::Margin;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    widgets::Paragraph,
};
use std::fmt::Display;

#[derive(Debug, Clone, Copy, Default, PartialEq, Hash)]
pub enum TournamentView {
    #[default]
    All,
    Open,
    Past,
}

impl TournamentView {
    fn next(&self) -> Self {
        match self {
            Self::All => Self::Open,
            Self::Open => Self::Past,
            Self::Past => Self::All,
        }
    }

    fn rule(&self, tournament_id: &TournamentId, world: &World) -> bool {
        match self {
            Self::All => true,
            Self::Open => world
                .tournaments
                .get(tournament_id)
                .map(|t| !t.has_started(Tick::now()))
                .unwrap_or_default(),
            Self::Past => world
                .past_tournaments
                .get(tournament_id)
                .map(|_| true)
                .unwrap_or_default(),
        }
    }
}

impl Display for TournamentView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::Open => write!(f, "Open to registration"),
            Self::Past => write!(f, "Past"),
        }
    }
}

#[derive(Debug, Default)]
pub struct TournamentPanel {
    index: Option<usize>,
    selected_tournament_id: TournamentId,
    tournament_ids: Vec<TournamentId>,
    past_tournament_ids: Vec<TournamentId>,
    all_tournament_ids: Vec<TournamentId>,
    view: TournamentView,
    update_view: bool,
    tick: usize,
}

impl TournamentPanel {
    pub fn new() -> Self {
        Self::default()
    }

    fn build_left_panel(&self, frame: &mut UiFrame, world: &World, area: Rect) {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

        let mut filter_all_button = Button::new(
            TournamentView::All.to_string(),
            UiCallback::SetTournamentPanelView {
                view: TournamentView::All,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View all tournaments.");

        let mut filter_open_button = Button::new(
            TournamentView::Open.to_string(),
            UiCallback::SetTournamentPanelView {
                view: TournamentView::Open,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View all tournaments yet to start.");

        let mut filter_past_button = Button::new(
            TournamentView::Past.to_string(),
            UiCallback::SetTournamentPanelView {
                view: TournamentView::Past,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View all past tournaments.");

        match self.view {
            TournamentView::All => filter_all_button.select(),
            TournamentView::Open => filter_open_button.select(),
            TournamentView::Past => filter_past_button.select(),
        }

        frame.render_interactive_widget(filter_all_button, split[0]);
        frame.render_interactive_widget(filter_open_button, split[1]);
        frame.render_interactive_widget(filter_past_button, split[2]);

        frame.render_widget(default_block().title("Tournaments ↓/↑"), split[3]);

        if self.view == TournamentView::Past {
            self.build_tournament_summary_list(frame, world, split[3].inner(Margin::new(1, 1)));
        } else {
            self.build_tournament_list(frame, world, split[3].inner(Margin::new(1, 1)));
        }
    }

    fn build_tournament_list(&self, frame: &mut UiFrame, world: &World, area: Rect) {
        if !self.tournament_ids.is_empty() {
            let mut options = vec![];
            for tournament_id in self.tournament_ids.iter() {
                let tournament = if let Some(t) = world.tournaments.get(tournament_id) {
                    t
                } else {
                    continue;
                };
                let mut style = UiStyle::DEFAULT;
                if tournament.organizer_id == world.own_team_id {
                    style = UiStyle::OWN_TEAM;
                }

                let text = format!("{:<24} {}", tournament.name(), tournament.stars());
                options.push((text, style));
            }
            let list = selectable_list(options);

            frame.render_stateful_interactive_widget(
                list,
                area,
                &mut ClickableListState::default().with_selected(self.index),
            );
        }
    }

    fn build_tournament_summary_list(&self, frame: &mut UiFrame, world: &World, area: Rect) {
        if !self.past_tournament_ids.is_empty() {
            let mut options = vec![];
            for tournament_id in self.tournament_ids.iter() {
                let tournament = if let Some(t) = world.past_tournaments.get(tournament_id) {
                    t
                } else {
                    continue;
                };
                let mut style = UiStyle::DEFAULT;
                if tournament.organizer_id == world.own_team_id {
                    style = UiStyle::OWN_TEAM;
                }

                let text = format!("{:<24} {}", tournament.name(), tournament.stars());
                options.push((text, style));
            }
            let list = selectable_list(options);

            frame.render_stateful_interactive_widget(
                list,
                area,
                &mut ClickableListState::default().with_selected(self.index),
            );
        }
    }

    fn build_right_panel(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let index = if let Some(index) = self.index {
            index
        } else {
            frame.render_widget(default_block(), area);
            return Ok(());
        };

        let tournament_id = self
            .tournament_ids
            .get(index)
            .expect("Tournament id selection should be valid.");

        let split = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(area);
        let inner = split[1].inner(Margin::new(1, 1));
        if self.view == TournamentView::Past {
            let tournament_summary = if let Some(t) = world.past_tournaments.get(tournament_id) {
                t
            } else {
                frame.render_widget(default_block(), area);
                return Ok(());
            };
            let planet_id = tournament_summary.planet_id;
            let tournament_main_button = Button::new(
                tournament_summary.name(),
                UiCallback::GoToPlanet { planet_id },
            );
            frame.render_interactive_widget(tournament_main_button, split[0]);
            self.render_past_tournament(tournament_summary, frame, world, inner)?;
            return Ok(());
        }

        let tournament = if let Some(t) = world.tournaments.get(tournament_id) {
            t
        } else {
            frame.render_widget(default_block(), area);
            return Ok(());
        };

        let planet_id = tournament.planet_id;
        let tournament_main_button =
            Button::new(tournament.name(), UiCallback::GoToPlanet { planet_id });
        frame.render_interactive_widget(tournament_main_button, split[0]);

        match tournament.state(Tick::now()) {
            TournamentState::Canceled => {}
            TournamentState::Registration => {
                self.render_registration_tournament(tournament, frame, world, inner)?
            }
            TournamentState::Confirmation => {
                self.render_confirmation_tournament(tournament, frame, inner)?
            }
            TournamentState::Syncing => self.render_syncing_tournament(tournament, frame, inner)?,
            TournamentState::Started => {
                if tournament.is_initialized() {
                    self.render_started_tournament(tournament, frame, world, inner)?
                }
            }
            TournamentState::Ended => {}
        }

        Ok(())
    }

    fn render_registration_tournament(
        &self,
        tournament: &Tournament,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let t_split = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(area);
        let countdown = (tournament
            .registrations_closing_at
            .saturating_sub(Tick::now()))
        .formatted();
        frame.render_widget(
            Paragraph::new(format!(
                "Registrations closing in {} - Max participants {}",
                countdown, tournament.max_participants
            ))
            .centered()
            .block(default_block()),
            t_split[0],
        );

        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
            .split(t_split[1]);

        let options = tournament
            .registered_teams
            .values()
            .sorted_by(|a, b| a.team_id.cmp(&b.team_id))
            .map(|team| {
                let mut style = UiStyle::DEFAULT;
                if team.team_id == world.own_team_id {
                    style = UiStyle::OWN_TEAM;
                } else if team.peer_id.is_some() {
                    style = UiStyle::NETWORK;
                }
                let text = format!("{:<MAX_NAME_LENGTH$} {}", team.name, team.rating().stars());
                (text, style)
            })
            .collect_vec();

        let list = selectable_list(options);

        frame.render_stateful_interactive_widget(
            list.block(default_block().title("Registered crews ↓/↑")),
            split[0],
            &mut ClickableListState::default().with_selected(None),
        );

        let own_team = world.get_own_team()?;

        if tournament.organizer_id == world.own_team_id {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from("Thanks for organizing!"),
                    Line::from("Be sure to online when registrations close"),
                    Line::from("or the tournament will be canceled."),
                ])
                .centered(),
                split[1],
            );
        } else {
            let b_split =
                Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(split[1]);
            let mut register_button = Button::new(
                "Register now!",
                UiCallback::RegisterToTournament {
                    tournament_id: tournament.id,
                },
            )
            .set_hotkey(ui_key::REGISTER_TO_TOURNAMENT)
            .set_hover_text(format!(
                "Register to {}. Participation will be confirmed on {} at {}.",
                tournament.name(),
                tournament.registrations_closing_at.formatted_as_date(),
                tournament.registrations_closing_at.formatted_as_time(),
            ));

            if let Err(err) = own_team.can_register_to_tournament(tournament, Tick::now()) {
                register_button.disable(Some(err.to_string()));
                if tournament.is_team_registered(&world.own_team_id) {
                    register_button.set_text("Already registered");
                }
            }

            frame.render_interactive_widget(register_button, b_split[0]);

            frame.render_widget(
                Paragraph::new(vec![
                Line::from("Register at any time and be sure to be online at the time registrations close"),
                Line::from("to be able to confirm your participation."),
                Line::from(format!("{} participants will be chosen at random from all the registered crews", tournament.max_participants)),
                Line::from("that confirmed their participation.")]).centered(),
                b_split[1],
            );
        }

        Ok(())
    }

    fn render_confirmation_tournament(
        &self,
        tournament: &Tournament,
        frame: &mut UiFrame,
        area: Rect,
    ) -> AppResult<()> {
        let t_split = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(area);
        frame.render_widget(
            Paragraph::new("Selecting participants...")
                .centered()
                .block(default_block()),
            t_split[0],
        );

        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
            .split(t_split[1]);

        let options = tournament
            .registered_teams
            .values()
            .sorted_by(|a, b| a.team_id.cmp(&b.team_id))
            .map(|team| {
                let style = if tournament.participants.contains_key(&team.team_id) {
                    UiStyle::OK
                } else {
                    UiStyle::WARNING
                };
                let text = format!("{:<MAX_NAME_LENGTH$} {}", team.name, team.rating().stars());
                (text, style)
            })
            .collect_vec();

        let list = selectable_list(options);

        frame.render_stateful_interactive_widget(
            list.block(default_block().title("Registered crews ↓/↑")),
            split[0],
            &mut ClickableListState::default().with_selected(None),
        );
        Ok(())
    }

    fn render_syncing_tournament(
        &self,
        tournament: &Tournament,
        frame: &mut UiFrame,
        area: Rect,
    ) -> AppResult<()> {
        let t_split = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(area);
        frame.render_widget(
            Paragraph::new("Syncing...")
                .centered()
                .block(default_block()),
            t_split[0],
        );

        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
            .split(t_split[1]);

        let options = tournament
            .registered_teams
            .values()
            .sorted_by(|a, b| a.team_id.cmp(&b.team_id))
            .map(|team| {
                let style = if tournament.participants.contains_key(&team.team_id) {
                    UiStyle::OK
                } else {
                    UiStyle::ERROR
                };
                let text = format!("{:<MAX_NAME_LENGTH$} {}", team.name, team.rating().stars());
                (text, style)
            })
            .collect_vec();

        let list = selectable_list(options);

        frame.render_stateful_interactive_widget(
            list.block(default_block().title("Registered crews ↓/↑")),
            split[0],
            &mut ClickableListState::default().with_selected(None),
        );
        Ok(())
    }

    fn render_started_tournament(
        &self,
        tournament: &Tournament,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let t_split = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(area);

        let active_games = tournament.active_games(&world.games);
        let past_game_summaries = tournament.past_game_summaries(&world.past_games);
        let number_of_rounds = number_of_rounds(tournament.participants.len());
        let current_round = current_round(
            tournament.participants.len(),
            past_game_summaries.len() + active_games.len(),
        ) + 1;

        frame.render_widget(
            Paragraph::new(format!(
                "Currently playing round {current_round}/{number_of_rounds}"
            ))
            .centered()
            .block(default_block()),
            t_split[0],
        );

        let brackets_split =
            Layout::horizontal([Constraint::Length(24)].repeat(number_of_rounds + 1))
                .split(t_split[1]);

        let brackets = tournament_brackets_lines::get_bracket_lines(
            tournament.winner.map(|id| {
                tournament
                    .participants
                    .get(&id)
                    .expect("Winner should be a participant")
                    .name
                    .clone()
            }),
            tournament.participants.len(),
            &active_games,
            &past_game_summaries,
            world.own_team_id,
            Tick::now(),
        );
        for (round_idx, lines) in brackets.iter().enumerate() {
            frame.render_widget(Paragraph::new(lines.clone()), brackets_split[round_idx]);
        }

        Ok(())
    }

    fn render_past_tournament(
        &self,
        tournament_summary: &TournamentSummary,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let t_split = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(area);
        let ended_at = tournament_summary
            .ended_at
            .expect("Tournament should be ended.");
        frame.render_widget(
            Paragraph::new(format!(
                "Ended on {} at {}",
                ended_at.formatted_as_date(),
                ended_at.formatted_as_time()
            ))
            .centered()
            .block(default_block()),
            t_split[0],
        );

        let games = vec![];
        let game_summaries = tournament_summary
            .game_ids
            .iter()
            .filter_map(|id| world.past_games.get(id))
            .collect::<Vec<&GameSummary>>();

        let number_of_rounds = number_of_rounds(tournament_summary.participants.len());

        let brackets_split =
            Layout::horizontal([Constraint::Length(24)].repeat(number_of_rounds + 1))
                .split(t_split[1]);

        let brackets = tournament_brackets_lines::get_bracket_lines(
            tournament_summary.winner.map(|id| {
                tournament_summary
                    .participants
                    .get(&id)
                    .expect("Winner should be a participant")
                    .name
                    .clone()
            }),
            tournament_summary.participants.len(),
            &games,
            &game_summaries,
            world.own_team_id,
            Tick::now(),
        );
        for (round_idx, lines) in brackets.iter().enumerate() {
            frame.render_widget(Paragraph::new(lines.clone()), brackets_split[round_idx]);
        }

        Ok(())
    }

    pub fn set_view(&mut self, filter: TournamentView) {
        self.view = filter;
        self.update_view = true;
    }

    pub fn reset_view(&mut self) {
        self.set_view(TournamentView::All);
    }
}

impl Screen for TournamentPanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;

        if world.dirty_ui || self.all_tournament_ids.len() != world.tournaments.len() {
            self.all_tournament_ids = world.tournaments.keys().copied().collect();
            self.past_tournament_ids = world.past_tournaments.keys().copied().collect();

            self.update_view = true;
        }

        if self.update_view {
            self.tournament_ids = if self.view == TournamentView::Past {
                self.past_tournament_ids.iter().copied().collect()
            } else {
                self.all_tournament_ids
                    .iter()
                    .filter(|&id| self.view.rule(id, world))
                    .copied()
                    .collect()
            };

            self.update_view = false;
        }

        if let Some(index) = self.index {
            if self.tournament_ids.is_empty() {
                self.index = None;
            } else if index >= self.tournament_ids.len() && !self.tournament_ids.is_empty() {
                self.set_index(self.tournament_ids.len() - 1);
            }

            if index < self.tournament_ids.len() {
                self.selected_tournament_id = self.tournament_ids[index];
            }
        } else if !self.tournament_ids.is_empty() {
            self.set_index(0);
        }

        Ok(())
    }
    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,

        _debug_view: bool,
    ) -> AppResult<()> {
        // Split into left and right panels
        let left_right_split =
            Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
                .split(area);
        self.build_left_panel(frame, world, left_right_split[0]);

        if self.tournament_ids.is_empty() {
            frame.render_widget(
                default_block().title(" No tournaments at the moment..."),
                left_right_split[1],
            );
            return Ok(());
        }
        self.build_right_panel(frame, world, left_right_split[1])?;
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            ui_key::CYCLE_VIEW => {
                return Some(UiCallback::SetTournamentPanelView {
                    view: self.view.next(),
                });
            }

            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" {} ", ui_key::CYCLE_VIEW.to_string()),
            " Next tab ".to_string(),
            format!(
                " {}/{} ",
                ui_key::PREVIOUS_SELECTION.to_string(),
                ui_key::NEXT_SELECTION.to_string()
            ),
            " Select player ".to_string(),
        ]
    }
}

impl SplitPanel for TournamentPanel {
    fn index(&self) -> Option<usize> {
        self.index
    }

    fn max_index(&self) -> usize {
        self.tournament_ids.len()
    }

    fn set_index(&mut self, index: usize) {
        if self.max_index() == 0 {
            self.index = None;
        } else {
            self.index = Some(index % self.max_index());
        }
    }
}
