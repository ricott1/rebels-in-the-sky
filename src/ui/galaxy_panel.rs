use super::button::Button;
use super::constants::{UiStyle, LEFT_PANEL_WIDTH};
use super::gif_map::{GifMap, ImageResizeInGalaxyGif};
use super::traits::SplitPanel;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::widgets::{space_adventure_button, thick_block};
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
use ratatui::widgets::{block, Borders, List, ListItem};
use ratatui::{
    layout::Layout,
    prelude::Rect,
    style::Style,
    text::Span,
    widgets::{Clear, Paragraph},
};
use std::{cmp::min, vec};

const TICKS_PER_REVOLUTION: usize = 3;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ZoomLevel {
    #[default]
    Out,
    In,
}

#[derive(Debug, Default)]
pub struct GalaxyPanel {
    planet_id: PlanetId,
    planets: PlanetMap,
    planet_index: usize,
    team_index: Option<usize>,
    tick: usize,
    zoom_level: ZoomLevel,
    gif_map: GifMap,
}

impl GalaxyPanel {
    pub fn new() -> Self {
        Self {
            planet_id: GALAXY_ROOT_ID.clone(),
            ..Default::default()
        }
    }

    pub fn set_zoom_level(&mut self, zoom_level: ZoomLevel) {
        self.zoom_level = zoom_level;
    }

    pub fn set_planet_id(&mut self, planet_id: PlanetId) {
        self.planet_id = planet_id;
    }

    pub fn set_planet_index(&mut self, index: usize) {
        self.planet_index = index;
    }

