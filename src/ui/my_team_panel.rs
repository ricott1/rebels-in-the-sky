use super::{
    button::Button,
    clickable_table::{ClickableCell, ClickableRow, ClickableTable, ClickableTableState},
    constants::{PrintableKeyCode, UiKey, UiStyle},
    gif_map::GifMap,
    traits::{Screen, SplitPanel, StyledRating},
    ui_callback::{CallbackRegistry, UiCallbackPreset},
    utils::hover_text_target,
    widgets::{
        default_block, explore_button, go_to_team_planet_button, render_player_description,
        render_spaceship_description, trade_button,
    },
};
use crate::{
    image::spaceship::SPACESHIP_IMAGE_HEIGHT,
    types::{PlanetId, TeamId},
    world::{
        constants::{BASE_BONUS, BONUS_PER_SKILL},
        resources::Resource,
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
    prelude::{Constraint, Layout, Rect},
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
    current_planet_id: Option<PlanetId>,
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

        self.current_planet_id = match world.get_own_team()?.current_location {
            TeamLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        };

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

        let split = Layout::vertical([Constraint::Length(24), Constraint::Min(8)]).split(area);

        let top_split =
            Layout::horizontal([Constraint::Min(10), Constraint::Length(60)]).split(split[0]);

        self.build_players_table(frame, world, top_split[0])?;

        let player_id = self.players[self.index];
        let player = world
            .get_player(player_id)
            .ok_or(format!("Player {:?} not found", player_id).to_string())?;

        render_player_description(
            player,
            &self.gif_map,
            &self.callback_registry,
            self.tick,
            frame,
            world,
            top_split[1],
        );

        let table_bottom = Layout::vertical([
            Constraint::Min(10),
            Constraint::Length(3), //role buttons
            Constraint::Length(3), //buttons
            Constraint::Length(1), //margin box
        ])
        .split(split[0]);

        let position_button_splits = Layout::horizontal([
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

        let button_splits = Layout::horizontal([
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

        let bottom_split = Layout::horizontal([
            Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2 + 28),
            Constraint::Length(44),
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

        let right_split = Layout::vertical([
            Constraint::Length(SPACESHIP_IMAGE_HEIGHT as u16 / 2 + 2),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(bottom_split[0]);
        let hover_text_target = hover_text_target(frame);
        match team.current_location {
            TeamLocation::OnPlanet { .. } => {
                let button_split =
                    Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(
                        right_split[1].inner(&Margin {
                            vertical: 0,
                            horizontal: 1,
                        }),
                    );

                if let Ok(go_to_team_planet_button) = go_to_team_planet_button(
                    world,
                    team,
                    &self.callback_registry,
                    hover_text_target,
                ) {
                    frame.render_widget(go_to_team_planet_button, button_split[0]);
                }

                if let Ok(explore_button) =
                    explore_button(world, team, &self.callback_registry, hover_text_target)
                {
                    frame.render_widget(explore_button, button_split[1]);
                }
            }
            TeamLocation::Travelling { .. } => {
                if let Ok(go_to_team_planet_button) = go_to_team_planet_button(
                    world,
                    team,
                    &self.callback_registry,
                    hover_text_target,
                ) {
                    frame.render_widget(
                        go_to_team_planet_button,
                        right_split[1].inner(&Margin {
                            vertical: 0,
                            horizontal: 1,
                        }),
                    );
                }
            }
            TeamLocation::Exploring { .. } => {
                if let Ok(explore_button) =
                    explore_button(world, team, &self.callback_registry, hover_text_target)
                {
                    frame.render_widget(
                        explore_button,
                        right_split[1].inner(&Margin {
                            vertical: 0,
                            horizontal: 1,
                        }),
                    );
                }
            }
        }

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

        match team.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                let planet = world.get_planet_or_err(planet_id)?;
                frame.render_widget(default_block().title("Market"), bottom_split[2]);

                let button_split = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(bottom_split[2].inner(&Margin {
                    horizontal: 1,
                    vertical: 1,
                }));

                let buy_ui_keys = [
                    UiKey::BUY_FOOD,
                    UiKey::BUY_GOLD,
                    UiKey::BUY_FUEL,
                    UiKey::BUY_RUM,
                ];
                let sell_ui_keys = [
                    UiKey::SELL_FOOD,
                    UiKey::SELL_GOLD,
                    UiKey::SELL_FUEL,
                    UiKey::SELL_RUM,
                ];

                for (button_split_idx, resource) in [
                    Resource::FOOD,
                    Resource::GOLD,
                    Resource::FUEL,
                    Resource::RUM,
                ]
                .iter()
                .enumerate()
                {
                    let resource_split = Layout::horizontal([
                        Constraint::Length(10), // name
                        Constraint::Max(6),     // buy 1
                        Constraint::Max(6),     // buy 10
                        Constraint::Max(6),     // buy 100
                        Constraint::Max(6),     // sell 1
                        Constraint::Max(6),     // sell 10
                        Constraint::Max(6),     // sell 100
                        Constraint::Min(0),     // shortcut
                    ])
                    .split(button_split[button_split_idx]);

                    let buy_unit_cost = planet.resource_buy_price(*resource);
                    let sell_unit_cost = planet.resource_sell_price(*resource);
                    frame.render_widget(
                        Paragraph::new(format!(
                            "{:<4} {}/{}",
                            resource,
                            buy_ui_keys[button_split_idx].to_string(),
                            sell_ui_keys[button_split_idx].to_string(),
                        )),
                        resource_split[0].inner(&Margin {
                            horizontal: 1,
                            vertical: 1,
                        }),
                    );
                    frame.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::styled(format!("{}", buy_unit_cost), UiStyle::OK),
                            Span::raw(format!("/")),
                            Span::styled(format!("{}", sell_unit_cost), UiStyle::ERROR),
                        ])),
                        resource_split[7].inner(&Margin {
                            horizontal: 1,
                            vertical: 1,
                        }),
                    );
                    for (idx, amount) in [1, 10, 100].iter().enumerate() {
                        if let Ok(btn) = trade_button(
                            &world,
                            resource.clone(),
                            amount.clone(),
                            buy_unit_cost,
                            &self.callback_registry,
                            hover_text_target,
                        ) {
                            frame.render_widget(btn, resource_split[idx + 1]);
                        }
                        if let Ok(btn) = trade_button(
                            &world,
                            resource.clone(),
                            -amount.clone(),
                            buy_unit_cost,
                            &self.callback_registry,
                            hover_text_target,
                        ) {
                            frame.render_widget(btn, resource_split[idx + 4]);
                        }
                    }
                }
            }
            TeamLocation::Travelling { to, .. } => {
                if let Ok(mut lines) = self
                    .gif_map
                    .lock()
                    .unwrap()
                    .travelling_spaceship_lines(team.id, self.tick, world)
                {
                    let area = bottom_split[2].inner(&Margin {
                        horizontal: 1,
                        vertical: 1,
                    });
                    // Apply y-centering
                    let min_offset = if lines.len() > area.height as usize {
                        (lines.len() - area.height as usize) / 2
                    } else {
                        0
                    };
                    let max_offset = lines.len().min(min_offset + area.height as usize);
                    if min_offset > 0 || max_offset < lines.len() {
                        lines = lines[min_offset..max_offset].to_vec();
                    }
                    let paragraph = Paragraph::new(lines);
                    frame.render_widget(paragraph.centered(), area);
                }
                let planet = world.get_planet_or_err(to)?;
                frame.render_widget(
                    default_block().title(format!("Travelling to {}", planet.name)),
                    bottom_split[2],
                );
            }
            TeamLocation::Exploring { around, .. } => {
                if let Ok(mut lines) = self
                    .gif_map
                    .lock()
                    .unwrap()
                    .exploring_spaceship_lines(team.id, self.tick, world)
                {
                    let area = bottom_split[2].inner(&Margin {
                        horizontal: 1,
                        vertical: 1,
                    });
                    // Apply y-centering
                    let min_offset = if lines.len() > area.height as usize {
                        (lines.len() - area.height as usize) / 2
                    } else {
                        0
                    };
                    let max_offset = lines.len().min(min_offset + area.height as usize);
                    if min_offset > 0 || max_offset < lines.len() {
                        lines = lines[min_offset..max_offset].to_vec();
                    }
                    let paragraph = Paragraph::new(lines);
                    frame.render_widget(paragraph.centered(), area);
                }
                let planet = world.get_planet_or_err(around)?;
                frame.render_widget(
                    default_block().title(format!("Exploring around {}", planet.name)),
                    bottom_split[2],
                );
            }
        }

        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        world: &World,
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

            UiKey::EXPLORE => {
                return Some(UiCallbackPreset::ExploreAroundPlanet);
            }

            UiKey::BUY_FOOD => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(buy_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_buy_price(Resource::FOOD))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::FOOD,
                            amount: 1,
                            unit_cost: buy_price,
                        });
                    }
                }
            }

            UiKey::BUY_GOLD => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(buy_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_buy_price(Resource::GOLD))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::FOOD,
                            amount: 1,
                            unit_cost: buy_price,
                        });
                    }
                }
            }

            UiKey::BUY_FUEL => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(buy_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_buy_price(Resource::FUEL))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::FUEL,
                            amount: 1,
                            unit_cost: buy_price,
                        });
                    }
                }
            }

            UiKey::BUY_RUM => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(buy_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_buy_price(Resource::RUM))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::RUM,
                            amount: 1,
                            unit_cost: buy_price,
                        });
                    }
                }
            }

            UiKey::SELL_FOOD => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(sell_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_sell_price(Resource::FOOD))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::FOOD,
                            amount: -1,
                            unit_cost: sell_price,
                        });
                    }
                }
            }

            UiKey::SELL_GOLD => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(sell_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_sell_price(Resource::GOLD))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::GOLD,
                            amount: -1,
                            unit_cost: sell_price,
                        });
                    }
                }
            }

            UiKey::SELL_FUEL => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(sell_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_sell_price(Resource::FUEL))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::FUEL,
                            amount: -1,
                            unit_cost: sell_price,
                        });
                    }
                }
            }

            UiKey::SELL_RUM => {
                if let Some(planet_id) = self.current_planet_id {
                    if let Ok(sell_price) = world
                        .get_planet_or_err(planet_id)
                        .map(|p| p.resource_sell_price(Resource::RUM))
                    {
                        return Some(UiCallbackPreset::TradeResource {
                            resource: Resource::RUM,
                            amount: -1,
                            unit_cost: sell_price,
                        });
                    }
                }
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
