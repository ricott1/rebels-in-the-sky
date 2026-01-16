use super::button::Button;
use super::clickable_list::ClickableListState;
use super::constants::*;
use super::gif_map::GifMap;
use super::traits::SplitPanel;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::utils::{format_satoshi, validate_textarea_input};
use super::widgets::{thick_block, PlayerWidgetView};
use super::{
    constants::UiStyle,
    traits::Screen,
    utils::{img_to_lines, input_from_key_event},
    widgets::{default_block, render_player_description, selectable_list},
};
use crate::image::utils::LightMaskStyle;
use crate::image::{color_map::ColorPreset, spaceship::SPACESHIP_IMAGE_WIDTH};
use crate::types::HashMapWithResult;
use crate::{
    core::*,
    image::color_map::ColorMap,
    types::{AppResult, PlanetId, PlayerId},
};
use core::fmt::Debug;
use core::panic;
use crossterm::event::{KeyCode, KeyEvent};
use itertools::Itertools;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ratatui::style::Styled;
use ratatui::text::Line;
use ratatui::{
    prelude::{Constraint, Layout, Margin, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Clear, Paragraph, Wrap},
};
use std::cmp::min;
use std::collections::HashMap;
use strum::IntoEnumIterator;
use tui_textarea::{CursorMove, TextArea};

const INITIAL_TEAM_SIZE: usize = 5;
const SPACESHIP_MODELS: [SpaceshipPrefab; 3] = [
    SpaceshipPrefab::Bresci,
    SpaceshipPrefab::Orwell,
    SpaceshipPrefab::Ibarruri,
];

#[derive(Debug, Default, PartialOrd, PartialEq)]
pub enum CreationState {
    #[default]
    TeamName,
    ShipName,
    Planet,
    Jersey,
    ShipModel,
    Players,
    Done,
}

