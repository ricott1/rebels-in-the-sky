use super::button::Button;
use super::clickable_list::ClickableListState;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::{
    constants::*,
    traits::{Screen, SplitPanel},
    widgets::{default_block, selectable_list},
};
use crate::core::team::Team;
use crate::core::Skill;
use crate::game_engine::Tournament;
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::ui::ui_key;
use crate::{
    core::{skill::Rated, world::World},
    types::TeamId,
};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use ratatui::layout::Margin;
use ratatui::style::Stylize;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    widgets::Paragraph,
};
use std::fmt::Display;

const IMG_FRAME_WIDTH: u16 = 80;

#[derive(Debug, Clone, Copy, Default, PartialEq, Hash)]
pub enum TournamentView {
    #[default]
    All,
    Open,
}

impl TournamentView {
    fn next(&self) -> Self {
        match self {
            Self::All => Self::Open,
            Self::Open => Self::All,
        }
    }

    fn rule(&self, tournament: &Tournament, own_team: &Team, team_rating: Skill) -> bool {
        match self {
            Self::All => true,
            Self::Open => own_team
                .can_register_to_tournament(tournament, team_rating)
                .is_ok(),
        }
    }
}

impl Display for TournamentView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::Open => write!(f, "Open to registration"),
        }
    }
}

#[derive(Debug, Default)]
pub struct TournamentPanel {
    pub index: Option<usize>,
    pub selected_tournament_id: TeamId,
    pub tournament_ids: Vec<TeamId>,
    pub all_tournament_ids: Vec<TeamId>,
    view: TournamentView,
    update_view: bool,
    tick: usize,
}

impl TournamentPanel {
    pub fn new() -> Self {
        Self::default()
    }

    fn build_left_panel(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let split = Layout::vertical([
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
        .set_hover_text("View all tournaments to which the team can register.");

        match self.view {
            TournamentView::All => filter_all_button.select(),
            TournamentView::Open => filter_open_button.select(),
        }

        frame.render_interactive_widget(filter_all_button, split[0]);
        frame.render_interactive_widget(filter_open_button, split[1]);

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

                let text = format!(
                    "{:<MAX_NAME_LENGTH$} {}",
                    tournament.planet_name,
                    world
                        .tournament_rating(&tournament.id)
                        .unwrap_or_default()
                        .stars()
                );
                options.push((text, style));
            }
            let list = selectable_list(options);

            frame.render_stateful_interactive_widget(
                list.block(default_block().title("Tournaments ↓/↑")),
                split[2],
                &mut ClickableListState::default().with_selected(self.index),
            );
        } else {
            frame.render_widget(default_block().title("Tournaments"), split[2]);
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

        let tournament = if let Some(t) = world.tournaments.get(tournament_id) {
            t
        } else {
            frame.render_widget(default_block(), area);
            return Ok(());
        };

        if !tournament.has_started(Tick::now()) {
            self.render_started_tournament(tournament, frame, world, area)?;
        } else if !tournament.has_ended() {
            self.render_started_tournament(tournament, frame, world, area)?;
        } else {
            self.render_started_tournament(tournament, frame, world, area)?;
        }

        Ok(())
    }

    fn render_started_tournament(
        &self,
        tournament: &Tournament,
        frame: &mut UiFrame,
        _world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

        frame.render_widget(
            Paragraph::new(tournament.name()).centered().bold(),
            split[0],
        );

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
        log::info!(
            "update {} and {}",
            world.tournaments.len(),
            self.all_tournament_ids.len()
        );
        if world.dirty_ui || self.all_tournament_ids.len() != world.tournaments.len() {
            self.all_tournament_ids = world.tournaments.keys().copied().collect();
            self.all_tournament_ids.sort_by(|a, b| {
                world
                    .tournament_rating(b)
                    .unwrap_or_default()
                    .partial_cmp(&world.tournament_rating(a).unwrap_or_default())
                    .expect("Rating should exists")
            });
            self.update_view = true;
        }

        if self.update_view {
            let own_team = world.get_own_team()?;
            let team_rating = world.team_rating(&world.own_team_id)?;

            self.tournament_ids = self
                .all_tournament_ids
                .iter()
                .filter(|&id| {
                    let tournament = world.tournaments.get(id).unwrap();
                    self.view.rule(tournament, own_team, team_rating)
                })
                .copied()
                .collect();
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
        let left_right_split = Layout::horizontal([
            Constraint::Length(LEFT_PANEL_WIDTH),
            Constraint::Min(IMG_FRAME_WIDTH),
        ])
        .split(area);
        self.build_left_panel(frame, world, left_right_split[0]);

        if self.all_tournament_ids.is_empty() {
            frame.render_widget(
                Paragraph::new(" No tournaments yet!"),
                left_right_split[1].inner(Margin::new(1, 1)),
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
        self.index = Some(index);
    }
}