    pub fn set_team_index(&mut self, index: Option<usize>) {
        self.team_index = index;
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

    fn render_planet_gif(
        &mut self,
        frame: &mut UiFrame,
        planet: &Planet,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let mut lines = match self.zoom_level {
            ZoomLevel::In => self.gif_map.planet_zoom_in_frame_lines(
                &self.planet_id,
                self.tick / planet.rotation_period,
                world,
            ),
            ZoomLevel::Out => self.gif_map.planet_zoom_out_frame_lines(
                planet,
                self.tick / TICKS_PER_REVOLUTION,
                world,
            ),
        }?;

        // Apply y-centering
        let min_offset = lines.len().saturating_sub(area.height as usize) / 2;
        let max_offset = min(lines.len(), min_offset + area.height as usize);
        if min_offset > 0 || max_offset < lines.len() {
            lines = lines[min_offset..max_offset].to_vec();
        }

        // Apply x-centering
        if lines[0].spans.len() > area.width as usize - 2 {
            let min_offset = lines[0].spans.len().saturating_sub(area.width as usize) / 2;
            let max_offset = min(lines[0].spans.len(), min_offset + area.width as usize);
            for line in lines.iter_mut() {
                line.spans = line.spans[min_offset..max_offset].to_vec();
            }
        }

        let paragraph = Paragraph::new(lines).centered();
        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_gif_rects(
        &mut self,
        frame: &mut UiFrame,
        planet: &Planet,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        // One rect for each satellite and one rect for the central planet
        for index in 0..planet.satellites.len() + 1 {
            let planet_name = if index == 0 {
                planet.name.clone()
            } else {
                world
                    .get_planet_or_err(&planet.satellites[index - 1])?
                    .name
                    .clone()
            };

            let (mut x, mut y, mut width, mut height) =
                self.get_planet_info_rect(&planet.id, index, world, area);

            // We check that the rect fits into the screen and eventually remove borders that are not
            // visible to avoid distorting it. There is a bit of magic numbering going on here and a lot of
            // trial and errors. For instance, the central block must be handled separately, otherwise
            // some borders disappear when the resoultion gets higher.
            let mut borders = Borders::NONE;
            let mut title_position = block::Position::Top;

            if x >= 0.0 {
                borders |= Borders::LEFT;
            } else {
                width = width.saturating_sub(x.abs() as u16);
                x = 0.0;
            }

            if (x + width as f32) <= area.width as f32 {
                borders |= Borders::RIGHT;
            } else {
                width = (area.width).saturating_sub(x.abs() as u16);
            }

            if y >= 0.0 {
                borders |= Borders::TOP;
            } else {
                title_position = block::Position::Bottom;
                height = height.saturating_sub(y.abs() as u16);
                y = 0.0;
            }

            if (y + height as f32) < (area.height + 4) as f32 {
                borders |= Borders::BOTTOM;
            } else {
                height = (area.height + 4).saturating_sub(y.abs() as u16);
            }

            let block = thick_block()
                .border_style(UiStyle::NETWORK)
                .title(planet_name)
                .borders(borders)
                .title_position(title_position);

            let rect = frame.to_screen_rect(Rect::new(x as u16, y as u16, width, height));

            if frame.is_hovering(rect) {
                self.planet_index = index;
            }

            let target_id = if self.planet_index == 0 {
                planet.id
            } else {
                planet.satellites[self.planet_index - 1]
            };

            let target = world.get_planet_or_err(&target_id)?;
            let zoom_level = if self.planet_index == 0 || target.satellites.len() == 0 {
                ZoomLevel::In
            } else {
                ZoomLevel::Out
            };

            let button = if index == self.planet_index {
                Button::new(
                    "",
                    UiCallback::ZoomToPlanet {
                        planet_id: target_id,
                        zoom_level,
                    },
                )
                .block(block.clone())
                .hover_block(block)
                .set_hover_style(UiStyle::DEFAULT)
            } else {
                Button::box_on_hover(
                    "",
                    UiCallback::ZoomToPlanet {
                        planet_id: target_id,
                        zoom_level,
                    },
                )
                .hover_block(block)
                .set_hover_style(UiStyle::DEFAULT)
            };

            frame.render_interactive(button, rect);
        }

        Ok(())
    }

    fn render_planet_buttons(
        &mut self,
        frame: &mut UiFrame,
        planet: &Planet,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let target = if self.planet_index == 0 {
            planet
        } else {
            world.get_planet_or_err(&planet.satellites[self.planet_index - 1])?
        };

        let mut current_id = target.id;
        let mut buttons = vec![];
        while world.get_planet_or_err(&current_id)?.satellite_of.is_some() {
            let parent_id = world.get_planet_or_err(&current_id)?.satellite_of.unwrap();
            let parent = world.get_planet_or_err(&parent_id)?;
            let button = Button::new(
                format!("{}", parent.name),
                UiCallback::GoToPlanetZoomOut {
                    planet_id: parent_id,
                },
            );
            buttons.push(button);
            current_id = parent_id;
        }

        //Order from parent to child
        buttons.reverse();

        let target_button = if target.satellites.len() > 0 {
            Button::new(
                target.name.clone(),
                UiCallback::GoToPlanetZoomOut {
                    planet_id: target.id,
                },
            )
        } else if let Some(parent_id) = target.satellite_of {
            Button::new(
                target.name.clone(),
                UiCallback::GoToPlanetZoomOut {
                    planet_id: parent_id,
                },
            )
        } else {
            unreachable!("There should be no planet with no satellites and no parent");
        };
        buttons.push(target_button.selected());

        if self.zoom_level == ZoomLevel::In {
            let own_team = world.get_own_team()?;
            match own_team.current_location {
                x if x
                    == TeamLocation::OnPlanet {
                        planet_id: planet.id,
                    } =>
                {
                    if let Ok(explore_button) = space_adventure_button(world, own_team) {
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
                                        "".to_string()
                                    }
                                }
                                _ => "".to_string(),
                            };

                            (
                                own_team.can_travel_to_planet(&planet, time),
                                format!(" ({})", time.formatted()),
                                format!(
                                    "Travel to {}: {}Time {} - Fuel {}",
                                    planet.name,
                                    distance_text,
                                    time.formatted(),
                                    (time as f32 * own_team.spaceship_fuel_consumption()) as u32,
                                ),
                            )
                        }
                        Err(e) => (Err(e), "".to_string(), format!("Travel to {}", planet.name)),
                    };

                    let mut travel_to_planet_button = Button::new(
                        format!("Travel{}", button_text),
                        UiCallback::TravelToPlanet {
                            planet_id: planet.id,
                        },
                    )
                    .set_hover_text(hover_text)
                    .set_hotkey(UiKey::TRAVEL);

                    if can_travel.is_err() {
                        travel_to_planet_button.disable(Some(can_travel.unwrap_err().to_string()));
                    }

                    buttons.push(travel_to_planet_button);
                }
            }
        }

        let mut constraints = vec![Constraint::Length(3)].repeat(buttons.len());
        constraints.push(Constraint::Length(target.team_ids.len() as u16 + 2));
        constraints.push(Constraint::Min(0));

        let split = Layout::vertical(constraints).split(area);
        for (idx, button) in buttons.iter().enumerate() {
            frame.render_widget(Clear, split[idx]);
            frame.render_interactive(button.clone(), split[idx]);
        }

        Ok(())
    }

    fn render_planet_lists(
        &mut self,
        frame: &mut UiFrame,
        planet: &Planet,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let target = if self.planet_index == 0 {
            planet
        } else {
            world.get_planet_or_err(&planet.satellites[self.planet_index - 1])?
        };

        let mut team_options = target
            .team_ids
            .iter()
            .filter(|&team_id| world.get_team_or_err(team_id).is_ok())
            .sorted_by(|&a, &b| {
                world
                    .team_rating(b)
                    .unwrap_or_default()
                    .partial_cmp(&world.team_rating(a).unwrap_or_default())
                    .expect("Value should be some")
            })
            .map(|team_id| {
                let team = world
                    .get_team_or_err(team_id)
                    .expect("Team should be part of the world");
                let mut style = UiStyle::DEFAULT;
                if *team_id == world.own_team_id {
                    style = UiStyle::OWN_TEAM;
                } else if team.peer_id.is_some() {
                    style = UiStyle::NETWORK;
                }
                let text = format!(
                    "{:<MAX_NAME_LENGTH$} {}",
                    team.name,
                    world.team_rating(team_id).unwrap_or_default().stars()
                );
                (team.id, text, style)
            })
            .take(10)
            .collect::<Vec<(TeamId, String, Style)>>();

        if let Some(team_index) = self.team_index {
            if team_options.len() > team_index {
                team_options[team_index].2 = UiStyle::SELECTED;
            }
        }

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
                frame.render_interactive(
                    Button::no_box(
                        Span::styled(text.clone(), style.clone()).into_left_aligned_line(),
                        UiCallback::GoToTeam {
                            team_id: team_id.clone(),
                        },
                    ),
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
                frame.render_interactive(
                    Button::no_box(
                        Span::styled(text.clone(), style.clone()).into_left_aligned_line(),
                        UiCallback::GoToPlayer {
                            player_id: player_id.clone(),
                        },
                    ),
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
        central_planet_id: &PlanetId,
        index: usize,
        world: &World,
        area: Rect,
    ) -> (f32, f32, u16, u16) {
        let central_planet = world.get_planet_or_err(central_planet_id).unwrap();
        match index {
            0 => {
                let size = ImageResizeInGalaxyGif::ZoomOutCentral {
                    planet_type: central_planet.planet_type,
                }
                .size() as u16;

                let width = area.width.min(size + 2);
                let height = area.height.min(size / 2 + 2);
                let x = area.width.saturating_sub(width) / 2;
                let y = area.height.saturating_sub(height) / 2 + 4;

                (x as f32, y as f32, width, height)
            }
            _ => {
                let satellite = world
                    .get_planet_or_err(&central_planet.satellites[index - 1])
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

                let x = (area.width as f32 / 2.0 + x_planet).round() - 2.0;
                let y = (area.height as f32 / 2.0 + y_planet / 2.0).round() + 2.0;

                let width = size + 5;
                let height = size / 2 + 3;

                (x, y, width, height)
            }
        }
    }

    fn select_target(&mut self, target: &Planet) -> Option<UiCallback> {
        match self.zoom_level {
            ZoomLevel::In => {
                if self.team_index.is_some() {
                    let team_id = target.team_ids[self.team_index?].clone();
                    return Some(UiCallback::GoToTeam { team_id });
                }
            }
            ZoomLevel::Out => {
                let zoom_level = if self.planet_index == 0 || target.satellites.len() == 0 {
                    ZoomLevel::In
                } else {
                    ZoomLevel::Out
                };
                return Some(UiCallback::ZoomToPlanet {
                    planet_id: target.id,
                    zoom_level,
                });
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
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        let planet = world.get_planet_or_err(&self.planet_id)?;
        // Ensure that rendering area has even width and height for correct rect centering
        let render_area = area; //Rect::new(
                                //     area.x,
                                //     area.y,
                                //     area.width - area.width % 2,
                                //     area.height - area.height % 2,
                                // );

        self.render_planet_gif(frame, planet, world, render_area)?;
        if self.zoom_level == ZoomLevel::Out {
            self.render_gif_rects(frame, planet, world, render_area)?;
        }

        let split =
            Layout::horizontal([Constraint::Max(LEFT_PANEL_WIDTH), Constraint::Min(0)]).split(area);

        self.render_planet_buttons(frame, planet, world, split[0])?;
        self.render_planet_lists(frame, planet, world, split[0])?;

        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, world: &World) -> Option<UiCallback> {
        let planet = if let Some(planet) = self.planets.get(&self.planet_id) {
            planet
        } else {
            return None;
        };

        match key_event.code {
            KeyCode::Up => match self.zoom_level {
                ZoomLevel::Out => {
                    self.planet_index = (self.planet_index + planet.satellites.len())
                        % (planet.satellites.len() + 1);
                }
                ZoomLevel::In => {
                    if planet.team_ids.len() == 0 {
                        self.team_index = None;
                    } else {
                        self.team_index = Some(
                            (self.team_index.unwrap_or_default() + planet.team_ids.len() - 1)
                                % planet.team_ids.len(),
                        );
                    }
                }
            },
            KeyCode::Down => match self.zoom_level {
                ZoomLevel::Out => {
                    self.planet_index = (self.planet_index + 1) % (planet.satellites.len() + 1);
                }
                ZoomLevel::In => {
                    if planet.team_ids.len() == 0 {
                        self.team_index = None;
                    } else {
                        self.team_index =
                            Some((self.team_index.unwrap_or_default() + 1) % planet.team_ids.len());
                    }
                }
            },

            KeyCode::Enter => {
                let target_id = if self.planet_index == 0 {
                    planet.id
                } else {
                    planet.satellites[self.planet_index - 1]
                };

                if let Ok(target) = world.get_planet_or_err(&target_id) {
                    return self.select_target(target);
                }
            }
            KeyCode::Backspace => {
                if self.zoom_level == ZoomLevel::Out || planet.satellites.len() == 0 {
                    if let Some(parent) = planet.satellite_of.clone() {
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
