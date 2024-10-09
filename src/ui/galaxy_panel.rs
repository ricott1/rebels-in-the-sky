use super::button::{Button, RadioButton};
use super::constants::{UiStyle, LEFT_PANEL_WIDTH};
use super::gif_map::{GifMap, ImageResizeInGalaxyGif};
use super::traits::SplitPanel;
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::utils::hover_text_target;
use super::widgets::quick_explore_button;
use super::{traits::Screen, widgets::default_block};
use crate::types::{AppResult, PlayerId, SystemTimeTick, TeamId};
use crate::ui::constants::*;
use crate::world::skill::Rated;
use crate::world::types::PlayerLocation;
use crate::{
    types::{PlanetId, PlanetMap},
    world::{
        constants::*, planet::Planet, types::TeamLocation, utils::ellipse_coords, world::World,
    },
};
use core::fmt::Debug;
use crossterm::event::{KeyCode, KeyEvent};
use itertools::Itertools;
use ratatui::layout::{Constraint, Margin};
use ratatui::widgets::{List, ListItem};
use ratatui::{
    layout::Layout,
    prelude::Rect,
    style::Style,
    text::Span,
    widgets::{Clear, Paragraph},
    Frame,
};
use std::sync::{Arc, Mutex};
use std::{cmp::min, vec};

const TICKS_PER_REVOLUTION: usize = 3;

#[derive(Debug, Default, PartialEq)]
pub enum ZoomLevel {
    #[default]
    Out,
    In,
}

#[derive(Debug, Default)]
pub struct GalaxyPanel {
    pub planet_id: PlanetId,
    pub planets: PlanetMap,
    pub planet_index: usize,
    pub team_index: Option<usize>,
    tick: usize,
    pub zoom_level: ZoomLevel,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    gif_map: Arc<Mutex<GifMap>>,
}

impl GalaxyPanel {
    pub fn new(
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        gif_map: Arc<Mutex<GifMap>>,
    ) -> Self {
        Self {
            planet_id: GALAXY_ROOT_ID.clone(),
            callback_registry,
            gif_map,
            ..Default::default()
        }
    }

    pub fn go_to_planet(
        &mut self,
        planet_id: PlanetId,
        team_index: Option<usize>,
        zoom_level: ZoomLevel,
    ) {
        self.planet_id = planet_id;
        self.planet_index = 0;
        self.zoom_level = zoom_level;
        if let Some(target) = self.planets.get(&self.planet_id) {
            self.team_index = if target.team_ids.len() == 0 {
                None
            } else {
                team_index
            };
        }
    }

