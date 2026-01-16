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
use crate::game_engine::timer::Period;
use crate::types::{HashMapWithResult, Tick};
use crate::ui::popup_message::PopupMessage;
use crate::ui::ui_key;
use crate::{
    core::*,
    game_engine::game::Game,
    store::load_game,
    types::{AppResult, GameId, PlanetId, StorableResourceMap, SystemTimeTick, TeamId},
};
use anyhow::anyhow;
use core::fmt::Debug;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::style::Stylize;
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
    Team,
    Games,
    Market,
    Shipyard,
    Asteroids,
}

impl MyTeamView {
    fn next(&self) -> Self {
        match self {
            MyTeamView::Info => MyTeamView::Team,
            MyTeamView::Team => MyTeamView::Games,
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
    player_widget_view: PlayerWidgetView,
    active_list: PanelList,
    past_game_ids: Vec<GameId>,
    loaded_games: HashMap<GameId, AppResult<Game>>,
    planet_markets: Vec<PlanetId>,
    challenge_teams: Vec<TeamId>,
    asteroid_ids: Vec<PlanetId>,
    own_team_id: TeamId,
    current_planet_id: Option<PlanetId>,
    tick: usize,
    gif_map: GifMap,
    // players_table: ClickableTable<'static>,
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
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View crew information.");

        let mut view_team_button = Button::new(
            "Team",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Team,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View team information.");

        let mut view_games_button = Button::new(
            "Games",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Games,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View recent games.");

        let mut view_market_button = Button::new(
            "Market",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Market,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View market, buy and sell resources.");

        let mut view_shipyard_button = Button::new(
            "Shipyard",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Shipyard,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View shipyard, improve your spaceship.");

        let mut view_asteroids_button = Button::new(
            format!(
                "Asteroids ({}{})",
                self.asteroid_ids.len(),
                if self.asteroid_ids.len() == MAX_NUM_ASTEROID_PER_TEAM {
                    " MAX"
                } else {
                    ""
                }
            ),
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Asteroids,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View asteorids found during exploration.");

        match self.view {
            MyTeamView::Info => view_info_button.select(),
            MyTeamView::Team => view_team_button.select(),
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
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

        frame.render_interactive_widget(view_info_button, split[0]);
        frame.render_interactive_widget(view_team_button, split[1]);
        frame.render_interactive_widget(view_games_button, split[2]);
        frame.render_interactive_widget(view_market_button, split[3]);
        frame.render_interactive_widget(view_shipyard_button, split[4]);
        frame.render_interactive_widget(view_asteroids_button, split[5]);

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
        let own_team = world.get_own_team()?;
        frame.render_widget(default_block().title("Planet Markets"), area);
        let split = Layout::horizontal([Constraint::Length(20), Constraint::Length(30)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let mut options = vec![];
        for id in self.planet_markets.iter() {
            let planet = world.planets.get_or_err(id)?;
            let text = planet.name.clone();
            let style = match own_team.current_location {
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
        frame.render_stateful_interactive_widget(
            list,
            split[0].inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.planet_index),
        );

        let planet_id =
            self.planet_markets[self.planet_index.unwrap_or_default() % self.planet_markets.len()];
        let planet = world.planets.get_or_err(&planet_id)?;
        let merchant_bonus = TeamBonus::TradePrice.current_team_bonus(world, &own_team.id)?;

        let mut lines = vec![Line::from(Span::styled(
            format!("{:<8} {:>4}/{:<4}", "Resource", "Buy", "Sell"),
            UiStyle::HEADER.bold(),
        ))];
        for resource in Resource::iter() {
            if resource == Resource::SATOSHI {
                continue;
            }

            let line = vec![
                Span::styled(format!("{:<8} ", resource.to_string()), resource.style()),
                Span::styled(
                    format!("{:>4}", planet.resource_buy_price(resource, merchant_bonus)),
                    UiStyle::OK,
                ),
                Span::raw("/"),
                Span::styled(
                    format!(
                        "{:<4}",
                        planet.resource_sell_price(resource, merchant_bonus)
                    ),
                    UiStyle::ERROR,
                ),
            ];
            lines.push(line.into());
        }

        frame.render_widget(
            Paragraph::new(lines),
            split[1].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        Ok(())
    }

    fn render_market_buttons(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let inner_area = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        let planet_id = match own_team.current_location {
            TeamLocation::OnPlanet { planet_id } => planet_id,
            TeamLocation::Travelling { .. } => {
                frame.render_widget(default_block().title("Market"), area);
                frame.render_widget(
                    Paragraph::new("There is no market available while travelling.").centered(),
                    inner_area,
                );
                return Ok(());
            }
            TeamLocation::Exploring { .. } => {
                frame.render_widget(default_block().title("Market"), area);
                frame.render_widget(
                    Paragraph::new("There is no market available while exploring.").centered(),
                    inner_area,
                );
                return Ok(());
            }
            TeamLocation::OnSpaceAdventure { .. } => {
                return Err(anyhow!("Team is on a space adventure"))
            }
        };

        let planet = world.planets.get_or_err(&planet_id)?;
        frame.render_widget(
            default_block().title(format!("Planet {} Market", planet.name)),
            area,
        );
        if planet.total_population() == 0 {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from("There is no market available on this planet."),
                    Line::from("Try another planet with more population."),
                ])
                .centered(),
                inner_area,
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
        .split(inner_area.inner(Margin {
            horizontal: 3,
            vertical: 0,
        }));

        let layout = Layout::horizontal([
            Constraint::Length(12), // name
            Constraint::Max(6),     // buy 1
            Constraint::Max(6),     // buy 10
            Constraint::Max(6),     // buy 100
            Constraint::Max(6),     // sell 1
            Constraint::Max(6),     // sell 10
            Constraint::Max(6),     // sell 100
            Constraint::Length(11), // price
            Constraint::Min(0),     // have
        ]);

        frame.render_widget(
            Paragraph::new(Span::styled(
                "        Key        Buy               Sell         Prices    In Stiva".to_string(),
                UiStyle::HEADER.bold(),
            )),
            button_split[0],
        );

        let buy_ui_keys = [
            ui_key::market::BUY_GOLD,
            ui_key::market::BUY_SCRAPS,
            ui_key::market::BUY_FUEL,
            ui_key::market::BUY_RUM,
        ];
        let sell_ui_keys = [
            ui_key::market::SELL_GOLD,
            ui_key::market::SELL_SCRAPS,
            ui_key::market::SELL_FUEL,
            ui_key::market::SELL_RUM,
        ];

        for (button_split_idx, resource) in [
            Resource::GOLD,
            Resource::SCRAPS,
            Resource::FUEL,
            Resource::RUM,
        ]
        .iter()
        .enumerate()
        {
            let resource_split = layout.split(button_split[button_split_idx + 1]);
            let merchant_bonus = TeamBonus::TradePrice.current_team_bonus(world, &own_team.id)?;
            let buy_unit_cost = planet.resource_buy_price(*resource, merchant_bonus);
            let sell_unit_cost = planet.resource_sell_price(*resource, merchant_bonus);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{:<6} ", resource.to_string()), resource.style()),
                    Span::styled(format!("{}", buy_ui_keys[button_split_idx]), UiStyle::OK),
                    Span::raw("/".to_string()),
                    Span::styled(
                        format!("{}", sell_ui_keys[button_split_idx]),
                        UiStyle::ERROR,
                    ),
                ])),
                resource_split[0].inner(Margin::new(1, 1)),
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{buy_unit_cost:>4}"), UiStyle::OK),
                    Span::raw("/".to_string()),
                    Span::styled(format!("{sell_unit_cost:<4}"), UiStyle::ERROR),
                ])),
                resource_split[7].inner(Margin::new(1, 1)),
            );

            frame.render_widget(
                Paragraph::new(format!(
                    "{:^7}",
                    own_team.resources.value(resource).to_string()
                )),
                resource_split[8].inner(Margin::new(1, 1)),
            );

            let max_buy_amount = own_team.max_resource_buy_amount(*resource, buy_unit_cost);
            for (idx, amount) in [1, 10, 100.min(max_buy_amount) as i32].iter().enumerate() {
                if let Ok(btn) = trade_resource_button(
                    world,
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
                    frame.render_interactive_widget(btn, resource_split[idx + 1]);
                }
            }

            let max_sell_amount = own_team.max_resource_sell_amount(*resource);
            for (idx, amount) in [1, 10, 100.min(max_sell_amount) as i32].iter().enumerate() {
                if let Ok(btn) = trade_resource_button(
                    world,
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
                    frame.render_interactive_widget(btn, resource_split[idx + 4]);
                }
            }
        }

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format!("Treasury {}", format_satoshi(own_team.balance()))),
                Line::from(get_fuel_spans(
                    own_team.fuel(),
                    own_team.fuel_capacity(),
                    BARS_LENGTH,
                )),
                Line::from(get_storage_spans(
                    &own_team.resources,
                    own_team.spaceship.storage_capacity(),
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
        let own_team = world.get_own_team()?;
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);

        let info = Paragraph::new(vec![
            Line::default(),
            Line::from(format!(
                "Rating {:5}  Reputation {:5}",
                world.team_rating(&own_team.id).unwrap_or_default().stars(),
                own_team.reputation.stars(),
            )),
            Line::from(vec![
                Span::raw(format!(
                    "Local Elo {:.0}",
                    own_team.local_game_rating.rating
                )),
                Span::styled(
                    format!("  Network Elo {:.0}", own_team.network_game_rating.rating),
                    UiStyle::NETWORK,
                ),
            ]),
            Line::from(format!(
                "Treasury {:<10}",
                format_satoshi(own_team.balance()),
            )),
            Line::from(get_crew_spans(
                own_team.player_ids.len(),
                own_team.spaceship.crew_capacity() as usize,
            )),
            Line::from(get_durability_spans(
                own_team.spaceship.current_durability(),
                own_team.spaceship.max_durability(),
                own_team.spaceship.shield_max_durability() as u32,
                own_team.spaceship.shield_max_durability() as u32,
                BARS_LENGTH,
            )),
            Line::from(get_fuel_spans(
                own_team.fuel(),
                own_team.fuel_capacity(),
                BARS_LENGTH,
            )),
            Line::from(get_storage_spans(
                &own_team.resources,
                own_team.spaceship.storage_capacity(),
                BARS_LENGTH,
            )),
            Line::from(vec![
                Span::styled(
                    format!("       {:<6} ", Resource::GOLD.to_string()),
                    Resource::GOLD.style(),
                ),
                Span::raw(format!(
                    "{:>4} Kg * {:>2} u/Kg = {:>4} u",
                    own_team.resources.value(&Resource::GOLD),
                    Resource::GOLD.to_storing_space(),
                    own_team.resources.value(&Resource::GOLD) * Resource::GOLD.to_storing_space()
                )),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("       {:<6} ", Resource::SCRAPS.to_string()),
                    Resource::SCRAPS.style(),
                ),
                Span::raw(format!(
                    "{:>4} t  * {:>2} u/t  = {:>4} u",
                    own_team.resources.value(&Resource::SCRAPS),
                    Resource::SCRAPS.to_storing_space(),
                    own_team.resources.value(&Resource::SCRAPS)
                        * Resource::SCRAPS.to_storing_space()
                )),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("       {:<6} ", Resource::RUM.to_string()),
                    Resource::RUM.style(),
                ),
                Span::raw(format!(
                    "{:>4} l  * {:>2} u/l  = {:>4} u",
                    own_team.resources.value(&Resource::RUM),
                    Resource::RUM.to_storing_space(),
                    own_team.resources.value(&Resource::RUM) * Resource::RUM.to_storing_space()
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

        let btm_split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            split[0].inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let btm_button_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(btm_split[1]);

        if let Ok(go_to_team_current_planet_button) =
            go_to_team_current_planet_button(world, &own_team.id)
        {
            frame.render_interactive_widget(go_to_team_current_planet_button, btm_button_split[0]);
        }

        if let Ok(home_planet_button) = go_to_team_home_planet_button(world, &own_team.id) {
            frame.render_interactive_widget(home_planet_button, btm_button_split[1]);
        }

        match own_team.current_location {
            TeamLocation::OnPlanet { .. } => {
                if let Some(upgrade) = &own_team.spaceship.pending_upgrade {
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

    fn render_team(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);

        frame.render_widget(default_block().title("Team"), split[0]);

        let btm_split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(split[0].inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        let top_button_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(btm_split[0]);

        let tactic_button = Button::new(
            format!("tactic: {}", own_team.game_tactic),
            UiCallback::SetTeamTactic {
                tactic: own_team.game_tactic.next(),
            },
        )
        .set_hover_text(format!(
            "{}: {}",
            own_team.game_tactic,
            own_team.game_tactic.description()
        ))
        .set_hotkey(ui_key::team::SET_TACTIC);
        frame.render_interactive_widget(tactic_button, top_button_split[0]);

        let can_change_training_focus = own_team.can_change_training_focus();
        let mut training_button = Button::new(
            format!(
                "Training: {}",
                if let Some(focus) = own_team.training_focus {
                    focus.to_string()
                } else {
                    "General".to_string()
                }
            ),
            UiCallback::NextTrainingFocus {
                team_id: own_team.id,
            },
        )
        .set_hover_text("Change the training focus, which skills increase faster.")
        .set_hotkey(ui_key::team::TRAINING_FOCUS);
        if let Err(err) = can_change_training_focus {
            training_button.disable(Some(err.to_string()));
        }
        frame.render_interactive_widget(training_button, top_button_split[1]);

        let local_challenge_button = Button::new(
            format!(
                "Auto-accept local challenges: {}",
                if own_team.autonomous_strategy.challenge_local {
                    "on"
                } else {
                    "off"
                }
            ),
            UiCallback::ToggleTeamAutonomousStrategyForLocalChallenges,
        )
        .set_hover_text("Accept challenges from local teams automatically.".to_string())
        .set_hotkey(ui_key::team::TOGGLE_ACCEPT_LOCAL_CHALLENGES);
        frame.render_interactive_widget(local_challenge_button, btm_split[1]);

        let network_challenge_button = Button::new(
            format!(
                "Auto-accept network challenges: {}",
                if own_team.autonomous_strategy.challenge_network {
                    "on"
                } else {
                    "off"
                }
            ),
            UiCallback::ToggleTeamAutonomousStrategyForNetworkChallenges,
        )
        .set_hover_text("Accept challenges from network teams automatically.".to_string())
        .set_hotkey(ui_key::team::TOGGLE_ACCEPT_NETWORK_CHALLENGES);
        frame.render_interactive_widget(network_challenge_button, btm_split[2]);

        match own_team.current_location {
            TeamLocation::OnPlanet { .. } => {
                if let Some(upgrade) = &own_team.spaceship.pending_upgrade {
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

        let split = Layout::horizontal([Constraint::Min(16), Constraint::Max(24)]).split(
            area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }),
        );

        let displayed_challenges = self.challenge_teams.len().min(area.height as usize / 3 - 1);
        let left_split = Layout::vertical([3].repeat(displayed_challenges)).split(split[0]);
        let right_split = Layout::vertical([3].repeat(displayed_challenges)).split(split[1]);

        for (idx, team_id) in self
            .challenge_teams
            .iter()
            .take(displayed_challenges)
            .enumerate()
        {
            let team = world.teams.get_or_err(team_id)?;
            frame.render_widget(
                Paragraph::new(format!(
                    "{:<MAX_NAME_LENGTH$} {}",
                    team.name,
                    world.team_rating(team_id).unwrap_or_default().stars()
                )),
                left_split[idx].inner(Margin::new(1, 1)),
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

        if self.past_game_ids.is_empty() {
            return Ok(());
        }

        let own_team = world.get_own_team()?;
        let split = Layout::horizontal([Constraint::Max(36), Constraint::Fill(1)]).split(area);
        let v_split = Layout::vertical([Constraint::Fill(1), Constraint::Length(3)])
            .split(split[0].inner(Margin::new(1, 1)));

        let mut options = vec![];
        if let Some(game_id) = own_team.current_game {
            if let Ok(game) = world.games.get_or_err(&game_id) {
                if let Some(action) = game.action_results.last() {
                    let text = format!(
                        " {:>12} {:>3}-{:<3} {:<}",
                        game.home_team_in_game.name,
                        action.home_score, // FIXME: this is not the correct score
                        action.away_score,
                        game.away_team_in_game.name,
                    );
                    let style = if action.home_score == action.away_score {
                        UiStyle::WARNING
                    } else if (action.home_score > action.away_score
                        && game.home_team_in_game.team_id == own_team.id)
                        || (action.home_score < action.away_score
                            && game.away_team_in_game.team_id == own_team.id)
                    {
                        UiStyle::OK
                    } else {
                        UiStyle::ERROR
                    };
                    options.push((text, style));
                }
            }
        }

        for game_id in self.past_game_ids.iter() {
            if let Some(game) = world.past_games.get(game_id) {
                let text = format!(
                    " {:>12} {:>3}-{:<3} {:<}",
                    game.home_team_name,
                    game.home_quarters_score.iter().sum::<u16>(),
                    game.away_quarters_score.iter().sum::<u16>(),
                    game.away_team_name,
                );

                let style = match game.winner {
                    Some(id) if id == own_team.id => UiStyle::OK,
                    Some(id) if id != own_team.id => UiStyle::ERROR,
                    None => UiStyle::WARNING,
                    _ => unreachable!(),
                };

                options.push((text, style));
            }
        }
        let list = selectable_list(options);

        frame.render_stateful_interactive_widget(
            list,
            v_split[0],
            &mut ClickableListState::default().with_selected(self.game_index),
        );

        let game_index = if let Some(index) = self.game_index {
            index % self.past_game_ids.len()
        } else {
            return Ok(());
        };

        let game_id = if let Some(&game_id) = self.past_game_ids.get(game_index) {
            game_id
        } else {
            return Ok(());
        };

        if world.games.contains_key(&game_id)
            || world.recently_finished_games.contains_key(&game_id)
        {
            let button = Button::new("Go to game", UiCallback::GoToGame { game_id })
                .set_hotkey(ui_key::GO_TO_GAME)
                .set_hover_text("Go to game");

            frame.render_interactive_widget(button, v_split[1]);
        } else if let Some(loaded_game) = self.loaded_games.get(&game_id) {
            let button = match loaded_game {
                Ok(game) => Button::new(
                    "Go to game",
                    UiCallback::GoToLoadedGame { game: game.clone() },
                )
                .set_hotkey(ui_key::GO_TO_GAME)
                .set_hover_text("Go to game"),

                Err(err) => Button::new("Go to game", UiCallback::None)
                    .set_hotkey(ui_key::GO_TO_GAME)
                    .set_hover_text("Go to game")
                    .disabled(Some(err.to_string())),
            };

            frame.render_interactive_widget(button, v_split[1]);
        }

        let summary = if let Ok(current_game) = world.games.get_or_err(&game_id) {
            let (home_quarters_score, away_quarters_score) = current_game.get_score_by_quarter();

            let lines = vec![
                Line::from(format!(
                    "Location {} - Attendance {}",
                    if let Ok(planet) = world.planets.get_or_err(&current_game.location) {
                        planet.name.as_str()
                    } else if current_game.planet_name != String::default() {
                        current_game.planet_name.as_str()
                    } else {
                        "Unknown"
                    },
                    current_game.attendance,
                )),
                Line::from(format!(
                    "Currently playing: {}",
                    current_game.timer.format(),
                )),
                Line::default(),
                Line::from(Span::styled(
                    format!(
                        "{:12} {} {} {} {} {}",
                        "Team", "Q1", "Q2", "Q3", "Q4", "Result"
                    ),
                    UiStyle::HEADER.bold(),
                )),
                Line::from(vec![
                    Span::styled(
                        format!("{:12} ", current_game.home_team_in_game.name),
                        if current_game.home_team_in_game.team_id == self.own_team_id {
                            UiStyle::OWN_TEAM
                        } else if current_game.is_network() {
                            UiStyle::NETWORK
                        } else {
                            UiStyle::DEFAULT
                        },
                    ),
                    Span::raw(format!(
                        "{:02} {} {} {} {:^6}",
                        home_quarters_score[0],
                        if current_game.timer.period() >= Period::Q2 {
                            format!("{:02}", home_quarters_score[1])
                        } else {
                            "--".to_string()
                        },
                        if current_game.timer.period() >= Period::Q3 {
                            format!("{:02}", home_quarters_score[2])
                        } else {
                            "--".to_string()
                        },
                        if current_game.timer.period() >= Period::Q4 {
                            format!("{:02}", home_quarters_score[3])
                        } else {
                            "--".to_string()
                        },
                        home_quarters_score.iter().sum::<u16>(),
                    )),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("{:12} ", current_game.away_team_in_game.name),
                        if current_game.away_team_in_game.team_id == self.own_team_id {
                            UiStyle::OWN_TEAM
                        } else if current_game.is_network() {
                            UiStyle::NETWORK
                        } else {
                            UiStyle::DEFAULT
                        },
                    ),
                    Span::raw(format!(
                        "{:02} {} {} {} {:^6}",
                        away_quarters_score[0],
                        if current_game.timer.period() >= Period::Q2 {
                            format!("{:02}", away_quarters_score[1])
                        } else {
                            "--".to_string()
                        },
                        if current_game.timer.period() >= Period::Q3 {
                            format!("{:02}", away_quarters_score[2])
                        } else {
                            "--".to_string()
                        },
                        if current_game.timer.period() >= Period::Q4 {
                            format!("{:02}", away_quarters_score[3])
                        } else {
                            "--".to_string()
                        },
                        away_quarters_score.iter().sum::<u16>(),
                    )),
                ]),
            ];

            Paragraph::new(lines)
        } else {
            let game_summary = world
                .past_games
                .get(&game_id)
                .ok_or(anyhow!("Unable to get past game."))?;

            let mut lines = vec![
                Line::from(format!(
                    "Location {} - Attendance {}",
                    if let Ok(planet) = world.planets.get_or_err(&game_summary.location) {
                        planet.name.as_str()
                    } else if game_summary.planet_name != String::default() {
                        game_summary.planet_name.as_str()
                    } else {
                        "Unknown"
                    },
                    game_summary.attendance
                )),
                Line::from(format!(
                    "Ended on {}",
                    game_summary
                        .ended_at
                        .expect("Past games should have ended")
                        .formatted_as_date()
                )),
                Line::default(),
                Line::from(Span::styled(
                    format!(
                        "{:12} {} {} {} {} {}",
                        "Team", "Q1", "Q2", "Q3", "Q4", "Result"
                    ),
                    UiStyle::HEADER.bold(),
                )),
                Line::from(vec![
                    Span::styled(
                        format!("{:12} ", game_summary.home_team_name),
                        if game_summary.home_team_id == self.own_team_id {
                            UiStyle::OWN_TEAM
                        } else if game_summary.is_network {
                            UiStyle::NETWORK
                        } else {
                            UiStyle::DEFAULT
                        },
                    ),
                    Span::raw(format!(
                        "{:02} {:02} {:02} {:02} {:^6} {}",
                        game_summary.home_quarters_score[0],
                        game_summary.home_quarters_score[1],
                        game_summary.home_quarters_score[2],
                        game_summary.home_quarters_score[3],
                        game_summary.home_quarters_score.iter().sum::<u16>(),
                        if game_summary.home_team_knocked_out {
                            "wasted"
                        } else {
                            ""
                        }
                    )),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("{:12} ", game_summary.away_team_name),
                        if game_summary.away_team_id == self.own_team_id {
                            UiStyle::OWN_TEAM
                        } else if game_summary.is_network {
                            UiStyle::NETWORK
                        } else {
                            UiStyle::DEFAULT
                        },
                    ),
                    Span::raw(format!(
                        "{:02} {:02} {:02} {:02} {:^6} {}",
                        game_summary.away_quarters_score[0],
                        game_summary.away_quarters_score[1],
                        game_summary.away_quarters_score[2],
                        game_summary.away_quarters_score[3],
                        game_summary.away_quarters_score.iter().sum::<u16>(),
                        if game_summary.away_team_knocked_out {
                            "wasted"
                        } else {
                            ""
                        }
                    )),
                ]),
            ];

            lines.append(&mut self.get_loaded_game_description(game_id, world));

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

    fn get_loaded_game_description<'a>(
        &'a mut self,
        game_id: GameId,
        world: &'a World,
    ) -> Vec<Line<'a>> {
        let game = if let Some(game) = world.recently_finished_games.get(&game_id) {
            game
        } else {
            let entry = self
                .loaded_games
                .entry(game_id)
                .or_insert_with(|| load_game(&game_id));

            match entry {
                Ok(game) => game,
                Err(_) => return vec![],
            }
        };

        let mut lines = vec![];

        let home_mvps = game
            .home_team_mvps
            .as_ref()
            .expect("Loaded game should have set mvps.");

        let mut extra_lines = home_mvps
            .iter()
            .map(|mvp| {
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    mvp.name,
                    format!("{:>2} {}", mvp.best_stats[0].1, mvp.best_stats[0].0),
                    format!("{:>2} {}", mvp.best_stats[1].1, mvp.best_stats[1].0),
                    format!("{:>2} {}", mvp.best_stats[2].1, mvp.best_stats[2].0)
                ))
            })
            .collect_vec();

        lines.append(&mut vec![
            Line::from(String::new()),
            Line::from(Span::styled(
                game.home_team_in_game.name.as_str(),
                UiStyle::HEADER.bold(),
            )),
        ]);
        lines.append(&mut extra_lines);

        let away_mvps = game
            .away_team_mvps
            .as_ref()
            .expect("Loaded game should have set mvps.");

        let mut extra_lines = away_mvps
            .iter()
            .map(|mvp| {
                Line::from(format!(
                    "{:<18}{:<8}{:<8}{:<8}",
                    mvp.name,
                    format!("{:>2} {}", mvp.best_stats[0].1, mvp.best_stats[0].0),
                    format!("{:>2} {}", mvp.best_stats[1].1, mvp.best_stats[1].0),
                    format!("{:>2} {}", mvp.best_stats[2].1, mvp.best_stats[2].0)
                ))
            })
            .collect_vec();
        lines.append(&mut vec![
            Line::from(String::new()),
            Line::from(Span::styled(
                game.away_team_in_game.name.as_str(),
                UiStyle::HEADER.bold(),
            )),
        ]);
        lines.append(&mut extra_lines);

        lines
    }

    fn render_shipyard(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_shipyard_upgrades_list(frame, world, split[0])?;

        let own_team = world.get_own_team()?;
        match own_team.current_location {
            TeamLocation::OnPlanet { .. } => {
                if let Some(upgrade) = &own_team.spaceship.pending_upgrade {
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

    fn render_shipyard_upgrades_list(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        frame.render_widget(default_block().title("Upgrades "), area);

        // |------|---------|
        // |      |         |
        // | list | upgrade |
        // |      | descrip |
        // |      |         |
        // |------|---------|
        // |  build button  |
        // |----------------|

        let v_split = Layout::vertical([
            Constraint::Length(SpaceshipUpgradeTarget::iter().count() as u16 + 2),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .split(area.inner(Margin::new(1, 1)));

        let h_split = Layout::horizontal([
            Constraint::Length(MAX_NAME_LENGTH as u16 + 2),
            Constraint::Fill(1),
        ])
        .split(v_split[0]);

        let own_team = world.get_own_team()?;

        let options = SpaceshipUpgradeTarget::iter()
            .map(|upgrade_target| {
                (
                    upgrade_target.to_string(),
                    if own_team.spaceship.can_be_upgraded(upgrade_target) {
                        UiStyle::DEFAULT
                    } else {
                        UiStyle::UNSELECTABLE
                    },
                )
            })
            .collect_vec();

        let list = selectable_list(options);

        frame.render_stateful_interactive_widget(
            list,
            h_split[0].inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(Some(self.spaceship_upgrade_index)),
        );

        let available = available_upgrade_targets(&own_team.spaceship);
        let possible_upgrade_target = available[self.spaceship_upgrade_index % available.len()];
        let bonus = TeamBonus::Upgrades.current_team_bonus(world, &own_team.id)?;
        let possible_upgrade = possible_upgrade_target.map(|target| Upgrade::new(target, bonus));

        let lines = if let Some(target) = possible_upgrade_target {
            let header = match target {
                SpaceshipUpgradeTarget::Repairs { .. } => "Repair spaceship ".to_string(),
                target => format!("Upgrade {target} "),
            };

            let subheader = match target {
                SpaceshipUpgradeTarget::Hull { component } => {
                    format!(
                        "{} --> {}",
                        component
                            .previous()
                            .expect("there should be a previous component"),
                        component
                    )
                }
                SpaceshipUpgradeTarget::ChargeUnit { component } => {
                    format!(
                        "{} --> {}",
                        component
                            .previous()
                            .expect("there should be a previous component"),
                        component
                    )
                }
                SpaceshipUpgradeTarget::Engine { component } => {
                    format!(
                        "{} --> {}",
                        component
                            .previous()
                            .expect("there should be a previous component"),
                        component
                    )
                }
                SpaceshipUpgradeTarget::Shooter { component } => {
                    format!(
                        "{} --> {}",
                        component
                            .previous()
                            .expect("there should be a previous component"),
                        component
                    )
                }
                SpaceshipUpgradeTarget::Storage { component } => {
                    format!(
                        "{} --> {}",
                        component
                            .previous()
                            .expect("there should be a previous component"),
                        component
                    )
                }
                SpaceshipUpgradeTarget::Shield { component } => {
                    format!(
                        "{} --> {}",
                        component
                            .previous()
                            .expect("there should be a previous component"),
                        component
                    )
                }
                SpaceshipUpgradeTarget::Repairs { .. } => format!(
                    "{} --> {}",
                    own_team.spaceship.current_durability(),
                    own_team.spaceship.max_durability()
                ),
            };

            let mut lines = vec![
                Line::from(Span::styled(header, UiStyle::HEADER.bold())).centered(),
                Line::from(subheader).centered(),
            ];

            lines.append(&mut spaceship_upgrade_target_description_lines(target));

            lines
        } else if self.spaceship_upgrade_index == SpaceshipUpgradeTarget::iter().count() - 1 {
            vec![
                Line::default(),
                Line::default(),
                Line::from("Fully repaired").centered(),
            ]
        } else {
            vec![
                Line::default(),
                Line::default(),
                Line::from("No more upgrades").centered(),
                Line::from("available").centered(),
            ]
        };

        frame.render_widget(Paragraph::new(lines), h_split[1].inner(Margin::new(3, 1)));

        Self::render_available_spaceship_upgrades(
            possible_upgrade,
            world,
            own_team,
            frame,
            v_split[1],
        )?;
        self.render_upgrade_spaceship_button(possible_upgrade, own_team, frame, v_split[2])?;

        Ok(())
    }

    fn render_available_spaceship_upgrades(
        possible_upgrade: Option<Upgrade<SpaceshipUpgradeTarget>>,
        world: &World,
        own_team: &Team,
        frame: &mut UiFrame,
        area: Rect,
    ) -> AppResult<()> {
        let spaceship = &own_team.spaceship;
        if let Some(pending_upgrade) = spaceship.pending_upgrade {
            let header = match pending_upgrade.target {
                SpaceshipUpgradeTarget::Repairs { .. } => "Repairing spaceship".to_string(),
                target => format!("Upgrading {target}"),
            };

            let countdown = (pending_upgrade.started + pending_upgrade.duration)
                .saturating_sub(world.last_tick_short_interval)
                .formatted();

            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![Span::styled(header, UiStyle::HEADER.bold())]),
                    Line::from(countdown),
                ])
                .centered(),
                area,
            )
        } else if let Some(upgrade) = possible_upgrade {
            {
                let mut lines =
                    vec![
                        Line::from(Span::styled("Upgrade cost", UiStyle::HEADER.bold())).centered(),
                    ];
                lines.append(&mut upgrade_resources_lines(upgrade.target, own_team));
                frame.render_widget(Paragraph::new(lines).centered(), area);
            }
        }

        Ok(())
    }

    fn render_upgrade_spaceship_button(
        &self,
        possible_upgrade: Option<Upgrade<SpaceshipUpgradeTarget>>,
        own_team: &Team,
        frame: &mut UiFrame,
        area: Rect,
    ) -> AppResult<()> {
        let spaceship = &own_team.spaceship;
        if let Some(pending_upgrade) = spaceship.pending_upgrade {
            let text = if matches!(
                pending_upgrade.target,
                SpaceshipUpgradeTarget::Repairs { .. }
            ) {
                "Repairing spaceship".to_string()
            } else {
                format!("Upgrading {}", pending_upgrade.target)
            };
            let build_button = Button::new(text.clone(), UiCallback::None)
                .disabled(Some(format!("Already {}", text.to_lowercase())));

            frame.render_interactive_widget(build_button, area);
        } else if let Some(upgrade) = possible_upgrade {
            let text = if matches!(upgrade.target, SpaceshipUpgradeTarget::Repairs { .. }) {
                format!("Repair spaceship ({})", upgrade.duration.formatted())
            } else {
                format!(
                    "Upgrade {} ({})",
                    upgrade.target,
                    upgrade.duration.formatted()
                )
            };

            let hotkey = if matches!(upgrade.target, SpaceshipUpgradeTarget::Repairs { .. }) {
                ui_key::REPAIR_SPACESHIP
            } else {
                ui_key::UPGRADE_SPACESHIP
            };

            let mut upgrade_button = Button::new(text, UiCallback::SetSpaceshipUpgrade { upgrade })
                .set_hotkey(hotkey)
                .set_hover_text(upgrade.target.description());

            let can_upgrade_spaceship = own_team.can_upgrade_spaceship(&upgrade);
            if let Err(e) = can_upgrade_spaceship.as_ref() {
                upgrade_button.disable(Some(e.to_string()));
            }

            frame.render_interactive_widget(upgrade_button, area);
        } else {
            let text = {
                let target =
                    SpaceshipUpgradeTarget::iter().collect_vec()[self.spaceship_upgrade_index];
                if matches!(target, SpaceshipUpgradeTarget::Repairs { .. }) {
                    "Spaceship fully repaired".to_string()
                } else {
                    format!("{target} fully upgraded")
                }
            };
            let build_button = Button::new(text, UiCallback::None).disabled(None::<String>);
            frame.render_interactive_widget(build_button, area);
        }

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

        if self.asteroid_ids.is_empty() {
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

        // |------|---------|
        // |      |         |
        // | list | upgrade |
        // |      | descrip |
        // |      |         |
        // |------|---------|
        // |  build button  |
        // |----------------|

        let v_split = Layout::vertical([
            Constraint::Length(MAX_NUM_ASTEROID_PER_TEAM as u16 + 2),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .split(area.inner(Margin::new(1, 1)));

        let h_split = Layout::horizontal([
            Constraint::Length(MAX_NAME_LENGTH as u16 + 2),
            Constraint::Fill(1),
        ])
        .split(v_split[0]);

        let own_team = world.get_own_team()?;

        let options = self
            .asteroid_ids
            .iter()
            .filter(|&asteroid_id| world.planets.get_or_err(asteroid_id).is_ok())
            .map(|&asteroid_id| {
                let asteroid = world.planets.get_or_err(&asteroid_id).unwrap();
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

        frame.render_stateful_interactive_widget(
            list,
            h_split[0].inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut ClickableListState::default().with_selected(self.asteroid_index),
        );

        if let Some(index) = self.asteroid_index {
            let asteroid_id = own_team.asteroid_ids[index % own_team.asteroid_ids.len()];
            let asteroid = world.planets.get_or_err(&asteroid_id)?;

            let mut lines = vec![Line::from(Span::styled(
                "Resources",
                UiStyle::HEADER.bold(),
            ))];
            for resource in Resource::iter() {
                if resource == Resource::SATOSHI {
                    continue;
                }
                let amount = asteroid
                    .resources
                    .get(&resource)
                    .copied()
                    .unwrap_or_default();

                lines.push(Line::from(Span::styled(
                    format!("{:<7} {}", resource.to_string(), (amount as f32).stars(),),
                    resource.style(),
                )));
            }

            frame.render_widget(
                Paragraph::new(lines).centered(),
                h_split[1].inner(Margin::new(1, 1)),
            );

            let possible_upgrade = if !asteroid
                .upgrades
                .contains(&AsteroidUpgradeTarget::TeleportationPad)
            {
                let bonus = TeamBonus::Upgrades.current_team_bonus(world, &own_team.id)?;
                Some(Upgrade::new(AsteroidUpgradeTarget::TeleportationPad, bonus))
            } else if own_team.has_space_cove_on().is_none() {
                // Build space cove button
                let bonus = TeamBonus::Upgrades.current_team_bonus(world, &own_team.id)?;
                Some(Upgrade::new(AsteroidUpgradeTarget::SpaceCove, bonus))
            } else {
                None
            };

            Self::render_available_asteroid_upgrades(
                asteroid,
                world,
                possible_upgrade,
                own_team,
                frame,
                v_split[1],
            )?;
            Self::render_build_asteroid_upgrade_button(
                asteroid,
                possible_upgrade,
                own_team,
                frame,
                v_split[2],
            )?;
        }

        Ok(())
    }

    fn render_available_asteroid_upgrades(
        asteroid: &Planet,
        world: &World,
        possible_upgrade: Option<Upgrade<AsteroidUpgradeTarget>>,
        own_team: &Team,
        frame: &mut UiFrame,
        area: Rect,
    ) -> AppResult<()> {
        if let Some(pending_upgrade) = asteroid.pending_upgrade {
            let countdown = (pending_upgrade.started + pending_upgrade.duration)
                .saturating_sub(world.last_tick_short_interval)
                .formatted();

            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        format!("Building {}", pending_upgrade.target),
                        UiStyle::HEADER.bold(),
                    )),
                    Line::from(countdown),
                ])
                .centered(),
                area,
            );
        } else if let Some(upgrade) = possible_upgrade {
            {
                let mut lines = vec![Line::from(Span::styled(
                    format!("{} upgrade cost", upgrade.target),
                    UiStyle::HEADER.bold(),
                ))];
                lines.append(&mut upgrade_resources_lines(upgrade.target, own_team));
                frame.render_widget(Paragraph::new(lines).centered(), area);
            }
        } else {
            let lines = vec![
                Line::from("Nothing left"),
                Line::from(format!("to build on {}", asteroid.name)),
            ];
            frame.render_widget(Paragraph::new(lines).centered(), area);
        }

        Ok(())
    }

    fn render_build_asteroid_upgrade_button(
        asteroid: &Planet,
        possible_upgrade: Option<Upgrade<AsteroidUpgradeTarget>>,
        own_team: &Team,
        frame: &mut UiFrame,
        area: Rect,
    ) -> AppResult<()> {
        if let Some(pending_upgrade) = asteroid.pending_upgrade {
            let build_button = Button::new(
                format!("Building {}", pending_upgrade.target),
                UiCallback::None,
            )
            .set_hover_text(format!(
                "Building {} on {}",
                pending_upgrade.target, asteroid.name
            ))
            .disabled(Some(format!("Already building {}", pending_upgrade.target)));

            frame.render_interactive_widget(build_button, area);
        } else if let Some(upgrade) = possible_upgrade {
            let on_click = if upgrade.target == AsteroidUpgradeTarget::SpaceCove {
                UiCallback::PushUiPopup {
                    popup_message: PopupMessage::BuildSpaceCove {
                        asteroid_name: asteroid.name.clone(),
                        asteroid_id: asteroid.id,
                        tick: Tick::now(),
                    },
                }
            } else {
                UiCallback::SetAsteroidPendingUpgrade {
                    asteroid_id: asteroid.id,
                    upgrade,
                }
            };

            let mut build_button = Button::new(
                format!(
                    "Build {} ({})",
                    upgrade.target,
                    upgrade.duration.formatted()
                ),
                on_click,
            )
            .set_hotkey(ui_key::BUILD_ASTEROID_UPGRADE)
            .set_hover_text(upgrade.target.description());

            if upgrade.target == AsteroidUpgradeTarget::SpaceCove {
                build_button = build_button.block(default_block().border_style(UiStyle::WARNING));
            }

            let can_upgrade_asteroid = own_team.can_upgrade_asteroid(asteroid, &upgrade);
            if let Err(e) = can_upgrade_asteroid.as_ref() {
                build_button.disable(Some(e.to_string()));
            }

            frame.render_interactive_widget(build_button, area);
        } else {
            let build_button = Button::new(
                format!("Nothing left to build on {}", asteroid.name),
                UiCallback::None,
            )
            .disabled(None::<String>);
            frame.render_interactive_widget(build_button, area);
        }

        Ok(())
    }

    fn render_selected_asteroid(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if self.asteroid_ids.is_empty() {
            frame.render_widget(default_block(), area);
            return Ok(());
        }

        let asteroid_id =
            self.asteroid_ids[self.asteroid_index.unwrap_or_default() % self.asteroid_ids.len()];
        let asteroid = world.planets.get_or_err(&asteroid_id)?;

        let mut parents = vec![asteroid];
        let mut current = asteroid;
        while let Some(parent_id) = current.satellite_of {
            let parent = world.planets.get_or_err(&parent_id)?;
            parents.push(parent);
            current = parent;
        }

        let mut parent_buttons = Vec::new();
        for parent in parents.iter().rev() {
            if !parent_buttons.is_empty() {
                parent_buttons
                    .push(Button::new(" --> ", UiCallback::None).set_hover_style(UiStyle::DEFAULT));
            }

            parent_buttons.push(
                Button::new(
                    parent.name.as_str(),
                    UiCallback::GoToPlanetZoomIn {
                        planet_id: parent.id,
                    },
                )
                .set_hover_text(format!("Go to {}", parent.name)),
            );
        }
        let constraints = parent_buttons
            .iter()
            .map(|b| b.text_width() as u16)
            .collect_vec();
        let area_top =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(area)[0];
        let buttons_split = Layout::horizontal(constraints)
            .horizontal_margin(5)
            .split(area_top);

        frame.render_widget(default_block(), area);
        for (idx, button) in parent_buttons.into_iter().enumerate() {
            frame.render_interactive_widget(button, buttons_split[idx]);
        }

        let split = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        let img_lines = self
            .gif_map
            .planet_zoom_out_frame_lines(asteroid, 0, world)?;
        frame.render_widget(Paragraph::new(img_lines).centered(), split[0]);

        if asteroid
            .upgrades
            .contains(&AsteroidUpgradeTarget::SpaceCove)
        {
            let b_split = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(split[1]);
            frame.render_interactive_widget(teleport_button(world, asteroid_id)?, b_split[0]);
            frame.render_interactive_widget(go_to_space_cove_button()?, b_split[1]);
        } else {
            frame.render_interactive_widget(teleport_button(world, asteroid_id)?, split[1]);
        }

        let b_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(split[2]);
        frame.render_interactive_widget(go_to_planet_button(world, asteroid_id)?, b_split[0]);

        let popup_message = PopupMessage::AbandonAsteroid {
            asteroid_name: asteroid.name.clone(),
            asteroid_id,
            tick: Tick::now(),
        };

        let abandon_asteroid_button =
            Button::new("Abandon", UiCallback::PushUiPopup { popup_message })
                .set_hotkey(ui_key::ABANDON_ASTEROID)
                .set_hover_text("Abandon this asteroid (there's no way back!)")
                .block(default_block().border_style(UiStyle::WARNING));

        frame.render_interactive_widget(abandon_asteroid_button, b_split[1]);

        Ok(())
    }

    fn render_player_buttons(
        &self,
        players: &[&Player],
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let player_index = if let Some(index) = self.player_index {
            index.min(players.len() - 1)
        } else {
            return Ok(());
        };

        let player = players[player_index % players.len()];
        let player_id = player.id;
        let button_splits = Layout::horizontal([
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(24),
            Constraint::Length(24),
            Constraint::Min(0),
        ])
        .split(area.inner(Margin {
            vertical: 0,
            horizontal: 1,
        }));

        let can_set_crew_role = own_team.can_set_crew_role(player);

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
            TeamBonus::Reputation.as_skill(player).percentage(),
            TeamBonus::TradePrice,
            TeamBonus::TradePrice.as_skill(player).percentage()
        ))
        .set_hotkey(ui_key::team::SET_CAPTAIN);
        if own_team.crew_roles.captain == Some(player.id) {
            captain_button = captain_button
                .set_hover_text("Remove player from captain role".to_string())
                .selected();
        } else if let Err(e) = can_set_crew_role.as_ref() {
            captain_button.disable(Some(e.to_string()));
        }
        frame.render_interactive_widget(captain_button, button_splits[0]);

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
            TeamBonus::SpaceshipSpeed.as_skill(player).percentage(),
            TeamBonus::Exploration,
            TeamBonus::Exploration.as_skill(player).percentage()
        ))
        .set_hotkey(ui_key::team::SET_PILOT);
        if own_team.crew_roles.pilot == Some(player.id) {
            pilot_button = pilot_button
                .set_hover_text("Remove player from pilot role".to_string())
                .selected();
        } else if let Err(e) = can_set_crew_role.as_ref() {
            pilot_button.disable(Some(e.to_string()));
        }
        frame.render_interactive_widget(pilot_button, button_splits[1]);

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
            TeamBonus::TirednessRecovery.as_skill(player).percentage(),
            TeamBonus::Training,
            TeamBonus::Training.as_skill(player).percentage()
        ))
        .set_hotkey(ui_key::team::SET_DOCTOR);
        if own_team.crew_roles.doctor == Some(player.id) {
            doctor_button = doctor_button
                .set_hover_text("Remove player from doctor role".to_string())
                .selected();
        } else if let Err(e) = can_set_crew_role.as_ref() {
            doctor_button.disable(Some(e.to_string()));
        }
        frame.render_interactive_widget(doctor_button, button_splits[2]);

        let mut engineer_button = Button::new(
            "engineer",
            UiCallback::SetCrewRole {
                player_id,
                role: CrewRole::Engineer,
            },
        )
        .set_hover_text(format!(
            "Set player to engineer role: {} +{}%, {} {}%",
            TeamBonus::Weapons,
            TeamBonus::Weapons.as_skill(player).percentage(),
            TeamBonus::Upgrades,
            TeamBonus::Upgrades.as_skill(player).percentage()
        ))
        .set_hotkey(ui_key::team::SET_ENGINEER);
        if own_team.crew_roles.engineer == Some(player.id) {
            engineer_button = engineer_button
                .set_hover_text("Remove player from engineer role".to_string())
                .selected();
        } else if let Err(e) = can_set_crew_role.as_ref() {
            engineer_button.disable(Some(e.to_string()));
        }
        frame.render_interactive_widget(engineer_button, button_splits[3]);

        let can_release = own_team.can_release_player(player);
        let popup_message = PopupMessage::ReleasePlayer {
            player_name: player.info.full_name(),
            player_id,
            not_enough_players_for_game: own_team.player_ids.len() - 1 < MIN_PLAYERS_PER_GAME,
            tick: Tick::now(),
        };
        let mut release_button = Button::new(
            format!("Fire {}", player.info.short_name()),
            UiCallback::PushUiPopup { popup_message },
        )
        .set_hover_text("Fire pirate from the crew!")
        .set_hotkey(ui_key::player::FIRE);
        if let Err(err) = can_release {
            release_button.disable(Some(err.to_string()));
        } else {
            release_button = release_button.block(default_block().border_style(UiStyle::WARNING));
        }

        frame.render_interactive_widget(release_button, button_splits[4]);

        if let Ok(drink_button) = drink_button(world, &player_id) {
            frame.render_interactive_widget(drink_button, button_splits[5]);
        }

        Ok(())
    }

    fn build_players_table<'a>(
        players: &'a Vec<&Player>,
        world: &'a World,
        table_width: u16,
    ) -> AppResult<ClickableTable<'a>> {
        let own_team = world.get_own_team()?;
        let header_cells = [
            "Name",
            "Overall",
            "Potential",
            "Current",
            "Best",
            "Role",
            "Crew bonus",
        ]
        .iter()
        .map(|h| ClickableCell::from(*h).style(UiStyle::HEADER.bold()));
        let header = ClickableRow::new(header_cells);

        // Calculate the available space for the players name in order to display the
        // full or shortened version.
        let name_header_width = table_width
            .saturating_sub(9 + 10 + 10 + 10 + 9 + 15 + 20)
            .max(1);

        let rows = players
            .iter()
            .map(|player| {
                let skills = player.current_skill_array();

                let current_role = match own_team.player_ids.iter().position(|id| *id == player.id)
                {
                    Some(idx) => format!(
                        "{:<2} {:<5}",
                        (idx as GamePosition).as_str(),
                        if (idx as GamePosition) < MAX_GAME_POSITION {
                            (idx as GamePosition).player_rating(skills).stars()
                        } else {
                            "".to_string()
                        }
                    ),
                    None => unreachable!("Player in MyTeam should have a position."),
                };
                let best_role = GamePosition::best(skills);
                let overall = player.average_skill().stars();
                let potential = player.potential.stars();

                let bonus_string_1 = match player.info.crew_role {
                    CrewRole::Pilot => {
                        let skill = TeamBonus::SpaceshipSpeed.as_skill(player);
                        Span::styled(
                            format!("{} +{}%", TeamBonus::SpaceshipSpeed, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Captain => {
                        let skill = TeamBonus::Reputation.as_skill(player);
                        Span::styled(
                            format!("{} +{}%", TeamBonus::Reputation, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Doctor => {
                        let skill = TeamBonus::TirednessRecovery.as_skill(player);
                        Span::styled(
                            format!("{} +{}%", TeamBonus::TirednessRecovery, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Engineer => {
                        let skill = TeamBonus::Weapons.as_skill(player);
                        Span::styled(
                            format!("{} +{}%", TeamBonus::Weapons, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Mozzo => Span::raw(""),
                };

                let bonus_string_2 = match player.info.crew_role {
                    CrewRole::Pilot => {
                        let skill = TeamBonus::Exploration.as_skill(player);
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::Exploration, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Captain => {
                        let skill = TeamBonus::TradePrice.as_skill(player);
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::TradePrice, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Doctor => {
                        let skill = TeamBonus::Training.as_skill(player);
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::Training, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Engineer => {
                        let skill = TeamBonus::Upgrades.as_skill(player);
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::Upgrades, skill.percentage()),
                            skill.style(),
                        )
                    }
                    CrewRole::Mozzo => Span::raw(""),
                };

                let name = if name_header_width >= 2 * MAX_NAME_LENGTH as u16 + 2 {
                    player.info.full_name()
                } else {
                    player.info.short_name()
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
                Constraint::Min(MAX_NAME_LENGTH as u16 + 2),
                Constraint::Length(9),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(9),
                Constraint::Length(15),
                Constraint::Length(20),
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
        let sorted_players = own_team
            .player_ids
            .iter()
            .map(|id| world.players.get(id).unwrap())
            .collect_vec()
            .sort_by_rating();

        let player_index = if let Some(index) = self.player_index {
            index.min(sorted_players.len() - 1)
        } else {
            return Ok(());
        };
        let player = sorted_players[player_index];

        let top_split =
            Layout::horizontal([Constraint::Min(10), Constraint::Length(60)]).split(area);

        let table = Self::build_players_table(&sorted_players, world, top_split[0].width)?;
        frame.render_stateful_interactive_widget(
            table.block(default_block().title(format!(
                "{} {} ↓/↑",
                own_team.name.clone(),
                world.team_rating(&own_team.id).unwrap_or_default().stars()
            ))),
            top_split[0],
            &mut ClickableTableState::default().with_selected(self.player_index),
        );

        render_player_description(
            player,
            self.player_widget_view,
            &mut self.gif_map,
            self.tick,
            world,
            frame,
            top_split[1],
        );

        if let Some(game_id) = own_team.current_game {
            let game = world.games.get_or_err(&game_id)?;
            let game_text = format!(
                "{:>} {:>3}-{:<3} {:<}",
                game.home_team_in_game.name,
                if let Some(action) = game.action_results.last() {
                    action.home_score
                } else {
                    0
                },
                if let Some(action) = game.action_results.last() {
                    action.away_score
                } else {
                    0
                },
                game.away_team_in_game.name,
            );
            let border_style = if game.is_network() {
                UiStyle::NETWORK
            } else {
                UiStyle::OWN_TEAM
            };

            let table_bottom = Layout::vertical([Constraint::Fill(1), Constraint::Length(6)])
                .split(top_split[0].inner(Margin::new(1, 1)));

            frame.render_interactive_widget(
                Button::new(
                    vec![
                        Line::from("Currently playing".to_string()).centered(),
                        Line::default(),
                        Line::from(game_text.to_string()).centered(),
                        Line::from(game.timer.format().to_string()).centered(),
                    ],
                    UiCallback::GoToGame { game_id },
                )
                .set_hover_text("Go to current game")
                .set_hotkey(ui_key::GO_TO_CURRENT_GAME)
                .block(default_block().border_style(border_style)),
                table_bottom[1],
            );
            return Ok(());
        }

        let table_bottom = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(3), //position buttons
            Constraint::Length(3), // role buttons
        ])
        .split(top_split[0].inner(Margin::new(1, 1)));
        let position_button_splits = Layout::horizontal([
            Constraint::Length(6),  //pg
            Constraint::Length(6),  //sg
            Constraint::Length(6),  //sf
            Constraint::Length(6),  //pf
            Constraint::Length(6),  //c
            Constraint::Length(6),  //bench
            Constraint::Length(6),  //bench
            Constraint::Length(30), //auto-assign
            Constraint::Min(0),
        ])
        .split(table_bottom[1].inner(Margin {
            vertical: 0,
            horizontal: 1,
        }));

        let player_id = player.id;
        for idx in 0..MAX_PLAYERS_PER_GAME {
            let position = idx as GamePosition;
            let rect = position_button_splits[idx];
            let mut button = Button::new(
                format!(
                    "{}:{:<2}",
                    (idx + 1),
                    if position == 5 {
                        "B1"
                    } else if position == 6 {
                        "B2"
                    } else {
                        position.as_str()
                    }
                ),
                UiCallback::SwapPlayerPositions {
                    player_id,
                    position: idx,
                },
            )
            .set_hover_text(format!(
                "Set player initial position to {}.",
                position.as_str()
            ))
            .set_hotkey(ui_key::team::set_player_position(position));

            let position = own_team.player_ids.iter().position(|id| *id == player.id);
            if position.is_some() && position.unwrap() == idx {
                button.select();
            }
            frame.render_interactive_widget(button, rect);
        }

        let auto_assign_button =
            Button::new("Auto-assign positions", UiCallback::AssignBestTeamPositions)
                .set_hover_text("Auto-assign players' initial position.")
                .set_hotkey(ui_key::team::AUTO_ASSIGN);
        frame.render_interactive_widget(auto_assign_button, position_button_splits[7]);
        self.render_player_buttons(&sorted_players, frame, world, table_bottom[2])?;

        Ok(())
    }

    fn render_on_planet_spaceship(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;

        let split = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );

        render_spaceship_description(
            own_team,
            world,
            world.team_rating(&own_team.id).unwrap_or_default(),
            true,
            true,
            &mut self.gif_map,
            self.tick,
            frame,
            area,
        );

        let explore_split =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(split[1]);
        if let Ok(explore_button) = space_adventure_button(world, own_team) {
            frame.render_interactive_widget(explore_button, explore_split[0]);
        }
        if let Ok(explore_button) = explore_button(world, own_team) {
            frame.render_interactive_widget(explore_button, explore_split[1]);
        }
        Ok(())
    }

    fn render_upgrading_spaceship(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        upgrade: &Upgrade<SpaceshipUpgradeTarget>,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let countdown = (upgrade.started + upgrade.duration)
            .saturating_sub(world.last_tick_short_interval)
            .formatted();
        render_spaceship_upgrade(
            own_team,
            upgrade.target,
            true,
            &mut self.gif_map,
            self.tick,
            frame,
            area,
        );

        let title = match upgrade.target {
            SpaceshipUpgradeTarget::Repairs { .. } => "Repairing spaceship".to_string(),
            _ => format!("Upgrading {}", upgrade.target),
        };

        frame.render_widget(
            default_block().title(format!("{title} - {countdown}")),
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
        let own_team = world.get_own_team()?;
        let spaceship = &own_team.spaceship;

        let available = available_upgrade_targets(spaceship);
        let possible_upgrade_target = available[self.spaceship_upgrade_index % available.len()];
        if let Some(target) = possible_upgrade_target {
            render_spaceship_upgrade(
                own_team,
                target,
                false,
                &mut self.gif_map,
                self.tick,
                frame,
                area,
            );

            let title = match target {
                SpaceshipUpgradeTarget::Repairs { .. } => "Repair spaceship".to_string(),
                _ => format!("Upgrade {target}"),
            };

            frame.render_widget(default_block().title(title), area);
        } else {
            render_spaceship_description(
                own_team,
                world,
                world.team_rating(&own_team.id).unwrap_or_default(),
                true,
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
        let own_team = world.get_own_team()?;
        if let Ok(mut lines) = self
            .gif_map
            .travelling_spaceship_lines(&own_team.spaceship, self.tick)
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
        let planet = world.planets.get_or_err(planet_id)?;
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
        let own_team = world.get_own_team()?;
        if let Ok(mut lines) = self
            .gif_map
            .exploring_spaceship_lines(&own_team.spaceship, self.tick)
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
        let planet = world.planets.get_or_err(planet_id)?;
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

        if self.planet_markets.is_empty() || world.dirty_ui {
            self.planet_markets = world
                .planets
                .iter()
                .filter(|(_, planet)| planet.total_population() > 0)
                .sorted_by(|(_, a), (_, b)| a.name.cmp(&b.name))
                .map(|(id, _)| *id)
                .collect::<Vec<PlanetId>>();
            if self.planet_index.is_none() && !self.planet_markets.is_empty() {
                self.planet_index = Some(0);
            }
        }

        if self.asteroid_ids.len() != own_team.asteroid_ids.len() || world.dirty_ui {
            self.asteroid_ids = own_team.asteroid_ids.clone();
        }

        self.asteroid_index = if !self.asteroid_ids.is_empty() {
            if let Some(index) = self.asteroid_index {
                Some(index % self.asteroid_ids.len())
            } else {
                Some(0)
            }
        } else {
            None
        };

        self.player_index = if !own_team.player_ids.is_empty() {
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
            self.past_game_ids = games;

            self.challenge_teams = world
                .teams
                .keys()
                .filter(|&id| {
                    let team = if let Ok(team) = world.teams.get_or_err(id) {
                        team
                    } else {
                        return false;
                    };
                    own_team.can_challenge_local_team(team).is_ok()
                        || own_team.can_challenge_network_team(team).is_ok()
                })
                .cloned()
                .collect();
            self.challenge_teams.sort_by(|a, b| {
                let a = world.teams.get_or_err(a).unwrap();
                let b = world.teams.get_or_err(b).unwrap();
                world
                    .team_rating(&b.id)
                    .unwrap_or_default()
                    .partial_cmp(&world.team_rating(&a.id).unwrap_or_default())
                    .unwrap()
            });

            // self.players_table = Self::build_players_table(players, world, table_width)
        }

        self.game_index = if !self.past_game_ids.is_empty() {
            if let Some(index) = self.game_index {
                Some(index % self.past_game_ids.len())
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
            MyTeamView::Team => self.render_team(frame, world, bottom_split[1])?,
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
        self.planet_index?;

        match key_event.code {
            KeyCode::Up => {
                self.next_index();
            }
            KeyCode::Down => {
                self.previous_index();
            }
            ui_key::CYCLE_VIEW => {
                return Some(UiCallback::SetMyTeamPanelView {
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
        ]
    }
}

impl SplitPanel for MyTeamPanel {
    fn index(&self) -> Option<usize> {
        if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
            return self.game_index;
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
            return self.planet_index;
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Shipyard {
            return Some(self.spaceship_upgrade_index);
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Asteroids {
            return self.asteroid_index;
        }

        // we should always have at least 1 player
        self.player_index
    }

    fn max_index(&self) -> usize {
        if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
            return self.past_game_ids.len();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Market {
            return self.planet_markets.len();
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Shipyard {
            return SpaceshipUpgradeTarget::iter().count();
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
        } else if self.active_list == PanelList::Bottom && self.view == MyTeamView::Games {
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
