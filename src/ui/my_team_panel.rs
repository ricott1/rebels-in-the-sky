use super::{
    button::Button,
    clickable_list::ClickableListState,
    clickable_table::{ClickableCell, ClickableRow, ClickableTable, ClickableTableState},
    constants::*,
    gif_map::GifMap,
    traits::{PercentageRating, Screen, SplitPanel, UiStyled},
    ui_callback::{CallbackRegistry, UiCallbackPreset},
    utils::{format_satoshi, hover_text_target},
    widgets::*,
};
use crate::{
    engine::game::Game,
    store::load_game,
    types::{AppResult, GameId, PlayerId, SystemTimeTick, Tick},
    world::{
        constants::*,
        position::{GamePosition, Position, MAX_POSITION},
        skill::Rated,
        spaceship::{SpaceshipComponent, SpaceshipUpgrade},
        types::{TeamBonus, TeamLocation},
        world::World,
    },
};
use crate::{
    types::{PlanetId, TeamId},
    world::{resources::Resource, role::CrewRole},
};
use anyhow::anyhow;
use core::fmt::Debug;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::{
    layout::Margin,
    prelude::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum MyTeamView {
    #[default]
    Info,
    Games,
    Market,
    Shipyard,
    Asteroids,
}

impl MyTeamView {
    fn next(&self) -> Self {
        match self {
            MyTeamView::Info => MyTeamView::Games,
            MyTeamView::Games => MyTeamView::Market,
            MyTeamView::Market => MyTeamView::Shipyard,
            MyTeamView::Shipyard => MyTeamView::Asteroids,
            MyTeamView::Asteroids => MyTeamView::Info,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
enum PanelList {
    #[default]
    Top,
    Bottom,
}

#[derive(Debug, Default)]
pub struct MyTeamPanel {
    player_index: Option<usize>,
    game_index: Option<usize>,
    planet_index: Option<usize>,
    upgrade_index: usize,
    asteroid_index: Option<usize>,
    view: MyTeamView,
    active_list: PanelList,
    players: Vec<PlayerId>,
    recent_games: Vec<GameId>,
    loaded_games: HashMap<GameId, Game>,
    planet_markets: Vec<PlanetId>,
    challenge_teams: Vec<TeamId>,
    asteroid_ids: Vec<PlanetId>,
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

    fn render_view_buttons(&self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let hover_text_target = hover_text_target(frame);
        let mut view_info_button = Button::new(
            "View: Info".into(),
            UiCallbackPreset::SetMyTeamPanelView {
                view: MyTeamView::Info,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View team information.".into(), hover_text_target);

        let mut view_games_button = Button::new(
            "View: Games".into(),
            UiCallbackPreset::SetMyTeamPanelView {
                view: MyTeamView::Games,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View recent games.".into(), hover_text_target);

        let mut view_market_button = Button::new(
            "View: Market".into(),
            UiCallbackPreset::SetMyTeamPanelView {
                view: MyTeamView::Market,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text(
            "View market, buy and sell resources.".into(),
            hover_text_target,
        );

        let mut view_shipyard_button = Button::new(
            "View: Shipyard".into(),
            UiCallbackPreset::SetMyTeamPanelView {
                view: MyTeamView::Shipyard,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text(
            "View shipyard, improve your spaceship.".into(),
            hover_text_target,
        );

        let mut view_asteroids_button = Button::new(
            format!("View: Asteroids ({})", self.asteroid_ids.len()),
            UiCallbackPreset::SetMyTeamPanelView {
                view: MyTeamView::Asteroids,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text(
            "View asteorids found during exploration.".into(),
            hover_text_target,
        );

        match self.view {
            MyTeamView::Info => view_info_button.disable(None),
            MyTeamView::Games => view_games_button.disable(None),
            MyTeamView::Market => view_market_button.disable(None),
            MyTeamView::Shipyard => view_shipyard_button.disable(None),
            MyTeamView::Asteroids => view_asteroids_button.disable(None),
        }

        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

        frame.render_widget(view_info_button, split[0]);
        frame.render_widget(view_games_button, split[1]);
        frame.render_widget(view_market_button, split[2]);
        frame.render_widget(view_shipyard_button, split[3]);
        frame.render_widget(view_asteroids_button, split[4]);

        Ok(())
    }

    fn render_market(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_planet_markets(frame, world, split[0])?;
        self.render_market_buttons(frame, world, split[1])?;

        Ok(())
    }

    fn render_planet_markets(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;
        frame.render_widget(default_block().title("Planet Markets"), area);
        let split = Layout::horizontal([Constraint::Length(20), Constraint::Length(30)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let mut options = vec![];
        for &id in self.planet_markets.iter() {
            let planet = world.get_planet_or_err(id)?;
            let text = planet.name.clone();
            let style = match team.current_location {
                TeamLocation::OnPlanet { planet_id } => {
                    if planet_id == planet.id {
                        UiStyle::OWN_TEAM
                    } else {
                        UiStyle::DEFAULT
                    }
                }
                _ => UiStyle::DEFAULT,
            };
            options.push((text, style));
        }

        let list = selectable_list(options, &self.callback_registry);

        frame.render_stateful_widget(
            list,
            split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.planet_index),
        );

        let planet_id = self.planet_markets[self.planet_index.unwrap_or_default()];
        let planet = world.get_planet_or_err(planet_id)?;
        let merchant_bonus = TeamBonus::TradePrice.current_team_bonus(world, team.id)?;

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(format!("Resource: Buy/Sell"), UiStyle::HEADER)),
                Line::from(vec![
                    Span::styled("Fuel      ", UiStyle::STORAGE_FUEL),
                    Span::styled(
                        format!(
                            "{}",
                            planet.resource_buy_price(Resource::FUEL, merchant_bonus)
                        ),
                        UiStyle::OK,
                    ),
                    Span::raw("/"),
                    Span::styled(
                        format!(
                            "{}",
                            planet.resource_sell_price(Resource::FUEL, merchant_bonus)
                        ),
                        UiStyle::ERROR,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Gold      ", UiStyle::STORAGE_GOLD),
                    Span::styled(
                        format!(
                            "{}",
                            planet.resource_buy_price(Resource::GOLD, merchant_bonus)
                        ),
                        UiStyle::OK,
                    ),
                    Span::raw("/"),
                    Span::styled(
                        format!(
                            "{}",
                            planet.resource_sell_price(Resource::GOLD, merchant_bonus)
                        ),
                        UiStyle::ERROR,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Scraps    ", UiStyle::STORAGE_SCRAPS),
                    Span::styled(
                        format!(
                            "{}",
                            planet.resource_buy_price(Resource::SCRAPS, merchant_bonus)
                        ),
                        UiStyle::OK,
                    ),
                    Span::raw("/"),
                    Span::styled(
                        format!(
                            "{}",
                            planet.resource_sell_price(Resource::SCRAPS, merchant_bonus)
                        ),
                        UiStyle::ERROR,
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Rum       ", UiStyle::STORAGE_RUM),
                    Span::styled(
                        format!(
                            "{}",
                            planet.resource_buy_price(Resource::RUM, merchant_bonus)
                        ),
                        UiStyle::OK,
                    ),
                    Span::raw("/"),
                    Span::styled(
                        format!(
                            "{}",
                            planet.resource_sell_price(Resource::RUM, merchant_bonus)
                        ),
                        UiStyle::ERROR,
                    ),
                ]),
            ]),
            split[1],
        );

        Ok(())
    }

    fn render_market_buttons(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;

        let planet_id = match team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            TeamLocation::Travelling { .. } => {
                frame.render_widget(default_block().title("Market"), area);
                frame.render_widget(
                    Paragraph::new("There is no market available while travelling."),
                    area.inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );
                return Ok(());
            }
            TeamLocation::Exploring { .. } => {
                frame.render_widget(default_block().title("Market"), area);
                frame.render_widget(
                    Paragraph::new("There is no market available while exploring."),
                    area.inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                );
                return Ok(());
            }
        };

        let planet = world.get_planet_or_err(planet_id)?;
        frame.render_widget(
            default_block().title(format!("Planet {} Market", planet.name)),
            area,
        );
        if planet.total_population() == 0 {
            frame.render_widget(
                Paragraph::new(
                    "There is no market available on this planet!\nTry another planet with more population.",
                ),
                area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
            );
            return Ok(());
        }
        let hover_text_target = hover_text_target(frame);

        let button_split = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("       Key                                       Buy/Sell"),
                UiStyle::HEADER,
            )),
            button_split[0],
        );

        let buy_ui_keys = [
            UiKey::BUY_FUEL,
            UiKey::BUY_GOLD,
            UiKey::BUY_SCRAPS,
            UiKey::BUY_RUM,
        ];
        let sell_ui_keys = [
            UiKey::SELL_FUEL,
            UiKey::SELL_GOLD,
            UiKey::SELL_SCRAPS,
            UiKey::SELL_RUM,
        ];

        for (button_split_idx, resource) in [
            Resource::FUEL,
            Resource::GOLD,
            Resource::SCRAPS,
            Resource::RUM,
        ]
        .iter()
        .enumerate()
        {
            let resource_split = Layout::horizontal([
                Constraint::Length(12), // name
                Constraint::Max(6),     // buy 1
                Constraint::Max(6),     // buy 10
                Constraint::Max(6),     // buy 100
                Constraint::Max(6),     // sell 1
                Constraint::Max(6),     // sell 10
                Constraint::Max(6),     // sell 100
                Constraint::Min(0),     // price
            ])
            .split(button_split[button_split_idx + 1]);

            let merchant_bonus = TeamBonus::TradePrice.current_team_bonus(world, team.id)?;
            let buy_unit_cost = planet.resource_buy_price(*resource, merchant_bonus);
            let sell_unit_cost = planet.resource_sell_price(*resource, merchant_bonus);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{:<6} ", resource.to_string()), resource.style()),
                    Span::styled(
                        format!("{}", buy_ui_keys[button_split_idx].to_string()),
                        UiStyle::OK,
                    ),
                    Span::raw(format!("/")),
                    Span::styled(
                        format!("{}", sell_ui_keys[button_split_idx].to_string()),
                        UiStyle::ERROR,
                    ),
                ])),
                resource_split[0].inner(Margin {
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
                resource_split[7].inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
            );

            let max_buy_amount = team.max_resource_buy_amount(*resource, buy_unit_cost);
            for (idx, amount) in [1, 10, max_buy_amount as i32].iter().enumerate() {
                if let Ok(btn) = trade_resource_button(
                    &world,
                    *resource,
                    *amount,
                    buy_unit_cost,
                    &self.callback_registry,
                    hover_text_target,
                    if idx == 0 {
                        Some(buy_ui_keys[button_split_idx])
                    } else {
                        None
                    },
                ) {
                    frame.render_widget(btn, resource_split[idx + 1]);
                }
            }

            let max_sell_amount = team.max_resource_sell_amount(*resource);
            for (idx, amount) in [1, 10, max_sell_amount as i32].iter().enumerate() {
                if let Ok(btn) = trade_resource_button(
                    &world,
                    *resource,
                    -*amount,
                    sell_unit_cost,
                    &self.callback_registry,
                    hover_text_target,
                    if idx == 0 {
                        Some(sell_ui_keys[button_split_idx])
                    } else {
                        None
                    },
                ) {
                    frame.render_widget(btn, resource_split[idx + 4]);
                }
            }
        }

        let mut info_spans = vec![];
        info_spans.append(&mut get_fuel_spans(team));
        info_spans.push(Span::raw("  "));
        info_spans.append(&mut get_storage_spans(team));
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(format!("Treasury: {}", format_satoshi(team.balance()))),
                Line::from(info_spans),
            ]),
            button_split[5].inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );

        Ok(())
    }

    fn render_info(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;
        let hover_text_target = hover_text_target(&frame);

        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);

        let info = Paragraph::new(vec![
            Line::from(""),
            Line::from(format!(
                "Rating {:5}  Reputation {:5}",
                world.team_rating(team.id).unwrap_or_default().stars(),
                team.reputation.stars(),
            )),
            Line::from(vec![
                Span::raw(format!(
                    "Game record: W{}/L{}/D{}  ",
                    team.game_record[0], team.game_record[1], team.game_record[2]
                )),
                Span::styled(
                    format!(
                        "Network: W{}/L{}/D{} ",
                        team.network_game_record[0],
                        team.network_game_record[1],
                        team.network_game_record[2]
                    ),
                    UiStyle::NETWORK,
                ),
            ]),
            Line::from(format!("Treasury: {:<10}", format_satoshi(team.balance()),)),
            Line::from(get_crew_spans(team)),
            Line::from(get_fuel_spans(team)),
            Line::from(get_storage_spans(team)),
            Line::from(vec![
                Span::styled("   Gold", UiStyle::STORAGE_GOLD),
                Span::raw(format!(
                    ":   {} Kg",
                    team.resources
                        .get(&Resource::GOLD)
                        .copied()
                        .unwrap_or_default()
                )),
            ]),
            Line::from(vec![
                Span::styled("   Scraps", UiStyle::STORAGE_SCRAPS),
                Span::raw(format!(
                    ": {} t",
                    team.resources
                        .get(&Resource::SCRAPS)
                        .copied()
                        .unwrap_or_default()
                )),
            ]),
            Line::from(vec![
                Span::styled("   Rum", UiStyle::STORAGE_RUM),
                Span::raw(format!(
                    ":    {} l",
                    team.resources
                        .get(&Resource::RUM)
                        .copied()
                        .unwrap_or_default()
                )),
            ]),
        ]);
        frame.render_widget(default_block().title("Info"), split[0]);
        frame.render_widget(
            info,
            split[0].inner(Margin {
                horizontal: 2,
                vertical: 1,
            }),
        );

        let btm_split = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(split[0].inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        let top_button_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(btm_split[1]);

        let offense_tactic_button = Button::new(
            format!("tactic: {}", team.game_tactic),
            UiCallbackPreset::SetTeamTactic {
                tactic: team.game_tactic.next(),
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Tactics affect the actions the team will choose during the game.\n{}: {}",
                team.game_tactic,
                team.game_tactic.description()
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::SET_TACTIC);
        frame.render_widget(offense_tactic_button, top_button_split[0]);

        let can_change_training_focus = team.can_change_training_focus();
        let mut training_button = Button::new(
            format!(
                "Training: {}",
                if let Some(focus) = team.training_focus {
                    focus.to_string()
                } else {
                    "General".to_string()
                }
            ),
            UiCallbackPreset::NextTrainingFocus { team_id: team.id },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            "Change the training focus, which affects how player skills increase.".into(),
            hover_text_target,
        )
        .set_hotkey(UiKey::TRAINING_FOCUS);
        if can_change_training_focus.is_err() {
            training_button.disable(Some(format!(
                "{}",
                can_change_training_focus.unwrap_err().to_string()
            )));
        }
        frame.render_widget(training_button, top_button_split[1]);

        let btm_button_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(btm_split[2]);

        if let Ok(go_to_team_current_planet_button) = go_to_team_current_planet_button(
            world,
            team,
            &self.callback_registry,
            hover_text_target,
        ) {
            frame.render_widget(go_to_team_current_planet_button, btm_button_split[0]);
        }

        if let Ok(home_planet_button) =
            go_to_team_home_planet_button(world, team, &self.callback_registry, hover_text_target)
        {
            frame.render_widget(home_planet_button, btm_button_split[1]);
        }

        match team.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                if let Some(upgrade) = &team.spaceship.pending_upgrade {
                    self.render_upgrading_spaceship(frame, world, split[1], upgrade)?
                } else {
                    self.render_on_planet_spaceship(frame, world, split[1], planet_id)?
                }
            }
            TeamLocation::Travelling {
                to,
                started,
                duration,
                ..
            } => {
                let countdown = if started + duration > world.last_tick_short_interval {
                    (started + duration - world.last_tick_short_interval).formatted()
                } else {
                    (0 as Tick).formatted()
                };
                self.render_travelling_spaceship(frame, world, split[1], to, countdown)?
            }
            TeamLocation::Exploring {
                around,
                started,
                duration,
                ..
            } => {
                let countdown = if started + duration > world.last_tick_short_interval {
                    (started + duration - world.last_tick_short_interval).formatted()
                } else {
                    (0 as Tick).formatted()
                };
                self.render_exploring_spaceship(frame, world, split[1], around, countdown)?
            }
        }
        Ok(())
    }

    fn render_games(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_challenge_teams(frame, world, split[0])?;
        self.render_recent_games(frame, world, split[1])?;
        Ok(())
    }

    fn render_challenge_teams(
        &self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        frame.render_widget(default_block().title("Open to challenge "), area);
        let team = world.get_own_team()?;
        let split = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );
        if let Some(game_id) = team.current_game {
            let game = world.get_game_or_err(game_id)?;
            let hover_text_target = hover_text_target(frame);

            let game_text = if let Some(action) = game.action_results.last() {
                format!(
                    "{} {:>3}-{:<3} {}",
                    game.home_team_in_game.name,
                    action.home_score,
                    action.away_score,
                    game.away_team_in_game.name,
                )
            } else {
                format!(
                    "{}   0-0   {}",
                    game.home_team_in_game.name, game.away_team_in_game.name,
                )
            };
            frame.render_widget(
                Button::new(
                    format!("Playing - {}", game_text),
                    UiCallbackPreset::GoToGame { game_id },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text("Go to current game".into(), hover_text_target)
                .set_hotkey(UiKey::GO_TO_GAME)
                .set_box_style(UiStyle::OWN_TEAM),
                split[0],
            );
            return Ok(());
        }

        let split = Layout::horizontal([Constraint::Min(16), Constraint::Max(24)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let displayed_challenges = self.challenge_teams.len().min(area.height as usize / 3 - 1);

        let left_split =
            Layout::vertical([Constraint::Length(3)].repeat(displayed_challenges)).split(split[0]);
        let right_split =
            Layout::vertical([Constraint::Length(3)].repeat(displayed_challenges)).split(split[1]);

        let hover_text_target = hover_text_target(&frame);

        for (idx, &team_id) in self
            .challenge_teams
            .iter()
            .take(displayed_challenges)
            .enumerate()
        {
            let team = world.get_team_or_err(team_id)?;

            frame.render_widget(
                Paragraph::new(format!(
                    "{:<MAX_NAME_LENGTH$} {}",
                    team.name,
                    world.team_rating(team_id).unwrap_or_default().stars()
                )),
                left_split[idx].inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
            );

            render_challenge_button(
                world,
                team,
                &self.callback_registry,
                hover_text_target,
                idx == 0,
                frame,
                right_split[idx],
            )?;
        }

        Ok(())
    }

    fn render_recent_games(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        frame.render_widget(default_block().title("Recent Games".to_string()), area);

        if self.recent_games.len() == 0 {
            return Ok(());
        }

        let team = world.get_own_team()?;
        let split = Layout::horizontal([Constraint::Max(36), Constraint::Min(20)]).split(area);

        let mut options = vec![];
        if team.current_game.is_some() {
            if let Some(game) = world.games.get(&team.current_game.unwrap()) {
                if let Some(action) = game.action_results.last() {
                    let text = format!(
                        " {:>12} {:>3}-{:<3} {:<}",
                        game.home_team_in_game.name,
                        action.home_score,
                        action.away_score,
                        game.away_team_in_game.name,
                    );
                    let style = UiStyle::OWN_TEAM;
                    options.push((text, style));
                }
            }
        }

        for game_id in self.recent_games.iter() {
            if let Some(game) = world.past_games.get(game_id) {
                let text = format!(
                    " {:>12} {:>3}-{:<3} {:<}",
                    game.home_team_name,
                    game.home_quarters_score.iter().sum::<u16>(),
                    game.away_quarters_score.iter().sum::<u16>(),
                    game.away_team_name,
                );

                let style = UiStyle::DEFAULT;
                options.push((text, style));
            }
        }
        let list = selectable_list(options, &self.callback_registry);

        frame.render_stateful_widget(
            list,
            split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.game_index),
        );

        let game_id = self.recent_games[self.game_index.unwrap()];
        let summary = if let Ok(current_game) = world.get_game_or_err(game_id) {
            Paragraph::new(format!(
                "Location {} - Attendance {}\nCurrently playing: {}",
                world.get_planet_or_err(current_game.location)?.name,
                current_game.attendance,
                current_game.timer.format(),
            ))
        } else {
            if self.loaded_games.get(&game_id).is_none() {
                let game = load_game(game_id)?;
                self.loaded_games.insert(game_id, game);
            }
            let game = world
                .past_games
                .get(&game_id)
                .ok_or(anyhow!("Unable to get past game."))?;

            let loaded_game = self
                .loaded_games
                .get(&game_id)
                .expect("Failed to load game");

            let home_mvps = loaded_game
                .home_team_mvps
                .as_ref()
                .expect("Loaded game should have set mvps.");
            let away_mvps = loaded_game
                .away_team_mvps
                .as_ref()
                .expect("Loaded game should have set mvps.");

            let lines = vec![
                Line::from(format!(
                    "Location {} - Attendance {}",
                    world.get_planet_or_err(game.location)?.name,
                    game.attendance
                )),
                Line::from(format!(
                    "Ended on {}",
                    game.ended_at
                        .expect("Past games should have ended")
                        .formatted_as_date()
                )),
                Line::from(""),
                Line::from(Span::styled(
                    format!(
                        "{:12} {} {} {} {} {}",
                        "Team", "Q1", "Q2", "Q3", "Q4", "Total"
                    ),
                    UiStyle::HEADER,
                )),
                Line::from(format!(
                    "{:12} {:02} {:02} {:02} {:02} {:<3} {}",
                    game.home_team_name,
                    game.home_quarters_score[0],
                    game.home_quarters_score[1],
                    game.home_quarters_score[2],
                    game.home_quarters_score[3],
                    game.home_quarters_score.iter().sum::<u16>(),
                    if game.home_team_knocked_out {
                        "knocked out"
                    } else {
                        ""
                    }
                )),
                Line::from(format!(
                    "{:12} {:02} {:02} {:02} {:02} {:<3} {}",
                    game.away_team_name,
                    game.away_quarters_score[0],
                    game.away_quarters_score[1],
                    game.away_quarters_score[2],
                    game.away_quarters_score[3],
                    game.away_quarters_score.iter().sum::<u16>(),
                    if game.away_team_knocked_out {
                        "knocked out"
                    } else {
                        ""
                    }
                )),
                Line::from(format!("")),
                Line::from(Span::styled(game.home_team_name.clone(), UiStyle::HEADER)),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    home_mvps[0].name,
                    format!(
                        "{:>2} {}",
                        home_mvps[0].best_stats[0].1, home_mvps[0].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[0].best_stats[1].1, home_mvps[0].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[0].best_stats[2].1, home_mvps[0].best_stats[2].0
                    )
                )),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    home_mvps[1].name,
                    format!(
                        "{:>2} {}",
                        home_mvps[1].best_stats[0].1, home_mvps[1].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[1].best_stats[1].1, home_mvps[1].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[1].best_stats[2].1, home_mvps[1].best_stats[2].0
                    )
                )),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    home_mvps[2].name,
                    format!(
                        "{:>2} {}",
                        home_mvps[2].best_stats[0].1, home_mvps[2].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[2].best_stats[1].1, home_mvps[2].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        home_mvps[2].best_stats[2].1, home_mvps[2].best_stats[2].0
                    )
                )),
                Line::from(""),
                Line::from(Span::styled(game.away_team_name.clone(), UiStyle::HEADER)),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    away_mvps[0].name,
                    format!(
                        "{:>2} {}",
                        away_mvps[0].best_stats[0].1, away_mvps[0].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[0].best_stats[1].1, away_mvps[0].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[0].best_stats[2].1, away_mvps[0].best_stats[2].0
                    )
                )),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    away_mvps[1].name,
                    format!(
                        "{:>2} {}",
                        away_mvps[1].best_stats[0].1, away_mvps[1].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[1].best_stats[1].1, away_mvps[1].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[1].best_stats[2].1, away_mvps[1].best_stats[2].0
                    )
                )),
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    away_mvps[2].name,
                    format!(
                        "{:>2} {}",
                        away_mvps[2].best_stats[0].1, away_mvps[2].best_stats[0].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[2].best_stats[1].1, away_mvps[2].best_stats[1].0
                    ),
                    format!(
                        "{:>2} {}",
                        away_mvps[2].best_stats[2].1, away_mvps[2].best_stats[2].0
                    )
                )),
            ];

            Paragraph::new(lines)
        };

        frame.render_widget(
            summary,
            split[1].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        Ok(())
    }

    fn render_shipyard(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_shipyard_upgrades(frame, world, split[0])?;

        let team = world.get_own_team()?;
        match team.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                if let Some(upgrade) = &team.spaceship.pending_upgrade {
                    self.render_upgrading_spaceship(frame, world, split[1], upgrade)?
                } else {
                    self.render_in_shipyard_spaceship(frame, world, split[1], planet_id)?
                }
            }
            TeamLocation::Travelling {
                to,
                started,
                duration,
                ..
            } => {
                let countdown = if started + duration > world.last_tick_short_interval {
                    (started + duration - world.last_tick_short_interval).formatted()
                } else {
                    (0 as Tick).formatted()
                };
                self.render_travelling_spaceship(frame, world, split[1], to, countdown)?
            }
            TeamLocation::Exploring {
                around,
                started,
                duration,
                ..
            } => {
                let countdown = if started + duration > world.last_tick_short_interval {
                    (started + duration - world.last_tick_short_interval).formatted()
                } else {
                    (0 as Tick).formatted()
                };
                self.render_exploring_spaceship(frame, world, split[1], around, countdown)?
            }
        }

        Ok(())
    }

    fn render_shipyard_upgrades(
        &self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(12), Constraint::Length(30)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );
        let team = world.get_own_team()?;
        frame.render_widget(default_block().title("Upgrades "), area);

        let hull_style = if team.spaceship.hull.can_be_upgraded() {
            UiStyle::DEFAULT
        } else {
            UiStyle::UNSELECTABLE
        };
        let engine_style = if team.spaceship.engine.can_be_upgraded() {
            UiStyle::DEFAULT
        } else {
            UiStyle::UNSELECTABLE
        };
        let storage_style = if team.spaceship.storage.can_be_upgraded() {
            UiStyle::DEFAULT
        } else {
            UiStyle::UNSELECTABLE
        };

        let options = vec![
            ("Hull".into(), hull_style),
            ("Engine".into(), engine_style),
            ("Storage".into(), storage_style),
        ];

        let list = selectable_list(options, &self.callback_registry);

        frame.render_stateful_widget(
            list,
            split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(Some(self.upgrade_index)),
        );

        let component = match self.upgrade_index {
            0 => "Hull",
            1 => "Engine",
            2 => "Storage",
            _ => panic!("Invalid upgrade index"),
        };

        let current = match self.upgrade_index {
            0 => team.spaceship.hull.to_string(),
            1 => team.spaceship.engine.to_string(),
            2 => team.spaceship.storage.to_string(),
            _ => panic!("Invalid upgrade index"),
        };
        let next = match self.upgrade_index {
            0 => team.spaceship.hull.next().to_string(),
            1 => team.spaceship.engine.next().to_string(),
            2 => team.spaceship.storage.next().to_string(),
            _ => panic!("Invalid upgrade index"),
        };

        let upgrade_cost = match self.upgrade_index {
            0 => team.spaceship.hull.upgrade_cost(),
            1 => team.spaceship.engine.upgrade_cost(),
            2 => team.spaceship.storage.upgrade_cost(),
            _ => panic!("Invalid upgrade index"),
        };

        let can_be_upgraded = match self.upgrade_index {
            0 => team.spaceship.hull.can_be_upgraded(),
            1 => team.spaceship.engine.can_be_upgraded(),
            2 => team.spaceship.storage.can_be_upgraded(),
            _ => panic!("Invalid upgrade index"),
        };

        let is_being_upgraded = if let Some(upgrade) = team.spaceship.pending_upgrade.as_ref() {
            match self.upgrade_index {
                0 => upgrade.hull.is_some(),
                1 => upgrade.engine.is_some(),
                2 => upgrade.storage.is_some(),
                _ => panic!("Invalid upgrade index"),
            }
        } else {
            false
        };

        let upgrade_to_text = if is_being_upgraded {
            "Currently upgrading".to_string()
        } else if can_be_upgraded {
            format!("{} -> {}", current, next)
        } else {
            "Fully upgraded".to_string()
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("Upgrade {}", component),
                UiStyle::HEADER,
            ))
            .centered(),
            Line::from(Span::raw(upgrade_to_text)).centered(),
            Line::from(""),
        ];

        if can_be_upgraded && !is_being_upgraded {
            for (resource, amount) in upgrade_cost.iter() {
                let have = team.resources.get(resource).copied().unwrap_or_default();
                let style = if amount.clone() > have {
                    UiStyle::ERROR
                } else {
                    UiStyle::OK
                };

                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", resource.to_string()), resource.style()),
                    Span::styled(format!("{}/{}", have, amount), style),
                ]));
            }
        }

        frame.render_widget(Paragraph::new(lines), split[1]);

        Ok(())
    }

    fn render_asteroids(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_asteroid_list(frame, world, split[0])?;
        self.render_selected_asteroid(frame, world, split[1])?;
        Ok(())
    }

    fn render_asteroid_list(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        frame.render_widget(default_block().title("Asteroids "), area);
        let team = world.get_own_team()?;

        if self.asteroid_ids.len() == 0 {
            frame.render_widget(
                Paragraph::new("No asteroid has been found yet, keep exploring!")
                    .wrap(Wrap { trim: true }),
                area.inner(Margin {
                    horizontal: 2,
                    vertical: 2,
                }),
            );
            return Ok(());
        }

        let split = Layout::horizontal([Constraint::Length(12), Constraint::Length(30)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let options = self
            .asteroid_ids
            .iter()
            .filter(|&&asteroid_id| world.get_planet_or_err(asteroid_id).is_ok())
            .map(|&asteroid_id| {
                let asteroid = world.get_planet_or_err(asteroid_id).unwrap();
                let style = match team.current_location {
                    TeamLocation::OnPlanet { planet_id } => {
                        if planet_id == asteroid_id {
                            UiStyle::OWN_TEAM
                        } else {
                            UiStyle::DEFAULT
                        }
                    }
                    _ => UiStyle::DEFAULT,
                };
                (asteroid.name.clone(), style)
            })
            .collect_vec();

        let list = selectable_list(options, &self.callback_registry);

        frame.render_stateful_widget(
            list,
            split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.asteroid_index),
        );

        Ok(())
    }

    fn render_selected_asteroid(
        &self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if self.asteroid_ids.len() == 0 {
            frame.render_widget(default_block(), area);
            return Ok(());
        }

        let asteroid =
            world.get_planet_or_err(self.asteroid_ids[self.asteroid_index.unwrap_or_default()])?;

        frame.render_widget(default_block().title(format!("{} ", asteroid.name)), area);

        let split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let img_lines = self
            .gif_map
            .lock()
            .unwrap()
            .planet_zoom_out_frame_lines(asteroid, 0, world)?;
        frame.render_widget(
            Paragraph::new(img_lines).centered(),
            split[0].inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
        );

        let b_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(split[1]);

        frame.render_widget(
            Paragraph::new(Span::styled(
                "No kartoffeln to plant",
                UiStyle::DISCONNECTED,
            ))
            .centered()
            .block(default_block()),
            b_split[0],
        );
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No kartoffeln to harvest",
                UiStyle::DISCONNECTED,
            ))
            .centered()
            .block(default_block()),
            b_split[1],
        );

        Ok(())
    }

    fn render_player_buttons(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;
        if self.player_index.is_none() {
            return Ok(());
        }
        let player_id = self.players[self.player_index.unwrap()];
        let player = world.get_player_or_err(player_id)?;
        let hover_text_target = hover_text_target(frame);
        let button_splits = Layout::horizontal([
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(32),
            Constraint::Length(32),
            Constraint::Min(0),
        ])
        .split(area.inner(Margin {
            vertical: 0,
            horizontal: 1,
        }));
        let can_set_as_captain = team.can_set_crew_role(&player, CrewRole::Captain);
        let mut captain_button = Button::new(
            "captain".into(),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Captain,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Set player to captain role: {} +{}%, {} {}%",
                TeamBonus::Reputation,
                TeamBonus::Reputation.as_skill(player)?.percentage(),
                TeamBonus::TradePrice,
                TeamBonus::TradePrice.as_skill(player)?.percentage()
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::SET_CAPTAIN);
        if can_set_as_captain.is_err() {
            captain_button.disable(None);
        }
        frame.render_widget(captain_button, button_splits[0]);

        let can_set_as_pilot = team.can_set_crew_role(&player, CrewRole::Pilot);

        let mut pilot_button = Button::new(
            "pilot".into(),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Pilot,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Set player to pilot role: {} +{}%, {} {}%",
                TeamBonus::SpaceshipSpeed,
                TeamBonus::SpaceshipSpeed.as_skill(player)?.percentage(),
                TeamBonus::Exploration,
                TeamBonus::Exploration.as_skill(player)?.percentage()
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::SET_PILOT);
        if can_set_as_pilot.is_err() {
            pilot_button.disable(None);
        }
        frame.render_widget(pilot_button, button_splits[1]);

        let can_set_as_doctor = team.can_set_crew_role(&player, CrewRole::Doctor);

        let mut doctor_button = Button::new(
            "doctor".into(),
            UiCallbackPreset::SetCrewRole {
                player_id,
                role: CrewRole::Doctor,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text(
            format!(
                "Set player to doctor role: {} +{}%, {} {}%",
                TeamBonus::TirednessRecovery,
                TeamBonus::TirednessRecovery.as_skill(player)?.percentage(),
                TeamBonus::Training,
                TeamBonus::Training.as_skill(player)?.percentage()
            ),
            hover_text_target,
        )
        .set_hotkey(UiKey::SET_DOCTOR);
        if can_set_as_doctor.is_err() {
            doctor_button.disable(None);
        }
        frame.render_widget(doctor_button, button_splits[2]);

        let can_release = team.can_release_player(&player);
        let mut release_button = Button::new(
            format!("Fire {}", player.info.shortened_name()),
            UiCallbackPreset::PromptReleasePlayer { player_id },
            Arc::clone(&self.callback_registry),
        )
        .set_hover_text("Fire pirate from the crew!".into(), hover_text_target)
        .set_hotkey(UiKey::FIRE);
        if can_release.is_err() {
            release_button.disable(Some(format!("{}", can_release.unwrap_err().to_string())));
        }

        frame.render_widget(release_button, button_splits[3]);

        if let Ok(drink_button) =
            drink_button(world, player_id, &self.callback_registry, hover_text_target)
        {
            frame.render_widget(drink_button, button_splits[4]);
        }

        Ok(())
    }

    fn build_players_table(&self, world: &World) -> AppResult<ClickableTable> {
        let team = world.get_own_team().unwrap();
        let header_cells = [
            " Name",
            "Overall",
            "Potential",
            "Current",
            "Best",
            "Role",
            "Crew bonus",
            "Crew bonus",
        ]
        .iter()
        .map(|h| ClickableCell::from(*h).style(UiStyle::HEADER));
        let header = ClickableRow::new(header_cells);
        let rows = self
            .players
            .iter()
            .map(|&id| {
                let player = world.get_player(id).unwrap();
                let skills = player.current_skill_array();

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
                let overall = player.average_skill().stars();
                let potential = player.potential.stars();

                let bonus_string_1 = match player.info.crew_role {
                    CrewRole::Pilot => {
                        let skill = TeamBonus::SpaceshipSpeed.as_skill(player)?;
                        Span::styled(
                            format!("{} +{}%", TeamBonus::SpaceshipSpeed, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Captain => {
                        let skill = TeamBonus::Reputation.as_skill(player)?;
                        Span::styled(
                            format!("{} +{}%", TeamBonus::Reputation, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Doctor => {
                        let skill = TeamBonus::TirednessRecovery.as_skill(player)?;
                        Span::styled(
                            format!("{} +{}%", TeamBonus::TirednessRecovery, skill.percentage()),
                            skill.style(),
                        )
                    }
                    _ => Span::raw(""),
                };

                let bonus_string_2 = match player.info.crew_role {
                    CrewRole::Pilot => {
                        let skill = TeamBonus::Exploration.as_skill(player)?;
                        Span::styled(
                            format!("{} +{}%", TeamBonus::Exploration, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Captain => {
                        let skill = TeamBonus::TradePrice.as_skill(player)?;
                        Span::styled(
                            format!("{} +{}%", TeamBonus::TradePrice, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Doctor => {
                        let skill = TeamBonus::Training.as_skill(player)?;
                        Span::styled(
                            format!("{} +{}%", TeamBonus::Training, skill.percentage()),
                            skill.style(),
                        )
                    }
                    _ => Span::raw(""),
                };

                let cells = [
                    ClickableCell::from(format!(
                        " {} {}",
                        player.info.first_name, player.info.last_name
                    )),
                    ClickableCell::from(overall),
                    ClickableCell::from(potential),
                    ClickableCell::from(current_role),
                    ClickableCell::from(format!(
                        "{:<2} {:<5}",
                        best_role.as_str(),
                        best_role.player_rating(skills).stars()
                    )),
                    ClickableCell::from(player.info.crew_role.to_string()),
                    ClickableCell::from(bonus_string_1),
                    ClickableCell::from(bonus_string_2),
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
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(17),
                Constraint::Length(17),
            ]);

        Ok(table)
    }

    fn render_players_top(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let hover_text_target = hover_text_target(frame);
        let team = world.get_own_team()?;
        let top_split =
            Layout::horizontal([Constraint::Min(10), Constraint::Length(60)]).split(area);

        let table = self.build_players_table(world)?;

        frame.render_stateful_widget(
            table.block(default_block().title(format!(
                "{} {} ↓/↑",
                team.name.clone(),
                world.team_rating(team.id).unwrap_or_default().stars()
            ))),
            top_split[0],
            &mut ClickableTableState::default().with_selected(self.player_index),
        );

        if self.player_index.is_none() {
            return Ok(());
        }
        let player_id = self.players[self.player_index.unwrap()];

        let player = world
            .get_player(player_id)
            .ok_or(anyhow!("Player {:?} not found", player_id))?;

        render_player_description(
            player,
            &self.gif_map,
            &self.callback_registry,
            self.tick,
            frame,
            world,
            top_split[1],
        );

        if team.current_game.is_none() {
            let table_bottom = Layout::vertical([
                Constraint::Min(10),
                Constraint::Length(3), //role buttons
                Constraint::Length(3), //buttons
                Constraint::Length(1), //margin box
            ])
            .split(area);

            let position_button_splits = Layout::horizontal([
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(3),  //margin
                Constraint::Length(32), //auto-assign
                Constraint::Min(0),
            ])
            .split(table_bottom[1].inner(Margin {
                vertical: 0,
                horizontal: 1,
            }));

            for idx in 0..MAX_POSITION as usize {
                let position = idx as Position;
                let rect = position_button_splits[idx];
                let mut button = Button::new(
                    format!("{}:{:<2}", (idx + 1), position.as_str()),
                    UiCallbackPreset::SwapPlayerPositions {
                        player_id,
                        position: idx,
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!("Set player initial position to {}.", position.as_str()),
                    hover_text_target,
                )
                .set_hotkey(UiKey::set_player_position(idx as Position));

                let position = team.player_ids.iter().position(|id| *id == player.id);
                if position.is_some() && position.unwrap() == idx {
                    button.disable(None);
                }
                frame.render_widget(button, rect);
            }

            let auto_assign_button = Button::new(
                "Auto-assign positions".into(),
                UiCallbackPreset::AssignBestTeamPositions,
                Arc::clone(&self.callback_registry),
            )
            .set_hover_text(
                "Auto-assign players' initial position.".into(),
                hover_text_target,
            )
            .set_hotkey(UiKey::AUTO_ASSIGN);
            frame.render_widget(auto_assign_button, position_button_splits[6]);
            self.render_player_buttons(frame, world, table_bottom[2])?;
        }

        Ok(())
    }

    fn render_on_planet_spaceship(
        &self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        _planet_id: PlanetId,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        let hover_text_target = hover_text_target(&frame);

        let split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );
        render_spaceship_description(&team, &self.gif_map, self.tick, world, frame, area);

        let explore_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(split[1]);
        if let Ok(explore_button) =
            quick_explore_button(world, team, &self.callback_registry, hover_text_target)
        {
            frame.render_widget(explore_button, explore_split[0]);
        }
        if let Ok(explore_button) =
            long_explore_button(world, team, &self.callback_registry, hover_text_target)
        {
            frame.render_widget(explore_button, explore_split[1]);
        }
        Ok(())
    }

    fn render_upgrading_spaceship(
        &self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        upgrade: &SpaceshipUpgrade,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;

        let countdown = if upgrade.started + upgrade.duration > world.last_tick_short_interval {
            (upgrade.started + upgrade.duration - world.last_tick_short_interval).formatted()
        } else {
            (0 as Tick).formatted()
        };

        render_spaceship_upgrade(
            &team,
            &upgrade,
            &self.gif_map,
            self.tick,
            world,
            frame,
            area,
        );

        frame.render_widget(
            default_block().title(format!(
                "Upgrading Spaceship {} - {}",
                upgrade.target()?,
                countdown
            )),
            area,
        );

        Ok(())
    }

    fn render_in_shipyard_spaceship(
        &self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        _planet_id: PlanetId,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        let hover_text_target = hover_text_target(&frame);

        let split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );

        let mut upgrade = SpaceshipUpgrade {
            hull: None,
            engine: None,
            storage: None,
            cost: vec![],
            started: Tick::now(),
            duration: SPACESHIP_UPGRADE_BASE_DURATION,
        };

        match self.upgrade_index {
            0 => {
                if team.spaceship.hull.can_be_upgraded() {
                    upgrade.hull = Some(team.spaceship.hull.next());
                    upgrade.cost = team.spaceship.hull.upgrade_cost();
                }
            }
            1 => {
                if team.spaceship.engine.can_be_upgraded() {
                    upgrade.engine = Some(team.spaceship.engine.next());
                    upgrade.cost = team.spaceship.engine.upgrade_cost();
                }
            }
            2 => {
                if team.spaceship.storage.can_be_upgraded() {
                    upgrade.storage = Some(team.spaceship.storage.next());
                    upgrade.cost = team.spaceship.storage.upgrade_cost();
                }
            }
            _ => panic!("Invalid upgrade_index"),
        };

        render_spaceship_upgrade(
            &team,
            &upgrade,
            &self.gif_map,
            self.tick,
            world,
            frame,
            area,
        );
        frame.render_widget(
            default_block().title(format!(
                "Upgraded Spaceship {}",
                upgrade.target().unwrap_or("None")
            )),
            area,
        );

        if let Ok(upgrade_button) =
            upgrade_spaceship_button(team, &self.callback_registry, hover_text_target, upgrade)
        {
            frame.render_widget(upgrade_button, split[1]);
        }

        Ok(())
    }

    fn render_travelling_spaceship(
        &self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        planet_id: PlanetId,
        countdown: String,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        if let Ok(mut lines) = self
            .gif_map
            .lock()
            .unwrap()
            .travelling_spaceship_lines(team.id, self.tick, world)
        {
            let rect = area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            });
            // Apply y-centering
            let min_offset = if lines.len() > rect.height as usize {
                (lines.len() - rect.height as usize) / 2
            } else {
                0
            };
            let max_offset = lines.len().min(min_offset + rect.height as usize);
            if min_offset > 0 || max_offset < lines.len() {
                lines = lines[min_offset..max_offset].to_vec();
            }
            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph.centered(), rect);
        }
        let planet = world.get_planet_or_err(planet_id)?;
        frame.render_widget(
            default_block().title(format!("Travelling to {} - {}", planet.name, countdown)),
            area,
        );
        Ok(())
    }

    fn render_exploring_spaceship(
        &self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        planet_id: PlanetId,
        countdown: String,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        if let Ok(mut lines) = self
            .gif_map
            .lock()
            .unwrap()
            .exploring_spaceship_lines(team.id, self.tick, world)
        {
            let rect = area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            });
            // Apply y-centering
            let min_offset = if lines.len() > rect.height as usize {
                (lines.len() - rect.height as usize) / 2
            } else {
                0
            };
            let max_offset = lines.len().min(min_offset + rect.height as usize);
            if min_offset > 0 || max_offset < lines.len() {
                lines = lines[min_offset..max_offset].to_vec();
            }
            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph.centered(), rect);
        }
        let planet = world.get_planet_or_err(planet_id)?;
        frame.render_widget(
            default_block().title(format!("Exploring around {} - {}", planet.name, countdown)),
            area,
        );
        Ok(())
    }

    pub fn set_view(&mut self, view: MyTeamView) {
        self.view = view;
    }

    pub fn reset_view(&mut self) {
        self.set_view(MyTeamView::Info);
    }
}

impl Screen for MyTeamPanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        self.own_team_id = world.own_team_id;
        let own_team = world.get_own_team()?;

        self.current_planet_id = match world.get_own_team()?.current_location {
            TeamLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        };

        if self.players.len() != own_team.player_ids.len() || world.dirty_ui {
            self.players = own_team.player_ids.clone();
            self.players.sort_by(|a, b| {
                let a = world.get_player(*a).unwrap();
                let b = world.get_player(*b).unwrap();
                if a.rating() == b.rating() {
                    b.average_skill()
                        .partial_cmp(&a.average_skill())
                        .expect("Skill value should exist")
                } else {
                    b.rating().cmp(&a.rating())
                }
            });
        }

        if self.planet_markets.len() == 0 || world.dirty_ui {
            self.planet_markets = world
                .planets
                .iter()
                .filter(|(_, planet)| planet.total_population() > 0)
                .sorted_by(|(_, a), (_, b)| a.name.cmp(&b.name))
                .map(|(id, _)| id.clone())
                .collect::<Vec<PlanetId>>();
            if self.planet_index.is_none() && self.planet_markets.len() > 0 {
                self.planet_index = Some(0);
            }
        }

        if self.asteroid_ids.len() != own_team.asteroid_ids.len() || world.dirty_ui {
            self.asteroid_ids = own_team.asteroid_ids.clone();
        }

        self.asteroid_index = if self.asteroid_ids.len() > 0 {
            if let Some(index) = self.asteroid_index {
                Some(index % self.asteroid_ids.len())
            } else {
                Some(0)
            }
        } else {
            None
        };

        self.player_index = if self.players.len() > 0 {
            if let Some(index) = self.player_index {
                Some(index % self.players.len())
            } else {
                Some(0)
            }
        } else {
            None
        };

        if world.dirty_ui {
            let mut games = vec![];
            if let Some(current_game) = own_team.current_game {
                games.push(current_game);
            }

            for game in world.past_games.values().sorted_by(|g1, g2| {
                g2.ended_at
                    .unwrap_or_default()
                    .cmp(&g1.ended_at.unwrap_or_default())
            }) {
                games.push(game.id);
            }
            self.recent_games = games;

            self.challenge_teams = world
                .teams
                .keys()
                .into_iter()
                .filter(|&&id| {
                    let team = world.get_team_or_err(id).unwrap();
                    team.can_challenge_team(own_team).is_ok()
                })
                .cloned()
                .collect();
            self.challenge_teams.sort_by(|a, b| {
                let a = world.get_team_or_err(*a).unwrap();
                let b = world.get_team_or_err(*b).unwrap();
                world
                    .team_rating(b.id)
                    .unwrap_or_default()
                    .partial_cmp(&world.team_rating(a.id).unwrap_or_default())
                    .unwrap()
            });
        }

        self.game_index = if self.recent_games.len() > 0 {
            if let Some(index) = self.game_index {
                Some(index % self.recent_games.len())
            } else {
                Some(0)
            }
        } else {
            None
        };

        Ok(())
    }

    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Length(24), Constraint::Min(8)]).split(area);

        if self.callback_registry.lock().unwrap().is_hovering(split[0]) {
            self.active_list = PanelList::Top;
        } else {
            self.active_list = PanelList::Bottom;
        }

        self.render_players_top(frame, world, split[0])?;

        let bottom_split =
            Layout::horizontal([Constraint::Length(32), Constraint::Min(40)]).split(split[1]);

        self.render_view_buttons(frame, bottom_split[0])?;

        match self.view {
            MyTeamView::Info => self.render_info(frame, world, bottom_split[1])?,
            MyTeamView::Games => self.render_games(frame, world, bottom_split[1])?,
            MyTeamView::Market => self.render_market(frame, world, bottom_split[1])?,
            MyTeamView::Shipyard => self.render_shipyard(frame, world, bottom_split[1])?,
            MyTeamView::Asteroids => self.render_asteroids(frame, world, bottom_split[1])?,
        }

        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallbackPreset> {
        if self.players.is_empty() {
            return None;
        }

        match key_event.code {
            KeyCode::Up => {
                self.next_index();
            }
            KeyCode::Down => {
                self.previous_index();
            }
            UiKey::CYCLE_VIEW => {
                return Some(UiCallbackPreset::SetMyTeamPanelView {
                    view: self.view.next(),
                });
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
        if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
            return self.game_index.unwrap_or_default();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
            return self.planet_index.unwrap_or_default();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Shipyard {
            return self.upgrade_index;
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Asteroids {
            return self.asteroid_index.unwrap_or_default();
        }

        // we should always have at least 1 player
        self.player_index.unwrap_or_default()
    }

    fn max_index(&self) -> usize {
        if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
            return self.recent_games.len();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
            return self.planet_markets.len();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Shipyard {
            return 3;
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Asteroids {
            return self.asteroid_ids.len();
        }
        self.players.len()
    }

    fn set_index(&mut self, index: usize) {
        if self.max_index() == 0 {
            if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
                self.game_index = None;
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
                self.planet_index = None;
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Shipyard {
                panic!("Max upgrade_index should be 3");
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Asteroids {
                self.asteroid_index = None;
            } else {
                self.player_index = None;
            }
        } else {
            if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
                self.game_index = Some(index % self.max_index());
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
                self.planet_index = Some(index % self.max_index());
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Shipyard {
                self.upgrade_index = index % self.max_index();
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Asteroids {
                self.asteroid_index = Some(index % self.max_index());
            } else {
                self.player_index = Some(index % self.max_index());
            }
        }
    }
}