    fn render_planet_gif(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let planet = world.get_planet_or_err(self.planet_id)?;
        let mut lines = match self.zoom_level {
            ZoomLevel::In => self.gif_map.lock().unwrap().planet_zoom_in_frame_lines(
                self.planet_id,
                self.tick / planet.rotation_period,
                world,
            ),
            ZoomLevel::Out => self.gif_map.lock().unwrap().planet_zoom_out_frame_lines(
                planet,
                self.tick / TICKS_PER_REVOLUTION,
                world,
            ),
        }?;

        // Apply y-centering
        let min_offset = if lines.len() > area.height as usize {
            (lines.len() - area.height as usize) / 2
        } else {
            0
        };
        let max_offset = min(lines.len(), min_offset + area.height as usize);
        if min_offset > 0 || max_offset < lines.len() {
            lines = lines[min_offset..max_offset].to_vec();
        }

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
        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_planet_buttons(
        &mut self,
        frame: &mut Frame,
        planet: &Planet,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let target = if self.planet_index == 0 {
            planet
        } else {
            world.get_planet_or_err(planet.satellites[self.planet_index - 1])?
        };

        let mut current_id = target.id.clone();
        let mut buttons = vec![];
        while world.get_planet_or_err(current_id)?.satellite_of.is_some() {
            let parent_id = world.get_planet_or_err(current_id)?.satellite_of.unwrap();
            let parent = world.get_planet_or_err(parent_id)?;
            let button = Button::new(
                format!("{}", parent.name).into(),
                UiCallback::GoToPlanetZoomOut {
                    planet_id: parent_id,
                },
                Arc::clone(&self.callback_registry),
            );
            buttons.push(button);
            current_id = parent_id;
        }

        //Order from parent to child
        buttons.reverse();

        let target_button = if target.satellites.len() > 0 {
            Button::new(
                target.name.clone().into(),
                UiCallback::GoToPlanetZoomOut {
                    planet_id: target.id,
                },
                Arc::clone(&self.callback_registry),
            )
        } else if let Some(parent_id) = target.satellite_of {
            Button::new(
                target.name.clone().into(),
                UiCallback::GoToPlanetZoomOut {
                    planet_id: parent_id,
                },
                Arc::clone(&self.callback_registry),
            )
        } else {
            panic!("There should be no planet with no satellites and no parent");
        };
        buttons.push(target_button);

        if self.zoom_level == ZoomLevel::In {
            let own_team = world.get_own_team()?;
            let hover_text_target = hover_text_target(frame);

            match own_team.current_location {
                x if x
                    == TeamLocation::OnPlanet {
                        planet_id: planet.id,
                    } =>
                {
                    if let Ok(explore_button) = quick_explore_button(
                        world,
                        own_team,
                        &self.callback_registry,
                        hover_text_target,
                    ) {
                        buttons.push(explore_button);
                    }
                }
                _ => {
                    let travel_time = world.travel_time_to_planet(own_team.id, planet.id);
                    let (can_travel, button_text, hover_text) = match travel_time {
                        Ok(time) => {
                            let distance_text = match own_team.current_location {
                                TeamLocation::OnPlanet { planet_id } => {
                                    if let Ok(distance) =
                                        world.distance_between_planets(planet_id, planet.id)
                                    {
                                        format!("Distance {:4} AU - ", distance as f32 / AU as f32)
                                    } else {
                                        "".into()
                                    }
                                }
                                _ => "".into(),
                            };

                            (
                                own_team.can_travel_to_planet(&planet, time),
                                time.formatted(),
                                format!(
                                    "Travel to {}: {}Time {} - Fuel {}",
                                    planet.name,
                                    distance_text,
                                    time.formatted(),
                                    (time as f32 * own_team.spaceship_fuel_consumption()) as u32,
                                ),
                            )
                        }
                        Err(e) => {
                            let err_string = e.to_string();
                            (
                                Err(e),
                                "".to_string(),
                                format!("Travel to {}: {}", planet.name, err_string),
                            )
                        }
                    };

                    let mut go_to_planet_button = Button::new(
                        format!("Travel ({})", button_text).into(),
                        UiCallback::TravelToPlanet {
                            planet_id: planet.id,
                        },
                        Arc::clone(&self.callback_registry),
                    )
                    .set_hover_text(hover_text, hover_text_target)
                    .set_hotkey(UiKey::TRAVEL);

                    if can_travel.is_err() {
                        go_to_planet_button.disable(Some(can_travel.unwrap_err().to_string()));
                    }

                    buttons.push(go_to_planet_button);
                }
            }
        }

        let mut constraints = vec![Constraint::Length(3)].repeat(buttons.len());
        constraints.push(Constraint::Length(target.team_ids.len() as u16 + 2));
        constraints.push(Constraint::Min(0));

        let split = Layout::vertical(constraints).split(area);
        for (idx, button) in buttons.iter().enumerate() {
            frame.render_widget(Clear, split[idx]);
            frame.render_widget(button.clone(), split[idx]);
        }

        Ok(())
    }

    fn render_planet_lists(
        &mut self,
        frame: &mut Frame,
        planet: &Planet,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let target = if self.planet_index == 0 {
            planet
        } else {
            world.get_planet_or_err(planet.satellites[self.planet_index - 1])?
        };

        let team_options = target
            .team_ids
            .iter()
            .filter(|&&team_id| world.get_team_or_err(team_id).is_ok())
            .sorted_by(|&&a, &&b| {
                world
                    .team_rating(b)
                    .unwrap_or_default()
                    .partial_cmp(&world.team_rating(a).unwrap_or_default())
                    .expect("Value should be some")
            })
            .map(|&team_id| {
                let team = world
                    .get_team_or_err(team_id)
                    .expect("Team should be part of the world");
                let mut style = UiStyle::DEFAULT;
                if team_id == world.own_team_id {
                    style = UiStyle::OWN_TEAM;
                } else if team.peer_id.is_some() {
                    style = UiStyle::NETWORK;
                }
                let text = format!(
                    "{:<MAX_NAME_LENGTH$} {}",
                    team.name,
                    world.team_rating(team.id).unwrap_or_default().stars()
                );
                (team.id, text, style)
            })
            .take(10)
            .collect::<Vec<(TeamId, String, Style)>>();

        let player_options = world
            .players
            .values()
            .filter(|player| {
                if player.team.is_some() {
                    return false;
                }

                let player_planet_id = match player.current_location {
                    PlayerLocation::OnPlanet { planet_id } => planet_id,
                    _ => panic!("Free pirate must be PlayerLocation::OnPlanet"),
                };

                let team_planet_id = target.id;

                player_planet_id == team_planet_id.clone()
            })
            .sorted_by(|a, b| b.rating().cmp(&a.rating()))
            .map(|player| {
                let text = format!("{:<26} {}", player.info.full_name(), player.stars());
                (player.id, text, UiStyle::DEFAULT)
            })
            .take(10)
            .collect::<Vec<(PlayerId, String, Style)>>();

        let resource_options = target
            .resources
            .iter()
            .sorted_by(|a, b| b.1.cmp(&a.1))
            .map(|(resource, &amount)| {
                let text = format!("{:<7} {}", resource.to_string(), (amount as f32).stars(),);
                (text, UiStyle::DEFAULT)
            })
            .collect::<Vec<(String, Style)>>();

        let team_list_height = if team_options.len() > 0 {
            team_options.len() as u16 + 2
        } else {
            0
        };

        let player_list_height = if player_options.len() > 0 {
            player_options.len() as u16 + 2
        } else {
            0
        };

        let resource_list_height = if resource_options.len() > 0 {
            resource_options.len() as u16 + 2
        } else {
            0
        };

        let split = Layout::vertical([
            Constraint::Length(15),
            Constraint::Length(team_list_height),
            Constraint::Length(player_list_height),
            Constraint::Length(resource_list_height),
            Constraint::Min(0),
        ])
        .split(area);

        if team_options.len() > 0 {
            frame.render_widget(Clear, split[1]);
            let l_split = Layout::vertical([Constraint::Length(1)].repeat(team_options.len()))
                .split(split[1].inner(Margin {
                    horizontal: 2,
                    vertical: 1,
                }));

            for (idx, (team_id, text, style)) in team_options.iter().enumerate() {
                frame.render_widget(
                    Button::no_box(
                        Span::styled(text.clone(), style.clone())
                            .into_left_aligned_line()
                            .into(),
                        UiCallback::GoToTeam {
                            team_id: team_id.clone(),
                        },
                        Arc::clone(&self.callback_registry),
                    )
                    .set_hover_style(UiStyle::HIGHLIGHT),
                    l_split[idx],
                );
            }
            frame.render_widget(default_block().title("Teams "), split[1]);
        }

        if player_options.len() > 0 {
            frame.render_widget(Clear, split[2]);
            let l_split = Layout::vertical([Constraint::Length(1)].repeat(player_options.len()))
                .split(split[2].inner(Margin {
                    horizontal: 2,
                    vertical: 1,
                }));

            for (idx, (player_id, text, style)) in player_options.iter().enumerate() {
                frame.render_widget(
                    Button::no_box(
                        Span::styled(text.clone(), style.clone())
                            .into_left_aligned_line()
                            .into(),
                        UiCallback::GoToPlayer {
                            player_id: player_id.clone(),
                        },
                        Arc::clone(&self.callback_registry),
                    )
                    .set_hover_style(UiStyle::HIGHLIGHT),
                    l_split[idx],
                );
            }
            frame.render_widget(default_block().title("Top free pirates "), split[2]);
        }

        if resource_options.len() > 0 {
            frame.render_widget(Clear, split[3]);
            frame.render_widget(
                List::new(
                    resource_options
                        .iter()
                        .map(|(text, style)| {
                            ListItem::new(Span::styled(format!(" {}", text), *style))
                        })
                        .collect::<Vec<ListItem>>(),
                )
                .block(default_block().title("Resources ")),
                split[3],
            );
        }

        Ok(())
    }

    fn get_planet_info_rect(
        &self,
        central_planet_id: PlanetId,
        index: usize,
        world: &World,
        area: Rect,
    ) -> Rect {
        let central_planet = world.get_planet_or_err(central_planet_id).unwrap();
        match index {
            0 => {
                let size = ImageResizeInGalaxyGif::ZoomOutCentral {
                    planet_type: central_planet.planet_type.clone(),
                }
                .size() as u16;
                let width = min(size + 2, area.width);
                let height = min(size / 2 + 2, area.height);
                let x = if area.width > width {
                    area.x + (area.width - width) / 2
                } else {
                    area.x
                };
                let y = if area.height > height {
                    area.y + (area.height - height) / 2
                } else {
                    area.y
                };

                Rect {
                    x,
                    y,
                    width,
                    height,
                }
            }
            _ => {
                let satellite = world
                    .get_planet_or_err(central_planet.satellites[index - 1])
                    .unwrap();
                let size = ImageResizeInGalaxyGif::ZoomOutSatellite {
                    planet_type: satellite.planet_type.clone(),
                }
                .size() as u16;

                let theta_0 = (index - 1) as f32 * 2.0 * std::f32::consts::PI
                    / central_planet.satellites.len() as f32;
                let theta = theta_0
                    + ((self.tick / TICKS_PER_REVOLUTION) as f32 * 2.0 * std::f32::consts::PI)
                        / satellite.revolution_period as f32;
                let (x_planet, y_planet) = ellipse_coords(satellite.axis, theta);

                let x = (area.width as f32 / 2.0 + x_planet).round() as u16 - 2;
                let y = (area.y as f32 / 2.0 + area.height as f32 / 2.0 + y_planet / 2.0).round()
                    as u16;

                let width = size + 5;
                let height = size / 2 + 3;

                Rect {
                    x,
                    y,
                    width,
                    height,
                }
            }
        }
    }

    fn select_target(&mut self) -> Option<UiCallback> {
        let target = self.planets.get(&self.planet_id)?;

        match self.zoom_level {
            ZoomLevel::In => {
                if self.team_index.is_some() {
                    let team_id = target.team_ids[self.team_index?].clone();
                    return Some(UiCallback::GoToTeam { team_id });
                }
            }
            ZoomLevel::Out => {
                let planet_id = self.planet_id.clone();
                return Some(UiCallback::ZoomInToPlanet { planet_id });
            }
        }
        None
    }
}

impl Screen for GalaxyPanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        if self.planets.len() < world.planets.len() || world.dirty_ui {
            self.planets = world.planets.clone();
        }
        Ok(())
    }
    fn render(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        let planet = world.get_planet_or_err(self.planet_id)?;
        // Ensure that rendering area has even width and odd height for correct rect centering
        let area = Rect {
            x: area.x,
            y: area.y,
            width: area.width - area.width % 2,
            height: area.height - area.height % 2,
        };

        self.render_planet_gif(frame, world, area)?;

        let split =
            Layout::horizontal([Constraint::Max(LEFT_PANEL_WIDTH), Constraint::Min(0)]).split(area);

        self.render_planet_buttons(frame, planet, world, split[0])?;
        self.render_planet_lists(frame, planet, world, split[0])?;

        if self.zoom_level == ZoomLevel::Out {
            let rects = (0..planet.satellites.len() + 1)
                .map(|idx| self.get_planet_info_rect(planet.id.clone(), idx, world, area))
                .collect::<Vec<Rect>>();

            for idx in 0..rects.len() {
                let planet_name = if idx == 0 {
                    planet.name.clone()
                } else {
                    world
                        .get_planet_or_err(planet.satellites[idx - 1])?
                        .name
                        .clone()
                };
                let button = RadioButton::box_on_hover(
                    "".into(),
                    UiCallback::ZoomInToPlanet {
                        planet_id: self.planet_id,
                    },
                    Arc::clone(&self.callback_registry),
                    &mut self.planet_index,
                    idx,
                )
                .set_box_hover_style(UiStyle::NETWORK)
                .set_box_hover_title(planet_name);
                let rect = rects[idx];
                let frame_rect = frame.area();
                if rect.x + rect.width <= frame_rect.width
                    && rect.y + rect.height <= frame_rect.height
                {
                    frame.render_widget(button, rect);
                }
            }
        }
        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, _world: &World) -> Option<UiCallback> {
        let target = self.planets.get(&self.planet_id);
        if target.is_none() {
            return None;
        }
        let target = target.unwrap();
        match key_event.code {
            KeyCode::Up => match self.zoom_level {
                ZoomLevel::Out => {
                    self.planet_index = (self.planet_index + target.satellites.len())
                        % (target.satellites.len() + 1);
                }
                ZoomLevel::In => {
                    if target.team_ids.len() == 0 {
                        self.team_index = None;
                    } else {
                        self.team_index = Some(
                            (self.team_index.unwrap_or_default() + target.team_ids.len() - 1)
                                % target.team_ids.len(),
                        );
                    }
                }
            },
            KeyCode::Down => match self.zoom_level {
                ZoomLevel::Out => {
                    self.planet_index = (self.planet_index + 1) % (target.satellites.len() + 1);
                }
                ZoomLevel::In => {
                    if target.team_ids.len() == 0 {
                        self.team_index = None;
                    } else {
                        self.team_index =
                            Some((self.team_index.unwrap_or_default() + 1) % target.team_ids.len());
                    }
                }
            },

            KeyCode::Enter => {
                return self.select_target();
            }
            KeyCode::Backspace => {
                if self.zoom_level == ZoomLevel::Out || target.satellites.len() == 0 {
                    if let Some(parent) = target.satellite_of.clone() {
                        self.planet_id = parent;
                    }
                }
                self.zoom_level = ZoomLevel::Out;
                self.planet_index = 0;
                self.team_index = None;
            }

            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        match self.zoom_level {
            ZoomLevel::In => vec![" Backspace ".to_string(), " Zoom out ".to_string()],
            ZoomLevel::Out => vec![
                " ↑/↓ ".to_string(),
                " Select ".to_string(),
                " Enter ".to_string(),
                " Zoom in ".to_string(),
                " Backspace ".to_string(),
                " Zoom out ".to_string(),
            ],
        }
    }
}

impl SplitPanel for GalaxyPanel {
    fn index(&self) -> usize {
        if let Some(index) = self.team_index {
            index
        } else {
            0
        }
    }
    fn max_index(&self) -> usize {
        let target = self.planets.get(&self.planet_id);
        if target.is_none() {
            return 0;
        }
        let target = target.unwrap();
        target.team_ids.len()
    }
    fn set_index(&mut self, index: usize) {
        let target = self.planets.get(&self.planet_id);
        if target.is_none() {
            self.team_index = None;
            return;
        }
        let target = target.unwrap();
        if index >= target.team_ids.len() {
            self.team_index = None;
            return;
        }
        self.team_index = Some(index);
    }
}
