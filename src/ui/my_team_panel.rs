use super::ui_frame::UiFrame;
use super::{
    button::Button,
    clickable_list::ClickableListState,
    clickable_table::{ClickableCell, ClickableRow, ClickableTable, ClickableTableState},
    constants::*,
    gif_map::GifMap,
    traits::{PercentageRating, Screen, SplitPanel, UiStyled},
    ui_callback::UiCallback,
    utils::format_satoshi,
    widgets::*,
};
use crate::world::constants::AsteroidFacilityCost;
use crate::{
    game_engine::game::Game,
    store::load_game,
    types::{AppResult, GameId, PlayerId, StorableResourceMap, SystemTimeTick, Tick},
    world::{
        position::{GamePosition, Position, MAX_POSITION},
        skill::Rated,
        spaceship::{SpaceshipComponent, SpaceshipUpgrade, SpaceshipUpgradeTarget},
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
};
use std::collections::HashMap;
use strum::IntoEnumIterator;

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
    max_player_index: usize,
    game_index: Option<usize>,
    planet_index: Option<usize>,
    spaceship_upgrade_index: usize,
    asteroid_index: Option<usize>,
    view: MyTeamView,
    active_list: PanelList,
    recent_games: Vec<GameId>,
    loaded_games: HashMap<GameId, Game>,
    planet_markets: Vec<PlanetId>,
    challenge_teams: Vec<TeamId>,
    asteroid_ids: Vec<PlanetId>,
    own_team_id: TeamId,
    current_planet_id: Option<PlanetId>,
    tick: usize,
    gif_map: GifMap,
}

impl MyTeamPanel {
    pub fn new() -> Self {
        Self::default()
    }

