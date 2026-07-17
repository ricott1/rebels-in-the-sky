use super::traits::{HelpContent, HelpPanel, Screen, SplitPanel};
use crate::game_engine::timer::Period;
use crate::image::utils::open_image;
use crate::types::{HashMapWithResult, PlayerId, Tick};
use crate::ui::checkbox::Checkbox;
use crate::ui::dropdown::{Dropdown, DropdownState, OpenDirection};
use crate::ui::ui_frame::UiFrame;
use crate::ui::ui_key;
use crate::ui::ui_screen::{tab_link, UiTab};
use crate::ui::utils::img_to_lines;
use crate::ui::PopupMessage;
use crate::ui::{
    button::Button,
    clickable_list::ClickableListState,
    clickable_table::{ClickableCell, ClickableRow, ClickableTable, ClickableTableState},
    constants::*,
    gif_map::GifMap,
    renders::*,
    traits::{PercentageRating, UiStyled},
    ui_callback::UiCallback,
    utils::format_satoshi,
};
use crate::{
    core::*,
    game_engine::game::Game,
    game_engine::tactic::Tactic,
    game_engine::types::{GamePositionFluidity, InGameDrinking, SubstitutionTendency},
    store::load_game,
    types::{AppResult, GameId, PlanetId, StorableResourceMap, SystemTimeTick, TeamId},
};
use anyhow::anyhow;
use core::fmt::Debug;
use itertools::Itertools;
use rand_distr::num_traits::Signed;
use ratatui::crossterm;
use ratatui::crossterm::event::KeyCode;
use ratatui::style::{Styled, Stylize};
use ratatui::text::Text;
use ratatui::{
    layout::Margin,
    prelude::{Constraint, Layout, Rect},
    symbols::{border, line},
    text::{Line, Span},
    widgets::{Borders, Paragraph, Wrap},
};
use std::collections::HashMap;
use strum::IntoEnumIterator;

const DROPDOWN_WIDTH: u16 = MAX_NAME_LENGTH as u16 + 2;
const ROLE_COLUMN_WIDTH: u16 = 9;
const ROLE_COLUMN_RIGHT_OFFSET: u16 = 15 + 17 + 2;
const TRAINING_COLUMN_WIDTH: u16 = 10;
const TRAINING_COLUMN_RIGHT_OFFSET: u16 = ROLE_COLUMN_RIGHT_OFFSET + ROLE_COLUMN_WIDTH + 1;
const POSITION_COLUMN_WIDTH: u16 = 9;
const POSITION_COLUMN_RIGHT_OFFSET: u16 = TRAINING_COLUMN_RIGHT_OFFSET + TRAINING_COLUMN_WIDTH + 4;
// (col, row) offset of each position's dropdown within the court image.
const DROPDOWN_OFFSETS: [(u16, u16); NUM_GAME_POSITIONS as usize] = [
    (7, 2),   // PG
    (25, 4),  // SG
    (2, 8),   // SF
    (24, 10), // PF
    (10, 12), // C
];
const TACTIC_DROPDOWN_ID: usize = usize::MAX;
const SUBSTITUTION_DROPDOWN_ID: usize = usize::MAX - 1;
const FLUIDITY_DROPDOWN_ID: usize = usize::MAX - 2;
const DRINKING_DROPDOWN_ID: usize = usize::MAX - 3;
const TRAINING_DROPDOWN_ID: usize = usize::MAX - 4;
const ROLE_DROPDOWN_ID: usize = usize::MAX - 5;
const POSITION_DROPDOWN_ID: usize = usize::MAX - 6;

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum MyTeamView {
    #[default]
    Info,
    GameSettings,
    Games,
    Market,
    Shipyard,
    Asteroids,
}

impl MyTeamView {
    const fn next(&self) -> Self {
        match self {
            Self::Info => Self::GameSettings,
            Self::GameSettings => Self::Games,
            Self::Games => Self::Market,
            Self::Market => Self::Shipyard,
            Self::Shipyard => Self::Asteroids,
            Self::Asteroids => Self::Info,
        }
    }

    const fn previous(&self) -> Self {
        match self {
            Self::Info => Self::Asteroids,
            Self::GameSettings => Self::Info,
            Self::Games => Self::GameSettings,
            Self::Market => Self::Games,
            Self::Shipyard => Self::Market,
            Self::Asteroids => Self::Shipyard,
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
    players_table: ClickableTable<'static>,
    players_table_state: ClickableTableState,
    planet_list_state: ClickableListState,
    game_list_state: ClickableListState,
    spaceship_upgrade_list_state: ClickableListState,
    asteroid_list_state: ClickableListState,
    game_roster_widget: Paragraph<'static>,
    position_dropdowns: Vec<DropdownState>,
    setting_dropdowns: HashMap<usize, DropdownState>,
}

impl MyTeamPanel {
    pub fn new() -> Self {
        let game_roster_widget = {
            let img = open_image("game/half_court.png")
                .expect("Should be able to create half_court image");
            Paragraph::new(img_to_lines(&img))
        };
        Self {
            game_roster_widget,
            position_dropdowns: (0..NUM_GAME_POSITIONS as usize)
                .map(DropdownState::new)
                .collect(),
            ..Default::default()
        }
    }

    fn render_view_buttons(&self, frame: &mut UiFrame, area: Rect) -> AppResult<()> {
        let mut view_info_button = Button::new(
            "Info",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Info,
            },
        )
        .bold()
        .hover_text("View crew information.");

        let mut view_team_button = Button::new(
            "Game Settings",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::GameSettings,
            },
        )
        .bold()
        .hover_text("View team information.");

        let mut view_games_button = Button::new(
            "Games",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Games,
            },
        )
        .bold()
        .hover_text("View recent games.");

