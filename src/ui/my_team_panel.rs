use super::{
    button::Button,
    clickable_table::{ClickableCell, ClickableRow, ClickableTable, ClickableTableState},
    constants::{PrintableKeyCode, UiKey, UiStyle},
    gif_map::GifMap,
    traits::{Screen, SplitPanel, StyledRating},
    ui_callback::{CallbackRegistry, UiCallbackPreset},
    widgets::{default_block, render_player_description, render_spaceship_description},
};
use crate::{
    image::spaceship::SPACESHIP_IMAGE_HEIGHT,
    types::{SystemTimeTick, TeamId},
    world::{
        constants::{BASE_BONUS, BONUS_PER_SKILL},
        role::CrewRole,
        skill::GameSkill,
    },
};
use crate::{
    image::spaceship::SPACESHIP_IMAGE_WIDTH,
    types::{AppResult, PlayerId},
    world::{
        position::{GamePosition, Position, MAX_POSITION},
        skill::Rated,
        types::TeamLocation,
        world::World,
    },
};
use core::fmt::Debug;
use ratatui::{
    layout::Margin,
    prelude::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::{sync::Arc, sync::Mutex};

#[derive(Debug, Default)]
pub struct MyTeamPanel {
    pub index: usize,
    players: Vec<PlayerId>,
    own_team_id: TeamId,
    tick: usize,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    gif_map: Arc<Mutex<GifMap>>,
}

impl MyTeamPanel {
    pub fn new(
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        gif_map: Arc<Mutex<GifMap>>,
    ) -> Self {
        Self {
            callback_registry,
            gif_map,
            ..Default::default()
        }
    }

    fn build_players_table(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let team = world.get_own_team().unwrap();

        let header_cells = [" Name", "Training", "Current", "Best", "Role", "Crew bonus"]
            .iter()
            .map(|h| ClickableCell::from(*h).style(UiStyle::HEADER));
        let header = ClickableRow::new(header_cells);
        let rows = self
            .players
            .iter()
            .map(|&id| {
                let player = world.get_player(id).unwrap();
                let skills = player.current_skill_array();
                let training_focus = if player.training_focus.is_none() {
                    "General".to_string()
                } else {
                    player.training_focus.unwrap().to_string()
                };

                let current_role = match team.player_ids.iter().position(|id| *id == player.id) {
                    Some(idx) => format!(
                        "{:<2} {:<5}",
                        (idx as Position).as_str(),
                        if (idx as Position) < MAX_POSITION {
                            (idx as Position).player_rating(skills).stars()
                        } else {
                            "".to_string()
                        }
                    ),
                    None => "Free agent".to_string(),
                };
                let best_role = Position::best(skills);

                let bonus_string = match player.info.crew_role {
                    CrewRole::Pilot => {
                        let bonus = world.spaceship_speed_bonus(team)?;
                        let fitness = ((bonus - BASE_BONUS) / BONUS_PER_SKILL).bound();
                        let style = fitness.style();

                        Span::styled(format!("Ship speed x{:.2}", bonus), style)
                    }
                    CrewRole::Captain => {
                        let bonus = world.team_reputation_bonus(team)?;
                        let fitness = ((bonus - BASE_BONUS) / BONUS_PER_SKILL).bound();
                        let style = fitness.style();
                        Span::styled(format!("Reputation x{:.2}", bonus), style)
                    }
                    CrewRole::Doctor => {
                        let bonus = world.tiredness_recovery_bonus(team)?;
                        let fitness = ((bonus - BASE_BONUS) / BONUS_PER_SKILL).bound();
                        let style = fitness.style();
                        Span::styled(format!("Recovery   x{:.2}", bonus), style)
                    }
                    _ => Span::raw(""),
                };

                let cells = [
                    ClickableCell::from(format!(
                        " {} {}",
                        player.info.first_name, player.info.last_name
                    )),
                    ClickableCell::from(training_focus.to_string()),
                    ClickableCell::from(current_role),
                    ClickableCell::from(format!(
                        "{:<2} {:<5}",
                        best_role.as_str(),
                        best_role.player_rating(skills).stars()
                    )),
                    ClickableCell::from(player.info.crew_role.to_string()),
                    ClickableCell::from(bonus_string),
                ];
                Ok(ClickableRow::new(cells))
            })
            .collect::<AppResult<Vec<ClickableRow>>>();
        let table = ClickableTable::new(rows?, Arc::clone(&self.callback_registry))
            .header(header)
            .hovering_style(UiStyle::HIGHLIGHT)
            .highlight_style(UiStyle::SELECTED)
            .widths(&[
                Constraint::Length(26),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(18),
            ]);

        frame.render_stateful_widget(
            table.block(default_block().title(format!("{} ↓/↑", team.name.clone()))),
            area,
            &mut ClickableTableState::default().with_selected(Some(self.index)),
        );
        Ok(())
    }
}

impl Screen for MyTeamPanel {
    fn name(&self) -> &str {
        "My Team"
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        self.own_team_id = world.own_team_id;

        if self.players.len() < world.players.len() || world.dirty_ui {
            let own_team = world.get_own_team().unwrap();
            self.players = own_team.player_ids.clone();
            self.players.sort_by(|a, b| {
                let a = world.get_player(*a).unwrap();
                let b = world.get_player(*b).unwrap();
                if a.rating() == b.rating() {
                    b.total_skills().cmp(&a.total_skills())
                } else {
                    b.rating().cmp(&a.rating())
                }
            });
        }
        self.index = self.index % self.players.len();
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;

        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(24), Constraint::Min(8)])
            .split(area);

        let top_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(60)])
            .split(split[0]);

        self.build_players_table(frame, world, top_split[0])?;

        let player_id = self.players[self.index];
        let player = world
            .get_player(player_id)
            .ok_or(format!("Player {:?} not found", player_id).to_string())?;

        render_player_description(player, &self.gif_map, self.tick, frame, world, top_split[1]);

        let table_bottom = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(3), //role buttons
                Constraint::Length(3), //buttons
                Constraint::Length(1), //margin box
            ])
            .split(split[0]);

        let position_button_splits = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(3),  //margin
                Constraint::Length(32), //auto-assign
                Constraint::Length(32), //tactic
                Constraint::Min(0),
            ])
            .split(table_bottom[1].inner(&Margin {
                vertical: 0,
                horizontal: 1,
            }));

        for idx in 0..MAX_POSITION as usize {
            let position = idx as Position;
            let rect = position_button_splits[idx];
            let mut button = Button::new(
                format!(
                    "{}:{:>2}",
                    (idx + 1).to_string(),
                    position.as_str().to_string()
                ),
                UiCallbackPreset::SwapPlayerPositions {
                    player_id,
                    position: idx,
                },
                Arc::clone(&self.callback_registry),
            );
            let position = team.player_ids.iter().position(|id| *id == player.id);
            if position.is_some() && position.unwrap() == idx {
                button.disable(None);
                button = button
                    .set_box_style(UiStyle::OK)
                    .set_hover_style(UiStyle::OK);
            }
            frame.render_widget(button, rect);
        }
        let auto_assign_button = Button::new(
            format!("{}: Auto-assign positions", UiKey::AUTO_ASSIGN.to_string()),
            UiCallbackPreset::AssignBestTeamPositions,
            Arc::clone(&self.callback_registry),
        );
        frame.render_widget(auto_assign_button, position_button_splits[6]);

        let offense_tactic_button = Button::new(
            format!("{}: {}", UiKey::SET_TACTIC.to_string(), team.game_tactic),
            UiCallbackPreset::SetTeamTactic {
                tactic: team.game_tactic.next(),
            },
            Arc::clone(&self.callback_registry),
        );
        frame.render_widget(offense_tactic_button, position_button_splits[7]);

        let button_splits = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(11),
                Constraint::Length(11),
                Constraint::Length(11),
                Constraint::Length(32),
                Constraint::Length(32),
                Constraint::Min(1),
            ])
            .split(table_bottom[2].inner(&Margin {
                vertical: 0,
                horizontal: 1,
            }));

        let can_set_as_captain = team.can_set_crew_role(&player, CrewRole::Captain);
        let mut captain_button = Button::new(
            format!("{}:Captain", UiKey::SET_CAPTAIN.to_string(),),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Captain,
            },
            Arc::clone(&self.callback_registry),
        );
        if can_set_as_captain.is_err() {
            captain_button.disable(None);
        }
        frame.render_widget(captain_button, button_splits[0]);

        let can_set_as_pilot = team.can_set_crew_role(&player, CrewRole::Pilot);
        let mut pilot_button = Button::new(
            format!("{}:Pilot", UiKey::SET_PILOT.to_string(),),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Pilot,
            },
            Arc::clone(&self.callback_registry),
        );
        if can_set_as_pilot.is_err() {
            pilot_button.disable(None);
        }
        frame.render_widget(pilot_button, button_splits[1]);

        let can_set_as_doctor = team.can_set_crew_role(&player, CrewRole::Doctor);
        let mut doctor_button = Button::new(
            format!("{}:Doctor", UiKey::SET_DOCTOR.to_string(),),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Doctor,
            },
            Arc::clone(&self.callback_registry),
        );
        if can_set_as_doctor.is_err() {
            doctor_button.disable(None);
        }
        frame.render_widget(doctor_button, button_splits[2]);

        let can_release = team.can_release_player(&player);
        let mut release_button = Button::new(
            format!(
                "{}: Release {}.{}",
                UiKey::HIRE_FIRE.to_string(),
                player.info.first_name.chars().next().unwrap_or_default(),
                player.info.last_name
            ),
            UiCallbackPreset::ReleasePlayer { player_id },
            Arc::clone(&self.callback_registry),
        );
        if can_release.is_err() {
            release_button.disable(Some(format!(
                "{}: {}",
                UiKey::HIRE_FIRE.to_string(),
                can_release.unwrap_err().to_string()
            )));
        }

        frame.render_widget(release_button, button_splits[3]);

        let can_change_training_focus = team.can_change_training_focus();
        let mut training_button = Button::new(
            format!("{}: Set training focus", UiKey::TRAINING_FOCUS.to_string()),
            UiCallbackPreset::NextTrainingFocus { player_id },
            Arc::clone(&self.callback_registry),
        );
        if can_change_training_focus.is_err() {
            training_button.disable(Some(format!(
                "{}: {}",
                UiKey::TRAINING_FOCUS.to_string(),
                can_change_training_focus.unwrap_err().to_string()
            )));
        }
        frame.render_widget(training_button, button_splits[4]);

        let bottom_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2 + 30),
                Constraint::Length(48),
                Constraint::Min(0),
            ])
            .split(split[1]);

        render_spaceship_description(
            &team,
            &self.gif_map,
            self.tick,
            world,
            frame,
            bottom_split[0],
        );

        let travel_button_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(SPACESHIP_IMAGE_HEIGHT as u16 / 2 + 2),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(bottom_split[0]);

        let travel_button = match team.current_location {
            TeamLocation::OnPlanet { planet_id } => Button::new(
                format!(
                    "{}: On planet {}",
                    UiKey::GO_TO_PLANET.to_string(),
                    world.get_planet_or_err(planet_id)?.name
                ),
                UiCallbackPreset::GoToCurrentTeamPlanet { team_id: team.id },
                Arc::clone(&self.callback_registry),
            ),
            TeamLocation::Travelling {
                from: _from,
                to,
                started,
                duration,
            } => {
                let to = world.get_planet_or_err(to)?.name.to_string();
                let text = if started + duration > world.last_tick_short_interval {
                    (started + duration - world.last_tick_short_interval).formatted()
                } else {
                    "landing".into()
                };
                let mut button = Button::new(
                    format!("Travelling to {} {}", to, text,),
                    UiCallbackPreset::None,
                    Arc::clone(&self.callback_registry),
                );
                button.disable(None);
                button
            }
        };
        frame.render_widget(
            travel_button,
            travel_button_split[1].inner(&Margin {
                vertical: 0,
                horizontal: 1,
            }),
        );

        let mut lines = vec![];
        if team.current_game.is_some() {
            if let Some(game) = world.games.get(&team.current_game.unwrap()) {
                if let Some(action) = game.action_results.last() {
                    lines.push(Line::from(format!(
                        " {:>12} {:>3}-{:<3} {:<} {}",
                        game.home_team_in_game.name,
                        action.home_score,
                        action.away_score,
                        game.away_team_in_game.name,
                        game.timer.format()
                    )));
                }
            }
        }

        for game in world.past_games.values() {
            lines.push(Line::from(format!(
                " {:>12} {:>3}-{:<3} {:<}",
                game.home_team_name, game.home_score, game.away_score, game.away_team_name,
            )));
        }
        frame.render_widget(
            Paragraph::new(lines).block(default_block().title("Recent Games".to_string())),
            bottom_split[1],
        );

        frame.render_widget(
            default_block().title("Future stuff".to_string()),
            bottom_split[2],
        );

        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
    ) -> Option<UiCallbackPreset> {
        if self.players.is_empty() {
            return None;
        }
        let player_id = self.players[self.index];
        match key_event.code {
            crossterm::event::KeyCode::Up => {
                self.next_index();
            }
            crossterm::event::KeyCode::Down => {
                self.previous_index();
            }

            UiKey::AUTO_ASSIGN => {
                return Some(UiCallbackPreset::AssignBestTeamPositions);
            }

            UiKey::SET_TACTIC => {
                return Some(UiCallbackPreset::SetNextTeamTactic);
            }

            UiKey::HIRE_FIRE => {
                return Some(UiCallbackPreset::ReleasePlayer { player_id });
            }

            UiKey::SET_CAPTAIN => {
                return Some(UiCallbackPreset::SetCrewRole {
                    player_id,
                    role: CrewRole::Captain,
                });
            }

            UiKey::SET_DOCTOR => {
                return Some(UiCallbackPreset::SetCrewRole {
                    player_id,
                    role: CrewRole::Doctor,
                });
            }

            UiKey::SET_PILOT => {
                return Some(UiCallbackPreset::SetCrewRole {
                    player_id,
                    role: CrewRole::Pilot,
                });
            }

            UiKey::GO_TO_PLANET => {
                return Some(UiCallbackPreset::GoToCurrentTeamPlanet {
                    team_id: self.own_team_id,
                });
            }

            crossterm::event::KeyCode::Char('1') => {
                return Some(UiCallbackPreset::SwapPlayerPositions {
                    player_id,
                    position: 0,
                });
            }
            crossterm::event::KeyCode::Char('2') => {
                return Some(UiCallbackPreset::SwapPlayerPositions {
                    player_id,
                    position: 1,
                });
            }
            crossterm::event::KeyCode::Char('3') => {
                return Some(UiCallbackPreset::SwapPlayerPositions {
                    player_id,
                    position: 2,
                });
            }
            crossterm::event::KeyCode::Char('4') => {
                return Some(UiCallbackPreset::SwapPlayerPositions {
                    player_id,
                    position: 3,
                });
            }
            crossterm::event::KeyCode::Char('5') => {
                return Some(UiCallbackPreset::SwapPlayerPositions {
                    player_id,
                    position: 4,
                });
            }

            UiKey::TRAINING_FOCUS => {
                return Some(UiCallbackPreset::NextTrainingFocus { player_id });
            }
            _ => {}
        }

        None
    }

    fn footer_spans(&self) -> Vec<Span> {
        vec![]
    }
}

impl SplitPanel for MyTeamPanel {
    fn index(&self) -> usize {
        self.index
    }

    fn max_index(&self) -> usize {
        self.players.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}