    fn render_view_buttons(&self, frame: &mut UiFrame, area: Rect) -> AppResult<()> {
        let mut view_info_button = Button::new(
            "Info",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Info,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View team information.");

        let mut view_games_button = Button::new(
            "Games",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Games,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View recent games.");

        let mut view_market_button = Button::new(
            "Market",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Market,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View market, buy and sell resources.");

        let mut view_shipyard_button = Button::new(
            "Shipyard",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Shipyard,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View shipyard, improve your spaceship.");

        let mut view_asteroids_button = Button::new(
            format!("Asteroids ({})", self.asteroid_ids.len()),
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Asteroids,
            },
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View asteorids found during exploration.");

        match self.view {
            MyTeamView::Info => view_info_button.select(),
            MyTeamView::Games => view_games_button.select(),
            MyTeamView::Market => view_market_button.select(),
            MyTeamView::Shipyard => view_shipyard_button.select(),
            MyTeamView::Asteroids => view_asteroids_button.select(),
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

        frame.render_interactive(view_info_button, split[0]);
        frame.render_interactive(view_games_button, split[1]);
        frame.render_interactive(view_market_button, split[2]);
        frame.render_interactive(view_shipyard_button, split[3]);
        frame.render_interactive(view_asteroids_button, split[4]);

        Ok(())
    }

    fn render_market(&self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_planet_markets(frame, world, split[0])?;
        self.render_market_buttons(frame, world, split[1])?;

        Ok(())
    }

    fn render_planet_markets(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        frame.render_widget(default_block().title("Planet Markets"), area);
        let split = Layout::horizontal([Constraint::Length(20), Constraint::Length(30)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let mut options = vec![];
        for id in self.planet_markets.iter() {
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

        let list = selectable_list(options);
        frame.render_stateful_interactive(
            list,
            split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.planet_index),
        );

        let planet_id = self.planet_markets[self.planet_index.unwrap_or_default()];
        let planet = world.get_planet_or_err(&planet_id)?;
        let merchant_bonus = TeamBonus::TradePrice.current_team_bonus(world, &team.id)?;

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(format!("Resource: Buy/Sell"), UiStyle::HEADER)),
                Line::from(vec![
                    Span::styled("Fuel      ", Resource::FUEL.style()),
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
                    Span::styled("Gold      ", Resource::GOLD.style()),
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
                    Span::styled("Scraps    ", Resource::SCRAPS.style()),
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
                    Span::styled("Rum       ", Resource::RUM.style()),
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

    fn render_market_buttons(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
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
            TeamLocation::OnSpaceAdventure { .. } => {
                return Err(anyhow!("Team is on a space adventure"))
            }
        };

        let planet = world.get_planet_or_err(&planet_id)?;
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

        let button_split = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
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

            let merchant_bonus = TeamBonus::TradePrice.current_team_bonus(world, &team.id)?;
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
                    if idx == 0 {
                        Some(buy_ui_keys[button_split_idx])
                    } else {
                        None
                    },
                    UiStyle::OK,
                ) {
                    frame.render_interactive(btn, resource_split[idx + 1]);
                }
            }

            let max_sell_amount = team.max_resource_sell_amount(*resource);
            for (idx, amount) in [1, 10, max_sell_amount as i32].iter().enumerate() {
                if let Ok(btn) = trade_resource_button(
                    &world,
                    *resource,
                    -*amount,
                    sell_unit_cost,
                    if idx == 0 {
                        Some(sell_ui_keys[button_split_idx])
                    } else {
                        None
                    },
                    UiStyle::ERROR,
                ) {
                    frame.render_interactive(btn, resource_split[idx + 4]);
                }
            }
        }

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(format!("Treasury {}", format_satoshi(team.balance()))),
                Line::from(get_fuel_spans(
                    team.fuel(),
                    team.fuel_capacity(),
                    BARS_LENGTH,
                )),
                Line::from(get_storage_spans(
                    &team.resources,
                    team.spaceship.storage_capacity(),
                    BARS_LENGTH,
                )),
            ]),
            button_split[5].inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );

        Ok(())
    }

    fn render_info(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let team = world.get_own_team()?;
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);

        let info = Paragraph::new(vec![
            Line::from(""),
            Line::from(format!(
                "Rating {:5}  Reputation {:5}",
                world.team_rating(&team.id).unwrap_or_default().stars(),
                team.reputation.stars(),
            )),
            Line::from(vec![
                Span::raw(format!(
                    "Game record W{}/L{}/D{}  ",
                    team.game_record[0], team.game_record[1], team.game_record[2]
                )),
                Span::styled(
                    format!(
                        "Network W{}/L{}/D{} ",
                        team.network_game_record[0],
                        team.network_game_record[1],
                        team.network_game_record[2]
                    ),
                    UiStyle::NETWORK,
                ),
            ]),
            Line::from(format!("Treasury {:<10}", format_satoshi(team.balance()),)),
            Line::from(get_crew_spans(team)),
            Line::from(get_durability_spans(
                team.spaceship.current_durability(),
                team.spaceship.durability(),
                BARS_LENGTH,
            )),
            Line::from(get_fuel_spans(
                team.fuel(),
                team.fuel_capacity(),
                BARS_LENGTH,
            )),
            Line::from(get_storage_spans(
                &team.resources,
                team.spaceship.storage_capacity(),
                BARS_LENGTH,
            )),
            Line::from(vec![
                Span::styled(
                    format!("{:>9} ", Resource::GOLD.to_string()),
                    Resource::GOLD.style(),
                ),
                Span::raw(format!("{:>3} Kg  ", team.resources.value(&Resource::GOLD))),
                Span::styled(format!("{:>10} ", "Kartoffeln"), UiStyle::STORAGE_KARTOFFEL),
                Span::raw(format!("{}", team.kartoffel_ids.len())),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("{:>8} ", Resource::RUM.to_string()),
                    Resource::RUM.style(),
                ),
                Span::raw(format!("{:>4} l   ", team.resources.value(&Resource::RUM))),
                Span::styled(
                    format!("{:>6} ", Resource::SCRAPS.to_string()),
                    Resource::SCRAPS.style(),
                ),
                Span::raw(format!("{:>5} t", team.resources.value(&Resource::SCRAPS))),
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

        let tactic_button = Button::new(
            format!("tactic: {}", team.game_tactic),
            UiCallback::SetTeamTactic {
                tactic: team.game_tactic.next(),
            },
        )
        .set_hover_text(format!(
            "Tactics affect the actions the team will choose during the game. {}: {}",
            team.game_tactic,
            team.game_tactic.description()
        ))
        .set_hotkey(UiKey::SET_TACTIC);
        frame.render_interactive(tactic_button, top_button_split[0]);

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
            UiCallback::NextTrainingFocus { team_id: team.id },
        )
        .set_hover_text("Change the training focus, which affects how player skills increase.")
        .set_hotkey(UiKey::TRAINING_FOCUS);
        if can_change_training_focus.is_err() {
            training_button.disable(Some(format!(
                "{}",
                can_change_training_focus.unwrap_err().to_string()
            )));
        }
        frame.render_interactive(training_button, top_button_split[1]);

        let btm_button_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(btm_split[2]);

        if let Ok(go_to_team_current_planet_button) =
            go_to_team_current_planet_button(world, &team.id)
        {
            frame.render_interactive(go_to_team_current_planet_button, btm_button_split[0]);
        }

        if let Ok(home_planet_button) = go_to_team_home_planet_button(world, &team.id) {
            frame.render_interactive(home_planet_button, btm_button_split[1]);
        }

        match team.current_location {
            TeamLocation::OnPlanet { .. } => {
                if let Some(upgrade) = &team.spaceship.pending_upgrade {
                    self.render_upgrading_spaceship(frame, world, split[1], upgrade)?
                } else {
                    self.render_on_planet_spaceship(frame, world, split[1])?
                }
            }
            TeamLocation::Travelling {
                to,
                started,
                duration,
                ..
            } => {
                let countdown = (started + duration)
                    .saturating_sub(world.last_tick_short_interval)
                    .formatted();
                self.render_travelling_spaceship(frame, world, split[1], &to, countdown)?
            }
            TeamLocation::Exploring {
                around,
                started,
                duration,
                ..
            } => {
                let countdown = (started + duration)
                    .saturating_sub(world.last_tick_short_interval)
                    .formatted();
                self.render_exploring_spaceship(frame, world, split[1], &around, countdown)?
            }
            TeamLocation::OnSpaceAdventure { .. } => {
                return Err(anyhow!("Team is on a space adventure"))
            }
        }
        Ok(())
    }

    fn render_games(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_challenge_teams(frame, world, split[0])?;
        self.render_recent_games(frame, world, split[1])?;
        Ok(())
    }

    fn render_challenge_teams(
        &self,
        frame: &mut UiFrame,
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
            let game = world.get_game_or_err(&game_id)?;
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
            frame.render_interactive(
                Button::new(
                    format!("Playing - {}", game_text),
                    UiCallback::GoToGame { game_id },
                )
                .set_hover_text("Go to current game")
                .set_hotkey(UiKey::GO_TO_GAME)
                .block(default_block().border_style(UiStyle::OWN_TEAM)),
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

        for (idx, team_id) in self
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

            render_challenge_button(world, team, idx == 0, frame, right_split[idx])?;
        }

        Ok(())
    }

    fn render_recent_games(
        &mut self,
        frame: &mut UiFrame,
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
        let list = selectable_list(options);

        frame.render_stateful_interactive(
            list,
            split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.game_index),
        );

        let game_id = self.recent_games[self.game_index.unwrap()];
        let summary = if let Ok(current_game) = world.get_game_or_err(&game_id) {
            Paragraph::new(format!(
                "Location {} - Attendance {}\nCurrently playing: {}",
                world.get_planet_or_err(&current_game.location)?.name,
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
                    world.get_planet_or_err(&game.location)?.name,
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

    fn render_shipyard(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_shipyard_upgrades(frame, world, split[0])?;

        let team = world.get_own_team()?;
        match team.current_location {
            TeamLocation::OnPlanet { .. } => {
                if let Some(upgrade) = &team.spaceship.pending_upgrade {
                    self.render_upgrading_spaceship(frame, world, split[1], upgrade)?
                } else {
                    self.render_in_shipyard_spaceship(frame, world, split[1])?
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
                self.render_travelling_spaceship(frame, world, split[1], &to, countdown)?
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
                self.render_exploring_spaceship(frame, world, split[1], &around, countdown)?
            }
            TeamLocation::OnSpaceAdventure { .. } => {
                return Err(anyhow!("Team is on a space adventure"))
            }
        }

        Ok(())
    }

    fn render_shipyard_upgrades(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(20), Constraint::Length(30)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );
        let team = world.get_own_team()?;
        frame.render_widget(default_block().title("Upgrades "), area);

        let target = match self.spaceship_upgrade_index {
            0 => SpaceshipUpgradeTarget::Hull {
                component: team.spaceship.hull.next(),
            },
            1 => SpaceshipUpgradeTarget::Engine {
                component: team.spaceship.engine.next(),
            },
            2 => SpaceshipUpgradeTarget::Storage {
                component: team.spaceship.storage.next(),
            },
            3 => SpaceshipUpgradeTarget::Shooter {
                component: team.spaceship.shooter.next(),
            },
            4 => SpaceshipUpgradeTarget::Repairs {
                amount: team.spaceship.durability() - team.spaceship.current_durability(),
            },
            _ => unreachable!(),
        };

        let can_be_upgraded = match self.spaceship_upgrade_index {
            0 => team.spaceship.hull.can_be_upgraded(),
            1 => team.spaceship.engine.can_be_upgraded(),
            2 => team.spaceship.storage.can_be_upgraded(),
            3 => team.spaceship.shooter.can_be_upgraded(),
            4 => team.spaceship.can_be_repaired(),
            _ => unreachable!(),
        };

        let options = SpaceshipUpgradeTarget::iter()
            .map(|upgrade_target| {
                (
                    upgrade_target.to_string(),
                    if team.spaceship.can_be_upgraded(upgrade_target) {
                        UiStyle::DEFAULT
                    } else {
                        UiStyle::UNSELECTABLE
                    },
                )
            })
            .collect_vec();

        let list = selectable_list(options);

        frame.render_stateful_interactive(
            list,
            split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(Some(self.spaceship_upgrade_index)),
        );

        let current = match self.spaceship_upgrade_index {
            0 => team.spaceship.hull.to_string(),
            1 => team.spaceship.engine.to_string(),
            2 => team.spaceship.storage.to_string(),
            3 => team.spaceship.shooter.to_string(),
            4 => {
                format!("Repairs {}", team.spaceship.current_durability())
            }
            _ => unreachable!(),
        };

        let next = match self.spaceship_upgrade_index {
            0 => team.spaceship.hull.next().to_string(),
            1 => team.spaceship.engine.next().to_string(),
            2 => team.spaceship.storage.next().to_string(),
            3 => team.spaceship.shooter.next().to_string(),
            4 => team.spaceship.durability().to_string(),
            _ => unreachable!(),
        };

        let is_being_upgraded = team.spaceship.pending_upgrade.is_some();

        let upgrade_to_text = match team.spaceship.pending_upgrade.as_ref() {
            Some(upgrade) => match upgrade.target {
                SpaceshipUpgradeTarget::Repairs { .. } => "Currently repairing".to_string(),
                _ => "Currently upgrading".to_string(),
            },
            None => {
                if can_be_upgraded {
                    format!("{} -> {}", current, next)
                } else {
                    if self.spaceship_upgrade_index == SpaceshipUpgradeTarget::MAX_INDEX - 1 {
                        "Fully repaired".to_string()
                    } else {
                        "Fully upgraded".to_string()
                    }
                }
            }
        };

        let header_text = match self.spaceship_upgrade_index {
            0 => "Upgrade Hull",
            1 => "Upgrade Engine",
            2 => "Upgrade Storage",
            3 => "Upgrade Shooter",
            4 => "Repair",
            _ => unreachable!(),
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(header_text, UiStyle::HEADER)).centered(),
            Line::from(Span::raw(upgrade_to_text)).centered(),
            Line::from(""),
        ];

        if can_be_upgraded && !is_being_upgraded {
            let upgrade = SpaceshipUpgrade::new(target);
            for (resource, amount) in upgrade.cost().iter() {
                let have = team.resources.value(resource);
                let style = if amount.clone() > have {
                    UiStyle::ERROR
                } else {
                    UiStyle::OK
                };

                lines.push(Line::from(vec![
                    Span::styled(format!("  {:<7} ", resource.to_string()), resource.style()),
                    Span::styled(format!("{}/{}", amount, have,), style),
                ]));
            }
        }

        frame.render_widget(Paragraph::new(lines), split[1]);

        Ok(())
    }

    fn render_asteroids(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_asteroid_list(frame, world, split[0])?;
        self.render_selected_asteroid(frame, world, split[1])?;
        Ok(())
    }

    fn render_asteroid_list(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        frame.render_widget(default_block().title("Asteroids "), area);

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

        let split = Layout::horizontal([Constraint::Length(20), Constraint::Length(30)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let own_team = world.get_own_team()?;

        let options = self
            .asteroid_ids
            .iter()
            .filter(|&asteroid_id| world.get_planet_or_err(asteroid_id).is_ok())
            .map(|&asteroid_id| {
                let asteroid = world.get_planet_or_err(&asteroid_id).unwrap();
                let style = match own_team.current_location {
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

        let list = selectable_list(options);

        frame.render_stateful_interactive(
            list,
            split[0],
            &mut ClickableListState::default().with_selected(self.asteroid_index),
        );

        let a_split = Layout::vertical([3, 3, 3]).split(split[1]);
        if let Some(index) = self.asteroid_index {
            let asteroid_id = own_team.asteroid_ids[index];
            let asteroid = world.get_planet_or_err(&asteroid_id)?;
            if own_team.can_teleport_to(asteroid_id) {
                let mut travel_to_planet_button = Button::new(
                    "Teleport",
                    UiCallback::TravelToPlanet {
                        planet_id: asteroid_id,
                    },
                )
                .set_hotkey(UiKey::TRAVEL)
                .set_hover_text(format!("Travel instantaneously to {}", asteroid.name));

                let duration = world.travel_time_to_planet(own_team.id, asteroid_id)?;
                if let Err(e) = own_team.can_travel_to_planet(&asteroid, duration) {
                    travel_to_planet_button.disable(Some(e.to_string()));
                }

                frame.render_interactive(travel_to_planet_button, a_split[0]);
            } else {
                let mut build_teleport_pod_button = Button::new(
                    "Build teleportation pod",
                    UiCallback::BuildTeleportationPod { asteroid_id },
                )
                .set_hotkey(UiKey::BUILD_TELEPORTATION_POD)
                .set_hover_text(format!(
                    "Build teleportation pad to travel instantaneously to {}",
                    asteroid.name
                ));

                if own_team.extra_teleportation_pods.contains(&asteroid_id) {
                    log::error!("Asteroid teleportation pad: This path should be unreachable");
                    build_teleport_pod_button.disable(Some("Teleportation pad already built"));
                } else {
                    for (resource, amount) in AsteroidFacilityCost::TELEPORTATION_POD {
                        if own_team.resources.value(&resource) < amount {
                            build_teleport_pod_button.disable(Some(format!(
                                "Not enough {} ({}/{})",
                                resource,
                                own_team.resources.value(&resource),
                                amount
                            )));
                            break;
                        }
                    }
                }

                frame.render_interactive(build_teleport_pod_button, a_split[0]);
            }
        }

        Ok(())
    }

    fn render_selected_asteroid(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if self.asteroid_ids.len() == 0 {
            frame.render_widget(default_block(), area);
            return Ok(());
        }

        let asteroid_id = self.asteroid_ids[self.asteroid_index.unwrap_or_default()];
        let asteroid = world.get_planet_or_err(&asteroid_id)?;

        let parent = world.get_planet_or_err(
            &asteroid
                .satellite_of
                .expect("Asteroid should orbit a planet"),
        )?;

        frame.render_widget(
            default_block().title(format!("{} (around {})", asteroid.name, parent.name)),
            area,
        );

        let split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let img_lines = self
            .gif_map
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

        frame.render_interactive(go_to_planet_button(world, &asteroid_id)?, b_split[0]);

        let abandon_asteroid_button =
            Button::new("Abandon", UiCallback::PromptAbandonAsteroid { asteroid_id })
                .set_hotkey(UiKey::ABANDON_ASTEROID)
                .set_hover_text("Abandon this asteroid (there's no way back!)");
        frame.render_interactive(abandon_asteroid_button, b_split[1]);

        Ok(())
    }

    fn render_player_buttons(
        &self,
        players: Vec<PlayerId>,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        let player_index = if let Some(index) = self.player_index {
            index
        } else {
            return Ok(());
        };

        let player_id = players[player_index];
        let player = world.get_player_or_err(&player_id)?;
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

        let can_set_crew_role = team.can_set_crew_role(&player);

        let mut captain_button = Button::new(
            "captain",
            UiCallback::SetCrewRole {
                player_id,
                role: CrewRole::Captain,
            },
        )
        .set_hover_text(format!(
            "Set player to captain role: {} +{}%, {} {}%",
            TeamBonus::Reputation,
            TeamBonus::Reputation.as_skill(player)?.percentage(),
            TeamBonus::TradePrice,
            TeamBonus::TradePrice.as_skill(player)?.percentage()
        ))
        .set_hotkey(UiKey::SET_CAPTAIN);
        if team.crew_roles.captain == Some(player.id) {
            captain_button = captain_button
                .set_hover_text(format!("Remove player from captain role"))
                .selected();
        } else if let Err(e) = can_set_crew_role.as_ref() {
            captain_button.disable(Some(e.to_string()));
        }
        frame.render_interactive(captain_button, button_splits[0]);

        let mut pilot_button = Button::new(
            "pilot",
            UiCallback::SetCrewRole {
                player_id,
                role: CrewRole::Pilot,
            },
        )
        .set_hover_text(format!(
            "Set player to pilot role: {} +{}%, {} {}%",
            TeamBonus::SpaceshipSpeed,
            TeamBonus::SpaceshipSpeed.as_skill(player)?.percentage(),
            TeamBonus::Exploration,
            TeamBonus::Exploration.as_skill(player)?.percentage()
        ))
        .set_hotkey(UiKey::SET_PILOT);
        if team.crew_roles.pilot == Some(player.id) {
            pilot_button = pilot_button
                .set_hover_text(format!("Remove player from pilot role"))
                .selected();
        } else if let Err(e) = can_set_crew_role.as_ref() {
            pilot_button.disable(Some(e.to_string()));
        }
        frame.render_interactive(pilot_button, button_splits[1]);

        let mut doctor_button = Button::new(
            "doctor",
            UiCallback::SetCrewRole {
                player_id,
                role: CrewRole::Doctor,
            },
        )
        .set_hover_text(format!(
            "Set player to doctor role: {} +{}%, {} {}%",
            TeamBonus::TirednessRecovery,
            TeamBonus::TirednessRecovery.as_skill(player)?.percentage(),
            TeamBonus::Training,
            TeamBonus::Training.as_skill(player)?.percentage()
        ))
        .set_hotkey(UiKey::SET_DOCTOR);
        if team.crew_roles.doctor == Some(player.id) {
            doctor_button = doctor_button
                .set_hover_text(format!("Remove player from doctor role"))
                .selected();
        } else if let Err(e) = can_set_crew_role.as_ref() {
            doctor_button.disable(Some(e.to_string()));
        }
        frame.render_interactive(doctor_button, button_splits[2]);

        let can_release = team.can_release_player(&player);
        let mut release_button = Button::new(
            format!("Fire {}", player.info.shortened_name()),
            UiCallback::PromptReleasePlayer { player_id },
        )
        .set_hover_text("Fire pirate from the crew!")
        .set_hotkey(UiKey::FIRE);
        if can_release.is_err() {
            release_button.disable(Some(format!("{}", can_release.unwrap_err().to_string())));
        }

        frame.render_interactive(release_button, button_splits[3]);

        if let Ok(drink_button) = drink_button(world, &player_id) {
            frame.render_interactive(drink_button, button_splits[4]);
        }

        Ok(())
    }

    fn build_players_table(
        &self,
        players: &Vec<PlayerId>,
        world: &World,
        table_width: u16,
    ) -> AppResult<ClickableTable> {
        let team = world.get_own_team().unwrap();
        let header_cells = [
            " Name",
            "Overall",
            "Potential",
            "Current",
            "Best",
            "Role",
            "Crew bonus",
        ]
        .iter()
        .map(|h| ClickableCell::from(*h).style(UiStyle::HEADER));
        let header = ClickableRow::new(header_cells);

        // Calculate the available space for the players name in order to display the
        // full or shortened version.
        let name_header_width = table_width - (9 + 10 + 10 + 10 + 9 + 15 + 17);

        let rows = players
            .iter()
            .map(|id| {
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
                    None => unreachable!("Player in MyTeam should have a position."),
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
                            format!(" {} +{}%", TeamBonus::Exploration, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Captain => {
                        let skill = TeamBonus::TradePrice.as_skill(player)?;
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::TradePrice, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Doctor => {
                        let skill = TeamBonus::Training.as_skill(player)?;
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::Training, skill.percentage()),
                            skill.style(),
                        )
                    }
                    _ => Span::raw(" "),
                };

                let name = if name_header_width >= 2 * MAX_NAME_LENGTH as u16 + 2 {
                    player.info.full_name()
                } else {
                    player.info.shortened_name()
                };

                let cells = [
                    ClickableCell::from(name),
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

        let table = ClickableTable::new(rows?)
            .header(header)
            .column_spacing(0)
            .widths(&[
                Constraint::Min(MAX_NAME_LENGTH as u16 + 4),
                Constraint::Length(9),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(9),
                Constraint::Length(15),
                Constraint::Length(17),
            ]);

        Ok(table)
    }

    fn render_players_top(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let mut players = own_team.player_ids.clone();
        players.sort_by(|a, b| {
            let a = world.get_player(a).unwrap();
            let b = world.get_player(b).unwrap();
            if a.rating() == b.rating() {
                b.average_skill()
                    .partial_cmp(&a.average_skill())
                    .expect("Skill value should exist")
            } else {
                b.rating().cmp(&a.rating())
            }
        });
        let top_split =
            Layout::horizontal([Constraint::Min(10), Constraint::Length(60)]).split(area);

        let table = self.build_players_table(&players, world, top_split[0].width)?;
        frame.render_stateful_interactive(
            table.block(default_block().title(format!(
                "{} {} ↓/↑",
                own_team.name.clone(),
                world.team_rating(&own_team.id).unwrap_or_default().stars()
            ))),
            top_split[0],
            &mut ClickableTableState::default().with_selected(self.player_index),
        );

        if self.player_index.is_none() {
            return Ok(());
        }
        let player_id = players[self.player_index.unwrap()];

        let player = world
            .get_player(&player_id)
            .ok_or(anyhow!("Player {:?} not found", player_id))?;

        render_player_description(
            player,
            &mut self.gif_map,
            self.tick,
            world,
            frame,
            top_split[1],
        );

        if own_team.current_game.is_none() {
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
                    UiCallback::SwapPlayerPositions {
                        player_id,
                        position: idx,
                    },
                )
                .set_hover_text(format!(
                    "Set player initial position to {}.",
                    position.as_str()
                ))
                .set_hotkey(UiKey::set_player_position(idx as Position));

                let position = own_team.player_ids.iter().position(|id| *id == player.id);
                if position.is_some() && position.unwrap() == idx {
                    button.select();
                }
                frame.render_interactive(button, rect);
            }

            let auto_assign_button =
                Button::new("Auto-assign positions", UiCallback::AssignBestTeamPositions)
                    .set_hover_text("Auto-assign players' initial position.")
                    .set_hotkey(UiKey::AUTO_ASSIGN);
            frame.render_interactive(auto_assign_button, position_button_splits[6]);
            self.render_player_buttons(players, frame, world, table_bottom[2])?;
        }

        Ok(())
    }

    fn render_on_planet_spaceship(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;

        let split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );
        render_spaceship_description(
            &team,
            world.team_rating(&team.id).unwrap_or_default(),
            true,
            &mut self.gif_map,
            self.tick,
            frame,
            area,
        );

        let explore_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(split[1]);
        if let Ok(explore_button) = space_adventure_button(world, team) {
            frame.render_interactive(explore_button, explore_split[0]);
        }
        if let Ok(explore_button) = explore_button(world, team) {
            frame.render_interactive(explore_button, explore_split[1]);
        }
        Ok(())
    }

    fn render_upgrading_spaceship(
        &mut self,
        frame: &mut UiFrame,
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
            true,
            &mut self.gif_map,
            self.tick,
            frame,
            area,
        );

        frame.render_widget(
            default_block().title(format!("{} - {}", upgrade.description(), countdown)),
            area,
        );

        Ok(())
    }

    fn render_in_shipyard_spaceship(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;

        let target = match self.spaceship_upgrade_index {
            0 if team.spaceship.hull.can_be_upgraded() => Some(SpaceshipUpgradeTarget::Hull {
                component: team.spaceship.hull.next(),
            }),
            1 if team.spaceship.engine.can_be_upgraded() => Some(SpaceshipUpgradeTarget::Engine {
                component: team.spaceship.engine.next(),
            }),
            2 if team.spaceship.storage.can_be_upgraded() => {
                Some(SpaceshipUpgradeTarget::Storage {
                    component: team.spaceship.storage.next(),
                })
            }
            3 if team.spaceship.shooter.can_be_upgraded() => {
                Some(SpaceshipUpgradeTarget::Shooter {
                    component: team.spaceship.shooter.next(),
                })
            }
            4 if team.spaceship.can_be_repaired() => Some(SpaceshipUpgradeTarget::Repairs {
                amount: team.spaceship.durability() - team.spaceship.current_durability(),
            }),
            _ => None,
        };

        if let Some(target) = target {
            let upgrade = SpaceshipUpgrade::new(target);
            render_spaceship_upgrade(
                &team,
                &upgrade,
                false,
                &mut self.gif_map,
                self.tick,
                frame,
                area,
            );
            frame.render_widget(default_block().title(upgrade.description()), area);

            if let Ok(upgrade_button) = upgrade_spaceship_button(team, upgrade) {
                let split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
                    area.inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    }),
                );
                frame.render_interactive(upgrade_button, split[1]);
            }
        } else {
            render_spaceship_description(
                &team,
                world.team_rating(&team.id).unwrap_or_default(),
                true,
                &mut self.gif_map,
                self.tick,
                frame,
                area,
            );
        }

        Ok(())
    }

    fn render_travelling_spaceship(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        planet_id: &PlanetId,
        countdown: String,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        if let Ok(mut lines) = self
            .gif_map
            .travelling_spaceship_lines(&team.spaceship, self.tick)
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
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        planet_id: &PlanetId,
        countdown: String,
    ) -> AppResult<()> {
        let team = world.get_own_team()?;
        if let Ok(mut lines) = self
            .gif_map
            .exploring_spaceship_lines(&team.spaceship, self.tick)
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

        self.player_index = if own_team.player_ids.len() > 0 {
            if let Some(index) = self.player_index {
                Some(index % own_team.player_ids.len())
            } else {
                Some(0)
            }
        } else {
            None
        };

        self.max_player_index = own_team.player_ids.len();

        if world.dirty_ui {
            let mut games = vec![];
            if let Some(current_game) = own_team.current_game {
                games.push(current_game);
            }

            for game in world
                .past_games
                .values()
                .filter(|g| g.home_team_id == own_team.id || g.away_team_id == own_team.id)
                .sorted_by(|g1, g2| {
                    g2.ended_at
                        .unwrap_or_default()
                        .cmp(&g1.ended_at.unwrap_or_default())
                })
            {
                games.push(game.id);
            }
            self.recent_games = games;

            self.challenge_teams = world
                .teams
                .keys()
                .into_iter()
                .filter(|&id| {
                    let team = world.get_team_or_err(id).unwrap();
                    team.can_challenge_team(own_team).is_ok()
                })
                .cloned()
                .collect();
            self.challenge_teams.sort_by(|a, b| {
                let a = world.get_team_or_err(a).unwrap();
                let b = world.get_team_or_err(b).unwrap();
                world
                    .team_rating(&b.id)
                    .unwrap_or_default()
                    .partial_cmp(&world.team_rating(&a.id).unwrap_or_default())
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

    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Length(24), Constraint::Min(8)]).split(area);

        if frame.is_hovering(split[0]) {
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
    ) -> Option<UiCallback> {
        if self.planet_index.is_none() {
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
                return Some(UiCallback::SetMyTeamPanelView {
                    view: self.view.next(),
                });
            }
            _ => {}
        }

        None
    }
}

impl SplitPanel for MyTeamPanel {
    fn index(&self) -> usize {
        if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
            return self.game_index.unwrap_or_default();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
            return self.planet_index.unwrap_or_default();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Shipyard {
            return self.spaceship_upgrade_index;
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
            return SpaceshipUpgradeTarget::MAX_INDEX;
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Asteroids {
            return self.asteroid_ids.len();
        }
        self.max_player_index
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
                self.spaceship_upgrade_index = index % self.max_index();
            } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Asteroids {
                self.asteroid_index = Some(index % self.max_index());
            } else {
                self.player_index = Some(index % self.max_index());
            }
        }
    }
}