        let mut view_market_button = Button::new(
            "Market",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Market,
            },
        )
        .bold()
        .hover_text("View market, buy and sell resources.");

        let mut view_shipyard_button = Button::new(
            "Shipyard",
            UiCallback::SetMyTeamPanelView {
                view: MyTeamView::Shipyard,
            },
        )
        .bold()
        .hover_text("View shipyard, improve your spaceship.");

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
        .hover_text("View asteorids found during exploration.");

        match self.view {
            MyTeamView::Info => view_info_button.select(),
            MyTeamView::GameSettings => view_team_button.select(),
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

    fn render_market(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(48), Constraint::Min(48)]).split(area);
        self.render_planet_markets(frame, world, split[0])?;

        let own_team = world.get_own_team()?;
        match own_team.current_location {
            TeamLocation::OnPlanet { planet_id } => {
                let planet = world.planets.get_or_err(&planet_id)?;
                render_market_on_planet(frame, world, own_team, planet, split[1])?;
            }
            TeamLocation::Travelling { .. } => {
                frame.render_widget(default_block().title("Market"), area);
                frame.render_widget(
                    Paragraph::new("There is no market available while travelling.").centered(),
                    split[1],
                );
            }
            TeamLocation::Exploring { .. } => {
                frame.render_widget(default_block().title("Market"), area);
                frame.render_widget(
                    Paragraph::new("There is no market available while exploring.").centered(),
                    split[1],
                );
            }
            TeamLocation::OnSpaceAdventure { .. } => {
                // This sbhould be unreachable
                return Err(anyhow!("Team is on a space adventure"));
            }
        };

        Ok(())
    }

    fn render_planet_markets(
        &mut self,
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
        self.planet_list_state.select(self.planet_index);
        frame.render_stateful_interactive_widget(
            list,
            split[0].inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut self.planet_list_state,
        );

        let planet_id =
            self.planet_markets[self.planet_index.unwrap_or_default() % self.planet_markets.len()];
        let planet = world.planets.get_or_err(&planet_id)?;
        let merchant_bonus = TeamBonus::Bargaining.current_team_bonus(world, &own_team.id)?;

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

    fn render_team_settings(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let split = Layout::horizontal([Constraint::Length(60), Constraint::Fill(1)]).split(area);

        frame.render_widget(default_block().title("Game Roster"), split[0]);
        let pitch_split = Layout::horizontal([Constraint::Length(41), Constraint::Length(17)])
            .split(split[0].inner(Margin::new(1, 1)));

        frame.render_widget(default_block().title("Game Settings"), split[1]);
        let settings_split = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(split[1].inner(Margin::new(2, 1)));

        // Render team in game positions
        let game_roster_area = pitch_split[0];
        frame.render_widget(&self.game_roster_widget, game_roster_area);

        let options = own_team
            .player_ids
            .iter()
            .map(|id| {
                let player = world.players.get_or_err(id)?;
                Ok(Text::from(player.info.short_name()))
            })
            .collect::<AppResult<Vec<Text>>>()?;

        // Bench/out dropdowns stack in the fill area to the right of the court image.
        let bench_area = pitch_split[1].inner(Margin::new(1, 0));
        let court = NUM_GAME_POSITIONS as usize;
        let num_dropdowns = self.position_dropdowns.len();

        let selected_player_index = self.player_index.and_then(|index| {
            let sorted_players = own_team
                .player_ids
                .iter()
                .map(|id| world.players.get(id).unwrap())
                .collect_vec()
                .sort_by_rating();
            let player = sorted_players[index.min(sorted_players.len() - 1)];
            own_team.player_ids.iter().position(|id| *id == player.id)
        });

        for idx in 0..num_dropdowns {
            let (rect, direction, title) = if idx < court {
                let (ox, oy) = DROPDOWN_OFFSETS[idx];
                let rect = Rect::new(
                    game_roster_area.x + ox,
                    game_roster_area.y + oy,
                    DROPDOWN_WIDTH,
                    3,
                );
                let direction = if idx < 3 {
                    OpenDirection::Down
                } else {
                    OpenDirection::Up
                };
                (
                    rect,
                    direction,
                    format!("{}:{}", idx + 1, (idx as GamePosition).as_role()),
                )
            } else {
                let slot = (idx - court) as u16; // 0-based bench/out slot
                let rect = Rect::new(
                    bench_area.x,
                    bench_area.y + slot * 3,
                    DROPDOWN_WIDTH.min(bench_area.width),
                    3,
                );
                let title = if idx < MAX_PLAYERS_PER_GAME {
                    format!("{}:{}", idx + 1, (idx as GamePosition).as_role())
                } else {
                    "Out".to_string()
                };
                (rect, OpenDirection::Down, title)
            };

            let is_open = self.position_dropdowns[idx].is_open();
            let player_ids = own_team.player_ids.clone();
            let position = idx as GamePosition;
            let mut dropdown = Dropdown::new(
                idx,
                options.clone(),
                Box::new(move |index| UiCallback::SwapPlayerPositions {
                    player_id: player_ids[index],
                    position,
                }),
            )
            .open_direction(direction)
            .hover_text(format!(
                "Set player initial position to {}.",
                position.as_role()
            ))
            .title(title)
            .block(default_block());
            if idx < MAX_PLAYERS_PER_GAME {
                if let Some(index) = selected_player_index {
                    dropdown =
                        dropdown.hotkey_select(ui_key::team::set_player_position(position), index);
                }
            }
            frame.render_layered_stateful_interactive_widget(
                dropdown,
                rect,
                &mut self.position_dropdowns[idx],
                if is_open { 1 } else { 0 },
            );
        }

        let can_change_team_settings = own_team.can_change_team_settings();
        let btm_split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(settings_split[0]);

        let drinking_variants: Vec<InGameDrinking> = InGameDrinking::iter().collect();
        let drinking_options: Vec<Text> = drinking_variants
            .iter()
            .map(|t| Text::from(t.to_string()))
            .collect();
        let drinking_is_open = self
            .setting_dropdowns
            .get(&DRINKING_DROPDOWN_ID)
            .map_or(false, |d| d.is_open());
        let drinking_dropdown = Dropdown::new(
            DRINKING_DROPDOWN_ID,
            drinking_options,
            Box::new(move |index| UiCallback::SetTeamInGameDrinking {
                in_game_drinking: drinking_variants[index],
            }),
        )
        .hotkey(ui_key::team::SET_IN_GAME_DRINKING)
        .title("In-game drinking")
        .hover_text(format!(
            "{}: {}",
            own_team.in_game_drinking,
            own_team.in_game_drinking.description()
        ))
        .open_direction(OpenDirection::Down)
        .disabled(can_change_team_settings.is_err())
        .block(default_block());
        frame.render_layered_stateful_interactive_widget(
            drinking_dropdown,
            btm_split[3],
            self.setting_dropdowns
                .entry(DRINKING_DROPDOWN_ID)
                .or_default(),
            if drinking_is_open { 1 } else { 0 },
        );

        let fluidity_variants: Vec<GamePositionFluidity> = GamePositionFluidity::iter().collect();
        let fluidity_options: Vec<Text> = fluidity_variants
            .iter()
            .map(|t| Text::from(t.to_string()))
            .collect();
        let fluidity_is_open = self
            .setting_dropdowns
            .get(&FLUIDITY_DROPDOWN_ID)
            .map_or(false, |d| d.is_open());
        let fluidity_dropdown = Dropdown::new(
            FLUIDITY_DROPDOWN_ID,
            fluidity_options,
            Box::new(move |index| UiCallback::SetTeamGamePositionFluidity {
                game_position_fluidity: fluidity_variants[index],
            }),
        )
        .hotkey(ui_key::team::SET_GAME_POSITION_FLUIDITY)
        .title("Position fluidity")
        .hover_text(format!(
            "{}: {}",
            own_team.game_position_fluidity,
            own_team.game_position_fluidity.description()
        ))
        .open_direction(OpenDirection::Down)
        .disabled(can_change_team_settings.is_err())
        .block(default_block());
        frame.render_layered_stateful_interactive_widget(
            fluidity_dropdown,
            btm_split[2],
            self.setting_dropdowns
                .entry(FLUIDITY_DROPDOWN_ID)
                .or_default(),
            if fluidity_is_open { 1 } else { 0 },
        );

        let sub_variants: Vec<SubstitutionTendency> = SubstitutionTendency::iter().collect();
        let sub_options: Vec<Text> = sub_variants
            .iter()
            .map(|t| Text::from(t.to_string()))
            .collect();
        let sub_is_open = self
            .setting_dropdowns
            .get(&SUBSTITUTION_DROPDOWN_ID)
            .map_or(false, |d| d.is_open());
        let substitution_dropdown = Dropdown::new(
            SUBSTITUTION_DROPDOWN_ID,
            sub_options,
            Box::new(move |index| UiCallback::SetTeamSubstitutionTendency {
                substitution_tendency: sub_variants[index],
            }),
        )
        .hotkey(ui_key::team::SET_SUBSTITUTION_TENDENCY)
        .title("Substitutions")
        .hover_text(format!(
            "{}: {}",
            own_team.substitution_tendency,
            own_team.substitution_tendency.description()
        ))
        .open_direction(OpenDirection::Down)
        .disabled(can_change_team_settings.is_err())
        .block(default_block());
        frame.render_layered_stateful_interactive_widget(
            substitution_dropdown,
            btm_split[1],
            self.setting_dropdowns
                .entry(SUBSTITUTION_DROPDOWN_ID)
                .or_default(),
            if sub_is_open { 1 } else { 0 },
        );

        let tactics: Vec<Tactic> = Tactic::iter().collect();
        let tactic_options: Vec<Text> = tactics.iter().map(|t| Text::from(t.to_string())).collect();
        let tactic_is_open = self
            .setting_dropdowns
            .get(&TACTIC_DROPDOWN_ID)
            .map_or(false, |d| d.is_open());
        let tactic_dropdown = Dropdown::new(
            TACTIC_DROPDOWN_ID,
            tactic_options,
            Box::new(move |index| UiCallback::SetTeamTactic {
                tactic: tactics[index],
            }),
        )
        .hotkey(ui_key::team::SET_TACTIC)
        .title("tactic")
        .hover_text(format!(
            "{}: {}",
            own_team.game_tactic,
            own_team.game_tactic.description()
        ))
        .open_direction(OpenDirection::Down)
        .disabled(can_change_team_settings.is_err())
        .block(default_block());
        frame.render_layered_stateful_interactive_widget(
            tactic_dropdown,
            btm_split[0],
            self.setting_dropdowns
                .entry(TACTIC_DROPDOWN_ID)
                .or_default(),
            if tactic_is_open { 1 } else { 0 },
        );

        let right_btm_split = Layout::vertical([4, 3]).split(settings_split[2]);
        frame.render_widget(
            default_block().title("Accept challenges"),
            right_btm_split[0],
        );

        let cb_split = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)])
            .split(right_btm_split[0].inner(Margin::new(1, 1)));
        let local_challenge_button = Checkbox::no_box(
            "local  ",
            UiCallback::ToggleTeamAutonomousStrategyForLocalChallenges,
            own_team.autonomous_strategy.challenge_local,
        )
        .hover_text("Accept challenges from local teams automatically.".to_string())
        .hotkey(ui_key::team::TOGGLE_ACCEPT_LOCAL_CHALLENGES);
        frame.render_interactive_widget(local_challenge_button, cb_split[0]);

        let network_challenge_button = Checkbox::no_box(
            "network",
            UiCallback::ToggleTeamAutonomousStrategyForNetworkChallenges,
            own_team.autonomous_strategy.challenge_network,
        )
        .hover_text("Accept challenges from network teams automatically.".to_string())
        .hotkey(ui_key::team::TOGGLE_ACCEPT_NETWORK_CHALLENGES);
        frame.render_interactive_widget(network_challenge_button, cb_split[1]);

        let auto_assign_button =
            Button::new("Auto-assign positions", UiCallback::AssignBestTeamPositions)
                .hover_text("Auto-assign players' initial position.")
                .hotkey(ui_key::team::AUTO_ASSIGN);
        frame.render_interactive_widget(auto_assign_button, right_btm_split[1]);

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
        frame.render_widget(default_block().title("Recent Games"), area);

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
                        action.home_score,
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

        self.game_list_state.select(self.game_index);
        frame.render_stateful_interactive_widget(list, v_split[0], &mut self.game_list_state);

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
            let button = Button::new(
                "Go to game",
                UiCallback::GoToGame {
                    game_id,
                    from_popup: false,
                },
            )
            .hotkey(ui_key::GO_TO_GAME)
            .hover_text("Go to game");

            frame.render_interactive_widget(button, v_split[1]);
        } else if let Some(loaded_game) = self.loaded_games.get(&game_id) {
            let button = match loaded_game {
                Ok(game) => Button::new(
                    "Go to game",
                    UiCallback::GoToLoadedGame { game: game.clone() },
                )
                .hotkey(ui_key::GO_TO_GAME)
                .hover_text("Go to game"),

                Err(err) => Button::new("Go to game", UiCallback::None)
                    .hotkey(ui_key::GO_TO_GAME)
                    .hover_text("Go to game")
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
                .ok_or_else(|| anyhow!("Unable to get past game."))?;

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
        &mut self,
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

        self.spaceship_upgrade_list_state
            .select(Some(self.spaceship_upgrade_index));
        frame.render_stateful_interactive_widget(
            list,
            h_split[0].inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut self.spaceship_upgrade_list_state,
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

        render_available_spaceship_upgrades(
            own_team.spaceship.pending_upgrade,
            possible_upgrade,
            world,
            own_team,
            frame,
            v_split[1],
        );
        self.render_upgrade_spaceship_button(possible_upgrade, own_team, frame, v_split[2])?;

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
                .hotkey(hotkey)
                .hover_text(upgrade.target.description());

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
        &mut self,
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

        self.asteroid_list_state.select(self.asteroid_index);
        frame.render_stateful_interactive_widget(
            list,
            h_split[0].inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut self.asteroid_list_state,
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
                .contains(&PlanetUpgradeTarget::TeleportationPad)
            {
                let bonus = TeamBonus::Upgrades.current_team_bonus(world, &own_team.id)?;
                Some(Upgrade::new(PlanetUpgradeTarget::TeleportationPad, bonus))
            } else if own_team.has_space_cove_on().is_none() {
                // Build space cove button
                let bonus = TeamBonus::Upgrades.current_team_bonus(world, &own_team.id)?;
                Some(Upgrade::new(PlanetUpgradeTarget::SpaceCove, bonus))
            } else {
                None
            };

            render_available_upgrades(
                asteroid.pending_upgrade,
                possible_upgrade,
                world,
                own_team,
                frame,
                v_split[1],
            )?;
            render_build_asteroid_upgrade_button(
                asteroid,
                possible_upgrade,
                own_team,
                frame,
                v_split[2],
            );
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
                .hover_text(format!("Go to {}", parent.name))
                .set_style(UiStyle::HELP_LINK),
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

        if asteroid.upgrades.contains(&PlanetUpgradeTarget::SpaceCove) {
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
            timestamp: Tick::now(),
        };

        let abandon_asteroid_button =
            Button::new("Abandon", UiCallback::PushUiPopup { popup_message })
                .hotkey(ui_key::ABANDON_ASTEROID)
                .hover_text("Abandon this asteroid (there's no way back!)")
                .block(default_block().border_style(UiStyle::WARNING));

        frame.render_interactive_widget(abandon_asteroid_button, b_split[1]);

        Ok(())
    }

    fn render_selected_player(
        &mut self,
        player: &Player,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;

        let player_id = player.id;
        let split = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(26),
            Constraint::Length(26),
        ])
        .split(area);

        let separator_set = border::Set {
            top_left: line::NORMAL.horizontal_down,
            bottom_left: line::NORMAL.horizontal_up,
            ..border::PLAIN
        };

        frame.render_widget(
            default_block()
                .borders(Borders::TOP | Borders::LEFT | Borders::BOTTOM)
                .title(format!("{}", player.info.full_name())),
            split[0],
        );
        frame.render_widget(
            default_block()
                .borders(Borders::TOP | Borders::LEFT | Borders::BOTTOM)
                .border_set(separator_set),
            split[1],
        );
        frame.render_widget(default_block().border_set(separator_set), split[2]);

        let mut info_lines = {
            let drunkenness = player.current_drunkenness(world);
            let drunkenness_style = if drunkenness.is_negative() {
                UiStyle::DRUNK
            } else {
                UiStyled::style(&((MAX_SKILL - drunkenness) / MAX_SKILL * GREEN_STYLE_SKILL))
            };

            let info_line = Line::from(vec![
                Span::raw(format!(
                    "{} {} ",
                    player.info.full_name(),
                    player.info.pronouns.to_be()
                )),
                Span::styled(
                    Player::drunkenness_description(drunkenness),
                    drunkenness_style,
                ),
                Span::raw(" and "),
                Span::styled(
                    player.satisfaction_description(),
                    UiStyled::style(&(player.satisfaction)),
                ),
            ]);
            vec![
                Line::from(format!(
                    "Joined the crew on {}",
                    player
                        .joined_team_on
                        .unwrap_or_default()
                        .formatted_as_date()
                )),
                info_line,
                Line::default(),
                Line::from(Span::styled("Opinions", UiStyle::HEADER.bold())),
            ]
        };

        for text in player.opinions.description(&world.teams) {
            info_lines.push(Line::from(format!(" - {text}")));
        }

        frame.render_widget(
            Paragraph::new(info_lines).wrap(Wrap::default()),
            split[0].inner(Margin::new(2, 1)),
        );

        let crew_lines = {
            let mut l = vec![Line::from(Span::styled(
                "Crew bonus",
                UiStyle::HEADER.bold(),
            ))];
            for bonus in TeamBonus::iter() {
                let skill = bonus.as_skill(player);
                l.push(Line::from(Span::styled(
                    format!("{} +{}%", bonus, skill.percentage()),
                    UiStyled::style(&skill),
                )));
            }
            l
        };
        frame.render_widget(
            Paragraph::new(crew_lines),
            split[1].inner(Margin::new(2, 1)),
        );

        let can_release = own_team.can_release_player(player);
        let popup_message = PopupMessage::ReleasePlayer {
            player_name: player.info.full_name(),
            player_id,
            not_enough_players_for_game: own_team.player_ids.len() - 1 < MIN_PLAYERS_PER_GAME,
            timestamp: Tick::now(),
        };
        let mut release_button = Button::new(
            format!("Fire {}", player.info.short_name()),
            UiCallback::PushUiPopup { popup_message },
        )
        .hover_text("Fire pirate from the crew!")
        .hotkey(ui_key::player::FIRE);
        if let Err(err) = can_release {
            release_button.disable(Some(err.to_string()));
        } else {
            release_button = release_button.block(default_block().border_style(UiStyle::WARNING));
        }

        let side_split = Layout::vertical([3, 3, 3]).split(split[2].inner(Margin::new(2, 1)));

        frame.render_interactive_widget(release_button, side_split[0]);

        if let Ok(drink_button) = drink_button(world, &player_id) {
            frame.render_interactive_widget(drink_button, side_split[1]);
        }

        if let Ok(gold_button) = gold_button(world, &player_id) {
            frame.render_interactive_widget(gold_button, side_split[2]);
        }

        Ok(())
    }

    fn render_roster_dropdown(
        &mut self,
        frame: &mut UiFrame,
        dropdown: Dropdown<'static>,
        id: usize,
        table_area: Rect,
        row: u16,
        right_offset: u16,
        width: u16,
        selected: usize,
    ) {
        let rect = Rect::new(
            table_area.x + table_area.width - 1 - right_offset - width,
            table_area.y + 2 + row,
            width,
            1,
        );
        let state = self.setting_dropdowns.entry(id).or_default();
        if !state.is_open() {
            state.select(selected);
        }
        let layer = if state.is_open() { 1 } else { 0 };
        frame.render_layered_stateful_interactive_widget(dropdown, rect, state, layer);
    }

    fn build_players_table(
        players: &Vec<&Player>,
        player_ids: &Vec<PlayerId>,
        table_width: u16,
    ) -> AppResult<ClickableTable<'static>> {
        let header_style = UiStyle::HEADER.bold();
        let header = ClickableRow::new(vec![
            ClickableCell::from("Name").style(header_style),
            ClickableCell::from("Overall").style(header_style),
            ClickableCell::from("Potential").style(header_style),
            ClickableCell::from("Position").style(header_style),
            ClickableCell::from(Line::from(vec![
                Span::styled("T", header_style.underlined()),
                Span::styled("raining", header_style),
            ])),
            ClickableCell::from("Role").style(header_style),
            ClickableCell::from("Crew bonus").style(header_style),
        ]);

        // Calculate the available space for the players name in order to display the
        // full or shortened version.
        let name_header_width = table_width
            .saturating_sub(
                7 + 10
                    + POSITION_COLUMN_WIDTH
                    + 4
                    + TRAINING_COLUMN_WIDTH
                    + TRAINING_COLUMN_RIGHT_OFFSET,
            )
            .max(1);

        let rows = players
            .iter()
            .map(|player| {
                let overall = player.average_skill().stars();
                let potential = player.potential.stars();
                let (position_index, _) = player_ids
                    .iter()
                    .enumerate()
                    .find(|(_, id)| **id == player.id)
                    .expect("Player id should be in player ids");
                let position = (position_index as GamePosition).as_role().to_string();

                let bonus_string_1 = match player.info.crew_role {
                    CrewRole::Pilot => {
                        let skill = TeamBonus::SpaceshipSpeed.as_skill(player);
                        Span::styled(
                            format!("{} +{}%", TeamBonus::SpaceshipSpeed, skill.percentage()),
                            UiStyled::style(&skill),
                        )
                    }
                    CrewRole::Captain => {
                        let skill = TeamBonus::Reputation.as_skill(player);
                        Span::styled(
                            format!("{} +{}%", TeamBonus::Reputation, skill.percentage()),
                            UiStyled::style(&skill),
                        )
                    }
                    CrewRole::Doctor => {
                        let skill = TeamBonus::TirednessRecovery.as_skill(player);
                        Span::styled(
                            format!("{} +{}%", TeamBonus::TirednessRecovery, skill.percentage()),
                            UiStyled::style(&skill),
                        )
                    }
                    CrewRole::Engineer => {
                        let skill = TeamBonus::Weapons.as_skill(player);
                        Span::styled(
                            format!("{} +{}%", TeamBonus::Weapons, skill.percentage()),
                            UiStyled::style(&skill),
                        )
                    }
                    CrewRole::Mozzo => Span::default(),
                };

                let bonus_string_2 = match player.info.crew_role {
                    CrewRole::Pilot => {
                        let skill = TeamBonus::Scouting.as_skill(player);
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::Scouting, skill.percentage()),
                            UiStyled::style(&skill),
                        )
                    }
                    CrewRole::Captain => {
                        let skill = TeamBonus::Bargaining.as_skill(player);
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::Bargaining, skill.percentage()),
                            UiStyled::style(&skill),
                        )
                    }
                    CrewRole::Doctor => {
                        let skill = TeamBonus::Training.as_skill(player);
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::Training, skill.percentage()),
                            UiStyled::style(&skill),
                        )
                    }
                    CrewRole::Engineer => {
                        let skill = TeamBonus::Upgrades.as_skill(player);
                        Span::styled(
                            format!(" {} +{}%", TeamBonus::Upgrades, skill.percentage()),
                            UiStyled::style(&skill),
                        )
                    }
                    CrewRole::Mozzo => Span::default(),
                };

                let name = if name_header_width >= 2 * MAX_NAME_LENGTH as u16 + 2 {
                    player.info.full_name()
                } else {
                    player.info.short_name()
                };
                let training = match player.training_focus {
                    Some(focus) => focus.to_string(),
                    None => "General".to_string(),
                };
                let cells = [
                    ClickableCell::from(name),
                    ClickableCell::from(overall),
                    ClickableCell::from(potential),
                    ClickableCell::from(position),
                    ClickableCell::from(training),
                    ClickableCell::from(player.info.crew_role.to_string()),
                    ClickableCell::from(bonus_string_1),
                    ClickableCell::from(bonus_string_2),
                ];
                Ok(ClickableRow::new(cells))
            })
            .collect::<AppResult<Vec<ClickableRow>>>();

        let table = ClickableTable::new(rows?)
            .header(header)
            .column_spacing(1)
            .widths(&[
                Constraint::Min(MAX_NAME_LENGTH as u16 + 2),
                Constraint::Length(7),
                Constraint::Length(10),
                Constraint::Length(POSITION_COLUMN_WIDTH),
                Constraint::Length(TRAINING_COLUMN_WIDTH),
                Constraint::Length(ROLE_COLUMN_WIDTH),
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
        let sorted_players = own_team
            .player_ids
            .iter()
            .map(|id| world.players.get(id).unwrap())
            .collect_vec()
            .sort_by_rating();

        let player_index = if let Some(index) = self.player_index {
            index.min(own_team.player_ids.len() - 1)
        } else {
            return Ok(());
        };
        let player = sorted_players[player_index];

        let top_split =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(60)]).split(area);

        let table_split = Layout::vertical([
            Constraint::Length(MAX_CREW_SIZE as u16 + 3),
            Constraint::Fill(1),
        ])
        .split(top_split[0]);

        self.players_table_state.select(self.player_index);
        frame.render_stateful_interactive_widget(
            &self.players_table,
            table_split[0],
            &mut self.players_table_state,
        );

        render_player_description(
            player,
            &world.players_scouting,
            self.player_widget_view,
            &mut self.gif_map,
            self.tick,
            world,
            frame,
            top_split[1],
        );

        if let Some(game_id) = own_team.current_game {
            let game = world.games.get_or_err(&game_id)?;
            let text = format!(
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

            frame.render_interactive_widget(
                Button::new(
                    vec![
                        Line::default(),
                        Line::default(),
                        Line::from("Currently playing".to_string()).centered(),
                        Line::default(),
                        Line::from(text).centered(),
                        Line::from(game.timer.format()).centered(),
                    ],
                    UiCallback::GoToGame {
                        game_id,
                        from_popup: false,
                    },
                )
                .hover_text("Go to current game")
                .hotkey(ui_key::GO_TO_CURRENT_GAME)
                .block(default_block().border_style(border_style)),
                table_split[1],
            );
            return Ok(());
        }

        if let Some(tournament_id) = own_team.playing_in_tournament() {
            let tournament = world.tournaments.get_or_err(&tournament_id)?;

            frame.render_interactive_widget(
                Button::new(
                    vec![
                        Line::default(),
                        Line::default(),
                        Line::from("Currently in tournament".to_string()).centered(),
                        Line::default(),
                        Line::from(tournament.name()).centered(),
                    ],
                    UiCallback::GoToTournament {
                        tournament_id,
                        from_popup: false,
                    },
                )
                .hover_text("Go to current tournament")
                .hotkey(ui_key::GO_TO_CURRENT_GAME)
                .block(default_block().border_style(UiStyle::NETWORK)),
                table_split[1],
            );
            return Ok(());
        }

        // If this is error, we should have branched before
        assert!(own_team.can_change_team_settings().is_ok());

        let mut training_variants: Vec<Option<TrainingFocus>> = vec![None];
        let mut focus = Some(TrainingFocus::default());
        while let Some(f) = focus {
            training_variants.push(Some(f));
            focus = f.next();
        }
        let training_options: Vec<Text> = training_variants
            .iter()
            .map(|f| {
                Text::from(match f {
                    Some(focus) => focus.to_string(),
                    None => "General".to_string(),
                })
            })
            .collect();
        let selected_focus = training_variants
            .iter()
            .position(|f| *f == player.training_focus)
            .unwrap_or_default();
        let player_id = player.id;
        let training_dropdown = Dropdown::new(
            TRAINING_DROPDOWN_ID,
            training_options,
            Box::new(move |index| UiCallback::SetTrainingFocus {
                player_id,
                training_focus: training_variants[index],
            }),
        )
        .hotkey(ui_key::player::TRAINING_FOCUS)
        .hover_text("Change the training focus to change skills increase faster.")
        .open_direction(OpenDirection::Down);

        self.render_roster_dropdown(
            frame,
            training_dropdown,
            TRAINING_DROPDOWN_ID,
            table_split[0],
            player_index as u16,
            TRAINING_COLUMN_RIGHT_OFFSET,
            TRAINING_COLUMN_WIDTH,
            selected_focus,
        );

        let role_variants = CrewRole::iter().collect_vec();
        let role_options: Vec<Text> = role_variants
            .iter()
            .map(|role| Text::from(role.to_string()))
            .collect();
        let selected_role = role_variants
            .iter()
            .position(|role| *role == player.info.crew_role)
            .unwrap_or_default();
        let on_select_variants = role_variants.clone();
        let mut role_dropdown = Dropdown::new(
            ROLE_DROPDOWN_ID,
            role_options,
            Box::new(move |index| UiCallback::SetCrewRole {
                player_id,
                role: on_select_variants[index],
            }),
        )
        .hover_text("Set the pirate's crew role.")
        .open_direction(OpenDirection::Down);
        for (index, role) in role_variants.iter().enumerate() {
            role_dropdown = role_dropdown.hotkey_select(ui_key::team::set_crew_role(*role), index);
        }

        self.render_roster_dropdown(
            frame,
            role_dropdown,
            ROLE_DROPDOWN_ID,
            table_split[0],
            player_index as u16,
            ROLE_COLUMN_RIGHT_OFFSET,
            ROLE_COLUMN_WIDTH,
            selected_role,
        );

        let num_positions = own_team.player_ids.len();
        let position_options: Vec<Text> = (0..num_positions)
            .map(|idx| Text::from((idx as GamePosition).as_role().to_string()))
            .collect();
        let selected_position = own_team
            .player_ids
            .iter()
            .position(|id| *id == player.id)
            .unwrap_or_default();
        let mut position_dropdown = Dropdown::new(
            POSITION_DROPDOWN_ID,
            position_options,
            Box::new(move |index| UiCallback::SwapPlayerPositions {
                player_id,
                position: index as GamePosition,
            }),
        )
        .hover_text("Set the pirate's game position.")
        .open_direction(OpenDirection::Down);
        for idx in 0..num_positions.min(MAX_PLAYERS_PER_GAME) {
            position_dropdown = position_dropdown
                .hotkey_select(ui_key::team::set_player_position(idx as GamePosition), idx);
        }

        self.render_roster_dropdown(
            frame,
            position_dropdown,
            POSITION_DROPDOWN_ID,
            table_split[0],
            player_index as u16,
            POSITION_COLUMN_RIGHT_OFFSET,
            6,
            selected_position,
        );

        self.render_selected_player(player, frame, world, table_split[1])?;

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
        if let Ok(space_adventure_button) = space_adventure_button(world, own_team) {
            frame.render_interactive_widget(space_adventure_button, explore_split[0]);
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

    pub const fn set_view(&mut self, view: MyTeamView) {
        self.view = view;
    }

    pub const fn reset_view(&mut self) {
        self.set_view(MyTeamView::Info);
    }
}

impl Screen for MyTeamPanel {
    fn tick(&mut self) {
        self.tick += 1;
    }

    fn dropdown(&mut self, id: usize) -> Option<&mut DropdownState> {
        for (idx, dropdown) in self.position_dropdowns.iter_mut().enumerate() {
            if id != idx {
                dropdown.close();
            }
        }
        for (other_id, dropdown) in self.setting_dropdowns.iter_mut() {
            if *other_id != id {
                dropdown.close();
            }
        }

        if self.setting_dropdowns.contains_key(&id) {
            self.setting_dropdowns.get_mut(&id)
        } else {
            self.position_dropdowns.get_mut(id)
        }
    }

    fn has_open_dropdown(&self) -> Option<usize> {
        if let Some((id, _)) = self.setting_dropdowns.iter().find(|(_, d)| d.is_open()) {
            return Some(*id);
        }
        self.position_dropdowns.iter().position(|d| d.is_open())
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        self.own_team_id = world.own_team_id;
        let own_team = world.get_own_team()?;

        self.current_planet_id = match world.get_own_team()?.current_location {
            TeamLocation::OnPlanet { planet_id } => Some(planet_id),
            _ => None,
        };

        if self.planet_markets.is_empty() || world.dirty_ui {
            let coves: HashMap<_, _> = world
                .teams
                .values()
                .filter_map(|team| team.space_cove.as_ref().map(|cove| (cove.planet_id, cove)))
                .collect();
            self.planet_markets = world
                .planets
                .iter()
                .filter(|(_, planet)| planet.has_market(coves.get(&planet.id).copied()))
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
            // Add a dropdown for a hired player, drop one for a fired player,
            // then fix selections if they diverged.
            let num_players = own_team.player_ids.len();
            self.position_dropdowns.truncate(num_players);
            while self.position_dropdowns.len() < num_players {
                let idx = self.position_dropdowns.len();
                self.position_dropdowns.push(DropdownState::new(idx));
            }
            for (index, dropdown) in self.position_dropdowns.iter_mut().enumerate() {
                dropdown.select(index);
            }

            let current_settings = [
                (
                    TACTIC_DROPDOWN_ID,
                    Tactic::iter()
                        .position(|t| t == own_team.game_tactic)
                        .unwrap_or(0),
                ),
                (
                    SUBSTITUTION_DROPDOWN_ID,
                    SubstitutionTendency::iter()
                        .position(|t| t == own_team.substitution_tendency)
                        .unwrap_or(0),
                ),
                (
                    FLUIDITY_DROPDOWN_ID,
                    GamePositionFluidity::iter()
                        .position(|t| t == own_team.game_position_fluidity)
                        .unwrap_or(0),
                ),
                (
                    DRINKING_DROPDOWN_ID,
                    InGameDrinking::iter()
                        .position(|t| t == own_team.in_game_drinking)
                        .unwrap_or(0),
                ),
            ];
            for (id, index) in current_settings {
                let dropdown = self.setting_dropdowns.entry(id).or_default();
                if !dropdown.is_open() {
                    dropdown.select(index);
                }
            }

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

            let sorted_players = own_team
                .player_ids
                .iter()
                .map(|id| world.players.get(id).unwrap())
                .collect_vec()
                .sort_by_rating();

            let table_width = UI_SCREEN_SIZE.0 - 60;
            self.players_table =
                Self::build_players_table(&sorted_players, &own_team.player_ids, table_width)?
                    .block(default_block().title(format!(
                        "{} {} ↓/↑",
                        own_team.name,
                        world.team_rating(&own_team.id).unwrap_or_default().stars()
                    )));
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
            MyTeamView::GameSettings => self.render_team_settings(frame, world, bottom_split[1])?,
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
            ui_key::CYCLE_VIEW_BACK => {
                return Some(UiCallback::SetMyTeamPanelView {
                    view: self.view.previous(),
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

impl HelpPanel for MyTeamPanel {
    fn help_content(&self) -> HelpContent {
        HelpContent {
            description: [
                "The captain's bridge: manage roster, training, tactics, ships, markets, asteroids, and games.",
                "Use Tab to cycle the inner view.",
                "",
                "Recruit new pirates from the Pirates panel.",
                "Scout rivals and challenge them from Crews.",
                "Watch your scheduled or finished games in Games.",
                "Travel between planets via the Galaxy star map.",
            ]
            .join("\n"),
            links: vec![
                tab_link("Pirates", UiTab::Pirates),
                tab_link("Crews", UiTab::Crews),
                tab_link("Games", UiTab::Games),
                tab_link("Galaxy", UiTab::Galaxy),
            ],
            controls: vec![
                Line::from("Controls:"),
                Line::from(format!(
                    "  {}        Cycle view (Info/Game Settings/Games/Market/Shipyard/Asteroids)",
                    ui_key::CYCLE_VIEW
                )),
                Line::from("  ↑/↓        Move highlight in the active list"),
                Line::from(format!(
                    "  {}/{}/{}/{}/{}  Set highlighted pirate as captain/doctor/engineer/pilot/mozzo",
                    ui_key::team::SET_CAPTAIN,
                    ui_key::team::SET_DOCTOR,
                    ui_key::team::SET_ENGINEER,
                    ui_key::team::SET_PILOT,
                    ui_key::team::SET_MOZZO,
                )),
                Line::from("  1-7        Place highlighted pirate in that game position"),
                Line::from(format!(
                    "  {}      Fire highlighted pirate",
                    ui_key::player::FIRE
                )),
                Line::default(),
                Line::from("  Game settings view"),
                Line::from(format!(
                    "  {} / {}      Set training focus / cycle tactic",
                    ui_key::player::TRAINING_FOCUS,
                    ui_key::team::SET_TACTIC
                )),
                Line::from(format!(
                    "  {} / {}      Cycle substitution tendency / game position fluidity",
                    ui_key::team::SET_SUBSTITUTION_TENDENCY,
                    ui_key::team::SET_GAME_POSITION_FLUIDITY
                )),
                Line::default(),
                Line::from("  Market view"),
                Line::from(format!(
                    "  {}/{}/{}/{}    Buy gold/scraps/fuel/rum",
                    ui_key::market::BUY_GOLD,
                    ui_key::market::BUY_SCRAPS,
                    ui_key::market::BUY_FUEL,
                    ui_key::market::BUY_RUM,
                )),
                Line::from(format!(
                    "  {}/{}/{}/{}    Sell gold/scraps/fuel/rum",
                    ui_key::market::SELL_GOLD,
                    ui_key::market::SELL_SCRAPS,
                    ui_key::market::SELL_FUEL,
                    ui_key::market::SELL_RUM,
                )),
            ],
        }
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