impl CreationState {
    pub fn next(&self) -> Self {
        match self {
            CreationState::TeamName => CreationState::ShipName,
            CreationState::ShipName => CreationState::Planet,
            CreationState::Planet => CreationState::Jersey,
            CreationState::Jersey => CreationState::ShipModel,
            CreationState::ShipModel => CreationState::Players,
            CreationState::Players => CreationState::Done,
            CreationState::Done => CreationState::Done,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            CreationState::TeamName => CreationState::TeamName,
            CreationState::ShipName => CreationState::TeamName,
            CreationState::Planet => CreationState::ShipName,
            CreationState::Jersey => CreationState::Planet,
            CreationState::ShipModel => CreationState::Jersey,
            CreationState::Players => CreationState::ShipModel,
            CreationState::Done => CreationState::Players,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
enum ConfirmChoice {
    #[default]
    Yes,
    No,
}

#[derive(Debug, Default)]
pub struct NewTeamScreen {
    state: CreationState,
    tick: usize,
    team_name_textarea: TextArea<'static>,
    ship_name_textarea: TextArea<'static>,
    spaceship_model_index: usize,
    planet_index: usize,
    planet_ids: Vec<PlanetId>,
    jersey_styles: Vec<JerseyStyle>,
    jersey_style_index: usize,
    red_color_preset: ColorPreset,
    green_color_preset: ColorPreset,
    blue_color_preset: ColorPreset,
    player_index: usize,
    // Map of planet_id -> (player_id, hiring cost)
    planet_players: HashMap<PlanetId, Vec<(PlayerId, u32)>>,
    selected_players: Vec<PlayerId>,
    confirm: ConfirmChoice,
    gif_map: GifMap,
}

impl NewTeamScreen {
    pub fn new() -> Self {
        let mut team_name_textarea = TextArea::default();
        team_name_textarea.set_cursor_style(UiStyle::SELECTED);
        team_name_textarea.set_block(
            default_block()
                .border_style(UiStyle::DEFAULT)
                .title("Team name"),
        );
        let mut ship_name_textarea = TextArea::default();
        ship_name_textarea.set_cursor_style(UiStyle::DEFAULT);
        ship_name_textarea.set_block(
            default_block()
                .border_style(UiStyle::UNSELECTABLE)
                .title("Spaceship name"),
        );
        let rng = &mut ChaCha8Rng::from_os_rng();
        let mut color_presets = ColorPreset::iter().collect_vec();
        color_presets.shuffle(rng);
        let red_color_preset = color_presets[0];
        let green_color_preset = color_presets[1];
        let blue_color_preset = color_presets[2];

        let jersey_styles = JerseyStyle::iter()
            .filter(|jersey_style| jersey_style.is_available_at_creation())
            .collect_vec();

        Self {
            team_name_textarea,
            ship_name_textarea,
            red_color_preset,
            green_color_preset,
            blue_color_preset,
            jersey_styles,
            ..Default::default()
        }
    }

    fn selected_ship(&self) -> Spaceship {
        let prefab = SPACESHIP_MODELS[self.spaceship_model_index];
        let name = self.ship_name_textarea.lines()[0].clone();
        let color_map = self.get_team_colors();
        prefab.spaceship().with_name(name).with_color_map(color_map)
    }

    fn get_team_colors(&self) -> ColorMap {
        ColorMap {
            red: self.red_color_preset.to_rgb(),
            green: self.green_color_preset.to_rgb(),
            blue: self.blue_color_preset.to_rgb(),
        }
    }

    pub fn set_team_colors(&mut self, color: ColorPreset, channel: usize) {
        match channel {
            0 => self.red_color_preset = color,
            1 => self.green_color_preset = color,
            2 => self.blue_color_preset = color,
            _ => panic!("Invalid color index"),
        }
    }

    pub fn clear_selected_players(&mut self) {
        self.selected_players.clear();
    }

    pub fn set_state(&mut self, state: CreationState) {
        self.state = state;
        self.set_index(0);
    }

    fn render_intro(&mut self, frame: &mut UiFrame, area: Rect) {
        let text = "
        It's the year 2101. Corporations have taken over the world. 
        The only way to be free is to join a pirate crew and start plundering the galaxy.
        The only means of survival is to play basketball.

        Now it's your turn to go out there and make a name for yourself.
        Create your crew and start wandering the galaxy in search of worthy basketball opponents.
        
        Choose your team name, customize your ship, and select a worthy crew.
        You won't keep any leftover money, so spend wisely!

        [Press enter to confirm selections.]"
            .to_string();

        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph.wrap(Wrap { trim: true }).centered(), area);

        // Render main block
        frame.render_widget(default_block(), area);
    }

    fn render_spaceship(&mut self, frame: &mut UiFrame, area: Rect) {
        let split = Layout::horizontal([
            Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(1),
        ])
        .split(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        if let Ok(gif) = self
            .selected_ship()
            .compose_image(Some(LightMaskStyle::radial()))
        {
            let img = gif[(self.tick) % gif.len()].clone();
            let paragraph = Paragraph::new(img_to_lines(&img));
            frame.render_widget(
                paragraph.centered(),
                split[0].inner(Margin {
                    vertical: 0,
                    horizontal: 1,
                }),
            );
        }
        let spaceship = self.selected_ship();
        let storage_units = 0;
        let spaceship_info = Paragraph::new(vec![
            Line::from(format!("Spaceship name: {}", spaceship.name)),
            Line::from(format!(
                "Max speed: {:.3} AU/h",
                spaceship.speed(storage_units) * HOURS as f32 / AU as f32
            )),
            Line::from(format!("Max Crew: {}", spaceship.crew_capacity())),
            Line::from(format!("Max Storage: {}", spaceship.storage_capacity())),
            Line::from(format!("Max Tank: {} t", spaceship.fuel_capacity())),
            Line::from(format!(
                "Bare consumption: {:.2} t/h",
                spaceship.fuel_consumption_per_tick(storage_units) * HOURS as f32
            )),
            Line::from(format!(
                "Max distance: {:.2} AU",
                spaceship.max_distance(spaceship.fuel_capacity()) / AU as f32
            )),
            Line::from(format!("Cost: {}", format_satoshi(spaceship.value()))),
        ]);

        frame.render_widget(
            spaceship_info,
            split[1].inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );

        // Render main block
        frame.render_widget(default_block(), area);
    }

    fn render_spaceship_selection(&self, frame: &mut UiFrame, area: Rect) {
        if self.state > CreationState::ShipModel {
            let selected_ship = SPACESHIP_MODELS[self.spaceship_model_index];
            frame.render_widget(
                Paragraph::new(format!(" {selected_ship}")).block(
                    thick_block()
                        .border_style(UiStyle::OK)
                        .title("Choose spaceship model ↓/↑"),
                ),
                area,
            );
        } else if self.state == CreationState::ShipModel {
            let options = SPACESHIP_MODELS
                .iter()
                .map(|ship| {
                    (
                        format!(
                            "{:MAX_NAME_LENGTH$} {:>6}",
                            ship,
                            format_satoshi(ship.value())
                        ),
                        UiStyle::DEFAULT,
                    )
                })
                .collect_vec();

            let list = selectable_list(options);
            frame.render_stateful_interactive_widget(
                list.block(
                    default_block()
                        .border_style(UiStyle::DEFAULT)
                        .title("Choose spaceship model ↓/↑"),
                ),
                area,
                &mut ClickableListState::default().with_selected(Some(self.spaceship_model_index)),
            );
        } else {
            frame.render_widget(
                default_block()
                    .border_style(UiStyle::UNSELECTABLE)
                    .title("Choose spaceship model ↓/↑"),
                area,
            );
        }
    }

    fn render_jersey_selection(&mut self, frame: &mut UiFrame, area: Rect) {
        if self.state > CreationState::Jersey {
            let selected_jersey_style = self.jersey_styles[self.jersey_style_index];
            frame.render_widget(
                Paragraph::new(format!(" {selected_jersey_style}")).block(
                    thick_block()
                        .border_style(UiStyle::OK)
                        .title("Choose jersey style ↓/↑"),
                ),
                area,
            );
        } else if self.state == CreationState::Jersey {
            let options = self
                .jersey_styles
                .iter()
                .map(|jersey_style| (format!("{jersey_style}"), UiStyle::DEFAULT))
                .collect_vec();

            let list = selectable_list(options);
            frame.render_stateful_interactive_widget(
                list.block(
                    default_block()
                        .border_style(UiStyle::DEFAULT)
                        .title("Choose jersey style ↓/↑"),
                ),
                area,
                &mut ClickableListState::default().with_selected(Some(self.jersey_style_index)),
            );
        } else {
            frame.render_widget(
                default_block()
                    .border_style(UiStyle::UNSELECTABLE)
                    .title("Choose jersey style ↓/↑"),
                area,
            );
        }
    }

    fn render_jersey(&self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let style = self.jersey_styles[self.jersey_style_index];
        let planet_id = self.planet_ids[self.planet_index];
        let planet_players = &self
            .planet_players
            .get(&planet_id)
            .unwrap_or_else(|| panic!("No players found on planet {}", planet_id.to_string()));
        let mut player = world.players.get_or_err(&planet_players[0].0)?.clone();
        let jersey = Jersey {
            style,
            color: self.get_team_colors(),
        };
        player.set_jersey(&jersey);

        // We cannot use the gif map because we are changing the jersey style
        if let Ok(gif) = player.compose_image() {
            let img = gif[(self.tick / 5) % gif.len()].clone();
            let paragraph = Paragraph::new(img_to_lines(&img));
            frame.render_widget(
                paragraph.centered(),
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                }),
            );
        }

        // Render main block
        frame.render_widget(default_block(), area);
        Ok(())
    }

    fn render_colors_selection(&self, frame: &mut UiFrame, area: Rect) {
        let block = if self.state > CreationState::ShipModel {
            thick_block().border_style(UiStyle::OK)
        } else if self.state >= CreationState::Jersey {
            default_block().border_style(UiStyle::DEFAULT)
        } else {
            default_block().border_style(UiStyle::UNSELECTABLE)
        };

        let color_split = Layout::horizontal([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

        if self.state >= CreationState::Jersey {
            let red_style = Style::default().bg(Color::Rgb(
                self.get_team_colors().red[0],
                self.get_team_colors().red[1],
                self.get_team_colors().red[2],
            ));
            let red = Button::no_box(
                vec![
                    Line::from(Span::styled(" ".repeat(area.width as usize / 3), red_style)),
                    Line::from(Span::styled(" ".repeat(area.width as usize / 3), red_style)),
                ],
                UiCallback::SetTeamColors {
                    color: self.red_color_preset.next(),
                    channel: 0,
                },
            );
            let green_style = Style::default().bg(Color::Rgb(
                self.get_team_colors().green[0],
                self.get_team_colors().green[1],
                self.get_team_colors().green[2],
            ));
            let green = Button::no_box(
                vec![
                    Line::from(Span::styled(
                        " ".repeat(area.width as usize / 3),
                        green_style,
                    )),
                    Line::from(Span::styled(
                        " ".repeat(area.width as usize / 3),
                        green_style,
                    )),
                ],
                UiCallback::SetTeamColors {
                    color: self.green_color_preset.next(),
                    channel: 1,
                },
            );

            let blue_style = Style::default().bg(Color::Rgb(
                self.get_team_colors().blue[0],
                self.get_team_colors().blue[1],
                self.get_team_colors().blue[2],
            ));
            let blue = Button::no_box(
                vec![
                    Line::from(Span::styled(
                        " ".repeat(area.width as usize / 3),
                        blue_style,
                    )),
                    Line::from(Span::styled(
                        " ".repeat(area.width as usize / 3),
                        blue_style,
                    )),
                ],
                UiCallback::SetTeamColors {
                    color: self.blue_color_preset.next(),
                    channel: 2,
                },
            );

            frame.render_interactive_widget(red, color_split[0].inner(Margin::new(1, 1)));
            frame.render_interactive_widget(green, color_split[1].inner(Margin::new(1, 1)));
            frame.render_interactive_widget(blue, color_split[2].inner(Margin::new(1, 1)));
        }

        frame.render_widget(block.clone().title("Choose 'r'"), color_split[0]);
        frame.render_widget(block.clone().title("Choose 'g'"), color_split[1]);
        frame.render_widget(block.title("Choose 'b'"), color_split[2]);
    }

    fn render_planet_selection(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if self.state > CreationState::Planet {
            let selected_planet = world
                .planets
                .get_or_err(&self.planet_ids[self.planet_index])?;
            frame.render_widget(
                Paragraph::new(format!(" {}", selected_planet.name.clone())).block(
                    thick_block()
                        .border_style(UiStyle::OK)
                        .title("Choose home planet ↓/↑"),
                ),
                area,
            );
        } else if self.state == CreationState::Planet {
            let options = self
                .planet_ids
                .iter()
                .map(|planet_id| {
                    let planet = world.planets.get_or_err(planet_id).unwrap();
                    (planet.name.clone(), UiStyle::DEFAULT)
                })
                .collect_vec();

            let list = selectable_list(options);
            frame.render_stateful_interactive_widget(
                list.block(
                    default_block()
                        .border_style(UiStyle::DEFAULT)
                        .title("Choose home planet ↓/↑"),
                ),
                area,
                &mut ClickableListState::default().with_selected(Some(self.planet_index)),
            );
        } else {
            frame.render_widget(
                default_block()
                    .border_style(UiStyle::UNSELECTABLE)
                    .title("Choose home planet ↓/↑"),
                area,
            );
        }
        Ok(())
    }

    fn render_planet(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let planet_id = self.planet_ids[self.planet_index];
        let planet = world.planets.get_or_err(&planet_id)?;

        let mut lines = self.gif_map.planet_zoom_in_frame_lines(
            &planet_id,
            self.tick / planet.rotation_period,
            world,
        )?;

        // Apply y-centering
        let min_offset = if lines.len() > area.height as usize {
            (lines.len() - area.height as usize) / 2
        } else {
            0
        };
        let max_offset = min(lines.len(), min_offset + area.height as usize);
        lines = lines[min_offset..max_offset].to_vec();

        // Apply x-centering
        if lines[0].spans.len() > area.width as usize - 2 {
            let min_offset = if lines[0].spans.len() > area.width as usize {
                (lines[0].spans.len() - area.width as usize) / 2
            } else {
                0
            };
            let max_offset = min(lines[0].spans.len(), min_offset + area.width as usize);
            for line in lines.iter_mut() {
                line.spans = line.spans[min_offset..max_offset].to_vec();
            }
        }

        let paragraph = Paragraph::new(lines).centered();
        frame.render_widget(
            paragraph,
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );
        frame.render_widget(default_block(), area);
        Ok(())
    }

    fn get_remaining_balance(&self) -> i32 {
        let hiring_costs = if let Some(planet_players) =
            self.planet_players.get(&self.planet_ids[self.planet_index])
        {
            let mut hiring_costs = 0_i32;
            for (player_id, hire_cost) in planet_players.iter() {
                if !self.selected_players.contains(player_id) {
                    continue;
                }
                hiring_costs += *hire_cost as i32;
            }
            hiring_costs
        } else {
            0
        };

        let ship_cost = if self.state >= CreationState::ShipModel {
            self.selected_ship().value()
        } else {
            0
        };
        INITIAL_TEAM_BALANCE as i32 - hiring_costs - ship_cost as i32
    }

    fn render_remaining_balance(&mut self, frame: &mut UiFrame, area: Rect) {
        let remaining_balance = self.get_remaining_balance();
        let text = format!(" Remaining balance: {remaining_balance:>} sat");

        let block = if remaining_balance >= 0 {
            thick_block().border_style(UiStyle::OK)
        } else {
            thick_block().border_style(UiStyle::ERROR)
        };
        frame.render_widget(Paragraph::new(text).block(block), area);
    }

    fn render_player_list(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if self.state < CreationState::Players {
            frame.render_widget(
                default_block()
                    .title(format!(
                        "Select {} players",
                        self.max_players_selected() - self.selected_players.len(),
                    ))
                    .style(UiStyle::UNSELECTABLE),
                area,
            );
            return Ok(());
        }

        let planet_id = self.planet_ids[self.planet_index];
        let planet_players = self.planet_players.get(&planet_id).unwrap();
        let options = planet_players
            .iter()
            .map(|&player_data| {
                let player_id = player_data.0;
                let mut style = UiStyle::DEFAULT;
                if self.selected_players.contains(&player_id)
                    && self.state <= CreationState::Players
                {
                    style = UiStyle::OK;
                }

                if self.state > CreationState::Players
                    && !self.selected_players.contains(&player_id)
                {
                    return ("".to_string(), style);
                }
                let player = world.players.get_or_err(&player_id).unwrap();
                let full_name = player.info.full_name();

                let max_width = 2 * MAX_NAME_LENGTH;
                let name = if full_name.len() <= max_width {
                    full_name
                } else {
                    player.info.short_name()
                };
                (
                    format!(
                        "{:max_width$}{:>9}",
                        name,
                        format_satoshi(player.hire_cost(0.0),)
                    ),
                    style,
                )
            })
            .collect_vec();

        let list = selectable_list(options);
        let block = if self.state > CreationState::Players {
            thick_block().style(UiStyle::OK)
        } else {
            default_block().style(UiStyle::DEFAULT)
        };

        let mut state = if self.state > CreationState::Players {
            ClickableListState::default().with_selected(None)
        } else {
            ClickableListState::default().with_selected(Some(self.player_index))
        };

        frame.render_stateful_interactive_widget(
            list.block(block.title(format!(
                "Select {} players",
                self.max_players_selected() - self.selected_players.len(),
            ))),
            area,
            &mut state,
        );
        Ok(())
    }

    fn render_player(&mut self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let planet_id = self.planet_ids[self.planet_index];
        let planet_players = self.planet_players.get(&planet_id).unwrap();
        let player = world
            .players
            .get_or_err(&planet_players[self.player_index].0)?;
        render_player_description(
            player,
            PlayerWidgetView::Skills,
            &mut self.gif_map,
            self.tick,
            world,
            frame,
            area,
        );
        Ok(())
    }

    fn render_confirm_box(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);
        let name = self.team_name_textarea.lines()[0].clone();
        let planet = world
            .planets
            .get_or_err(&self.planet_ids[self.planet_index])?;
        let text = Paragraph::new(vec![
            Line::from(Span::raw(format!("{} from {}", name, planet.name))),
            Line::from(Span::raw("Ready to sail the cosmic waves?")),
        ])
        .centered();
        frame.render_widget(
            text,
            split[1].inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );

        let side_width = if split[2].width > 24 {
            (split[2].width - 24) / 2
        } else {
            0
        };
        let button_split = Layout::horizontal([
            Constraint::Length(side_width),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(side_width),
        ])
        .split(split[2]);

        let yes_button = Button::new(
            UiText::YES,
            UiCallback::GeneratePlayerTeam {
                name: self.team_name_textarea.lines()[0].clone(),
                home_planet: self.planet_ids[self.planet_index],
                jersey_style: self.jersey_styles[self.jersey_style_index],
                jersey_colors: self.get_team_colors(),
                players: self.selected_players.clone(),
                spaceship: self.selected_ship(),
            },
        )
        .set_style(UiStyle::OK);
        frame.render_interactive_widget(yes_button, button_split[1]);

        let no_button =
            Button::new(UiText::NO, UiCallback::CancelGeneratePlayerTeam).set_style(UiStyle::ERROR);

        frame.render_interactive_widget(no_button, button_split[2]);
        frame.render_widget(thick_block().border_style(UiStyle::HIGHLIGHT), area);
        Ok(())
    }

    fn max_players_selected(&self) -> usize {
        // self.selected_players.len() >= self.selected_ship().crew_capacity() as usize
        let planet_id = self.planet_ids[self.planet_index];
        let planet_players = self.planet_players.get(&planet_id).unwrap();
        min(INITIAL_TEAM_SIZE, planet_players.len())
    }
    fn enough_players_selected(&self) -> bool {
        self.selected_players.len() >= self.max_players_selected()
    }
}

impl Screen for NewTeamScreen {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;

        // If planets is empty, we initialize the list of planets and planet_players
        if self.planet_ids.is_empty() {
            self.planet_ids = world
                .planets
                .keys()
                .filter(|&planet_id| {
                    if let Some(planet) = world.planets.get(planet_id) {
                        return planet.total_population() > 0;
                    }
                    false
                })
                .sorted_by(|a, b| a.cmp(b))
                .copied()
                .collect_vec();
            for player in world.players.values() {
                if player.team.is_none() {
                    let planet_players = self
                        .planet_players
                        .entry(player.info.home_planet_id)
                        .or_default();
                    planet_players.push((player.id, player.hire_cost(0.0)));
                    planet_players.sort_by(|a, b| {
                        let p1 = world
                            .players
                            .get(&a.0)
                            .map(|p| p.hire_cost(0.0))
                            .unwrap_or_default();
                        let p2 = world
                            .players
                            .get(&b.0)
                            .map(|p| p.hire_cost(0.0))
                            .unwrap_or_default();
                        p2.cmp(&p1)
                    });
                }
            }
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
        if self.planet_ids.is_empty() {
            return Ok(());
        }
        let v_split = Layout::horizontal([
            Constraint::Length(1),
            Constraint::Length(LEFT_PANEL_WIDTH), //selections
            Constraint::Min(10),                  //planet_players
            Constraint::Length(1),
        ])
        .split(area);

        let planet_split_height = if self.state == CreationState::Planet {
            self.planet_ids.len() as u16 + 2
        } else {
            3
        };

        let jersey_split_height = if self.state == CreationState::Jersey {
            self.jersey_styles.len() as u16 + 2
        } else {
            3
        };

        let ship_split_height = if self.state == CreationState::ShipModel {
            SPACESHIP_MODELS.len() as u16 + 2
        } else {
            3
        };

        let player_split_height = if self.state >= CreationState::Players {
            self.planet_players[&self.planet_ids[self.planet_index]].len() as u16 + 2
        } else {
            3
        };

        let h_split = Layout::vertical([
            Constraint::Length(3),                   // remaining balance
            Constraint::Length(3),                   // team name
            Constraint::Length(3),                   // spaceship name
            Constraint::Length(planet_split_height), // planet
            Constraint::Length(4),                   // colors
            Constraint::Length(jersey_split_height), // jersey style
            Constraint::Length(ship_split_height),   // ship
            Constraint::Length(player_split_height), // player_list
            Constraint::Min(0),                      // filler
        ])
        .split(v_split[1]);

        self.render_remaining_balance(frame, h_split[0]);

        frame.render_widget(&self.team_name_textarea, h_split[1]);
        if self.state == CreationState::TeamName {
            self.render_intro(frame, v_split[2]);
        }

        frame.render_widget(&self.ship_name_textarea, h_split[2]);
        if self.state == CreationState::ShipName {
            self.render_intro(frame, v_split[2]);
        }

        self.render_planet_selection(frame, world, h_split[3])?;
        if self.state == CreationState::Planet {
            self.render_planet(frame, world, v_split[2])?;
        }

        self.render_colors_selection(frame, h_split[4]);
        self.render_jersey_selection(frame, h_split[5]);
        if self.state == CreationState::Jersey {
            self.render_jersey(frame, world, v_split[2])?;
        }

        self.render_spaceship_selection(frame, h_split[6]);
        if self.state == CreationState::ShipModel {
            self.render_spaceship(frame, v_split[2]);
        }

        self.render_player_list(frame, world, h_split[7])?;
        if self.state == CreationState::Players {
            self.render_player(frame, world, v_split[2])?;
        }

        if self.state >= CreationState::Done {
            let width = 50;
            let height = 12;
            let x = if area.width > width {
                (area.width - width) / 2
            } else {
                0
            };
            let y = if area.height > height {
                (area.height - height) / 2
            } else {
                0
            };
            let confirm_box = frame.to_screen_rect(Rect::new(x, y, width, height));
            frame.render_widget(Clear, confirm_box);
            self.render_confirm_box(frame, world, confirm_box)?;
        }
        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, _world: &World) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            _ => {
                match self.state {
                    CreationState::TeamName => match key_event.code {
                        KeyCode::Enter => {
                            if !validate_textarea_input(&mut self.team_name_textarea, "Team name") {
                                return None;
                            }
                            let mut name = self.team_name_textarea.lines()[0].trim().to_string();
                            // Capitalize first letter of name
                            name = name
                                .chars()
                                .enumerate()
                                .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                                .collect();

                            self.team_name_textarea.move_cursor(CursorMove::End);
                            self.team_name_textarea.delete_line_by_head();
                            self.team_name_textarea.set_yank_text(name);
                            self.team_name_textarea.paste();
                            self.team_name_textarea.set_cursor_style(UiStyle::DEFAULT);

                            self.team_name_textarea.set_block(
                                thick_block().border_style(UiStyle::OK).title("Team name"),
                            );
                            self.ship_name_textarea.set_block(
                                default_block()
                                    .border_style(UiStyle::DEFAULT)
                                    .title("Spaceship name"),
                            );

                            self.ship_name_textarea.set_cursor_style(UiStyle::SELECTED);

                            self.set_state(self.state.next());
                        }
                        _ => {
                            self.team_name_textarea
                                .input(input_from_key_event(key_event));
                            validate_textarea_input(&mut self.team_name_textarea, "Team name");
                        }
                    },
                    CreationState::ShipName => match key_event.code {
                        KeyCode::Enter => {
                            if !validate_textarea_input(
                                &mut self.ship_name_textarea,
                                "Spaceship name",
                            ) {
                                return None;
                            }
                            let mut name = self.ship_name_textarea.lines()[0].trim().to_string();
                            // Capitalize first letter of name
                            name = name
                                .chars()
                                .enumerate()
                                .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                                .collect();

                            self.ship_name_textarea.move_cursor(CursorMove::End);
                            self.ship_name_textarea.delete_line_by_head();
                            self.ship_name_textarea.set_yank_text(name);
                            self.ship_name_textarea.paste();
                            self.ship_name_textarea.set_cursor_style(UiStyle::DEFAULT);

                            self.ship_name_textarea.set_block(
                                thick_block()
                                    .border_style(UiStyle::OK)
                                    .title("Spaceship name"),
                            );
                            self.set_state(self.state.next())
                        }
                        KeyCode::Backspace => {
                            if self.ship_name_textarea.lines()[0].is_empty() {
                                self.team_name_textarea
                                    .set_block(default_block().title("Team name"));
                                self.ship_name_textarea.set_block(
                                    default_block()
                                        .border_style(UiStyle::UNSELECTABLE)
                                        .title("Spaceship name"),
                                );
                                self.team_name_textarea.set_cursor_style(UiStyle::SELECTED);
                                self.ship_name_textarea.set_cursor_style(UiStyle::DEFAULT);

                                self.set_state(self.state.previous());
                            } else {
                                self.ship_name_textarea
                                    .input(input_from_key_event(key_event));
                                validate_textarea_input(
                                    &mut self.ship_name_textarea,
                                    "Spaceship name",
                                );
                            }
                        }
                        _ => {
                            self.ship_name_textarea
                                .input(input_from_key_event(key_event));
                            validate_textarea_input(&mut self.ship_name_textarea, "Spaceship name");
                        }
                    },
                    CreationState::Planet => match key_event.code {
                        KeyCode::Enter => self.set_state(self.state.next()),
                        KeyCode::Backspace => {
                            self.ship_name_textarea
                                .set_block(default_block().title("Spaceship name"));
                            self.ship_name_textarea.set_cursor_style(UiStyle::SELECTED);
                            self.set_state(self.state.previous());
                        }

                        _ => {}
                    },
                    CreationState::Jersey => match key_event.code {
                        KeyCode::Enter => {
                            self.set_state(self.state.next());
                        }
                        KeyCode::Backspace => {
                            self.set_state(self.state.previous());
                        }

                        KeyCode::Char('r') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.red_color_preset.next(),
                                channel: 0,
                            });
                        }
                        KeyCode::Char('g') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.green_color_preset.next(),
                                channel: 1,
                            });
                        }
                        KeyCode::Char('b') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.blue_color_preset.next(),
                                channel: 2,
                            });
                        }
                        KeyCode::Char('R') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.red_color_preset.previous(),
                                channel: 0,
                            });
                        }
                        KeyCode::Char('G') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.green_color_preset.previous(),
                                channel: 1,
                            });
                        }
                        KeyCode::Char('B') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.blue_color_preset.previous(),
                                channel: 2,
                            });
                        }
                        _ => {}
                    },
                    CreationState::ShipModel => match key_event.code {
                        KeyCode::Enter => self.set_state(self.state.next()),
                        KeyCode::Backspace => {
                            self.set_state(self.state.previous());
                        }

                        KeyCode::Char('r') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.red_color_preset.next(),
                                channel: 0,
                            });
                        }
                        KeyCode::Char('g') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.green_color_preset.next(),
                                channel: 1,
                            });
                        }
                        KeyCode::Char('b') => {
                            return Some(UiCallback::SetTeamColors {
                                color: self.blue_color_preset.next(),
                                channel: 2,
                            });
                        }
                        _ => {}
                    },
                    CreationState::Players => match key_event.code {
                        KeyCode::Enter => {
                            let planet_id = self.planet_ids[self.planet_index];
                            let planet_players = self.planet_players.get(&planet_id).unwrap();
                            let (player_id, _) = planet_players[self.player_index];
                            if self.selected_players.contains(&player_id) {
                                self.selected_players.retain(|&x| x != player_id);
                            } else if self.selected_players.len() < self.max_players_selected() {
                                self.selected_players.push(player_id);
                            }
                            if self.get_remaining_balance() >= 0 && self.enough_players_selected() {
                                self.set_state(self.state.next());
                            }
                        }
                        KeyCode::Backspace => {
                            self.clear_selected_players();
                            self.set_state(self.state.previous());
                        }

                        _ => {}
                    },
                    CreationState::Done => match key_event.code {
                        KeyCode::Enter => {
                            return Some(UiCallback::GeneratePlayerTeam {
                                name: self.team_name_textarea.lines()[0].clone(),
                                home_planet: self.planet_ids[self.planet_index],
                                jersey_style: self.jersey_styles[self.jersey_style_index],
                                jersey_colors: self.get_team_colors(),
                                players: self.selected_players.clone(),
                                spaceship: self.selected_ship().clone(),
                            });
                        }
                        KeyCode::Backspace => {
                            self.set_index(0);
                            return Some(UiCallback::CancelGeneratePlayerTeam);
                        }
                        KeyCode::Left => {
                            self.confirm = ConfirmChoice::Yes;
                        }
                        KeyCode::Right => {
                            self.confirm = ConfirmChoice::No;
                        }
                        _ => {}
                    },
                }
            }
        }

        None
    }
}

impl SplitPanel for NewTeamScreen {
    fn index(&self) -> Option<usize> {
        match self.state {
            CreationState::Planet => Some(self.planet_index),
            CreationState::Jersey => Some(self.jersey_style_index),
            CreationState::ShipModel => Some(self.spaceship_model_index),
            CreationState::Players => Some(self.player_index),
            _ => None,
        }
    }

    fn max_index(&self) -> usize {
        match self.state {
            CreationState::Planet => self.planet_ids.len(),
            CreationState::Jersey => self.jersey_styles.len(),
            CreationState::ShipModel => SPACESHIP_MODELS.len(),
            CreationState::Players => {
                let planet_id = self.planet_ids[self.planet_index];
                if let Some(planet_players) = self.planet_players.get(&planet_id) {
                    planet_players.len()
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn set_index(&mut self, index: usize) {
        match self.state {
            CreationState::Planet => {
                self.planet_index = index;
            }
            CreationState::Jersey => {
                self.jersey_style_index = index;
            }
            CreationState::ShipModel => {
                self.spaceship_model_index = index;
            }
            CreationState::Players => {
                self.player_index = index;
            }
            _ => {}
        }
    }
}
