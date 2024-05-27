use super::button::{Button, RadioButton};
use super::clickable_list::ClickableListState;
use super::constants::{UiStyle, LEFT_PANEL_WIDTH};
use super::gif_map::{GifMap, ImageResizeInGalaxyGif};
use super::traits::SplitPanel;

use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::utils::hover_text_target;
use super::{
    traits::Screen,
    widgets::{default_block, selectable_list},
};
use crate::types::{AppResult, SystemTimeTick, AU};
use crate::ui::constants::UiKey;
use crate::world::skill::Rated;
use crate::{
    types::{PlanetId, PlanetMap},
    world::{
        constants::*, planet::Planet, types::TeamLocation, utils::ellipse_coords, world::World,
    },
};
use core::fmt::Debug;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Constraint;
use ratatui::widgets::{List, ListItem};
use ratatui::{
    layout::{Alignment, Layout},
    prelude::Rect,
    style::{Color, Style},
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

        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_team_list(
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
                format!("{}", parent.name.clone()),
                UiCallbackPreset::GoToPlanetZoomOut {
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
                target.name.clone(),
                UiCallbackPreset::GoToPlanetZoomOut {
                    planet_id: target.id,
                },
                Arc::clone(&self.callback_registry),
            )
        } else if let Some(parent_id) = target.satellite_of {
            Button::new(
                target.name.clone(),
                UiCallbackPreset::GoToPlanetZoomOut {
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
                    let can_explore = own_team.can_explore_around_planet(&planet);
                    let explore_time = BASE_EXPLORATION_TIME;
                    let mut explore_button = Button::new(
                        format!("Explore ({})", explore_time.formatted()),
                        UiCallbackPreset::ExploreAroundPlanet,
                        Arc::clone(&self.callback_registry),
                    )
                    .set_hover_text(
                        format!(
                            "Explore around {}, who knows what you may find!",
                            planet.name
                        ),
                        hover_text_target,
                    )
                    .set_hotkey(UiKey::EXPLORE);
                    if can_explore.is_err() {
                        explore_button.disable(Some(can_explore.unwrap_err().to_string()));
                    }
                    buttons.push(explore_button);
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
                                    (time as f32 * own_team.spaceship.fuel_consumption()) as u32,
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
                        format!("Travel ({})", button_text),
                        UiCallbackPreset::TravelToPlanet {
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

        let width = (LEFT_PANEL_WIDTH).min(area.width);
        let height = (3 * buttons.len() as u16 + target.team_ids.len() as u16 + 2)
            .max(14)
            .min(area.height);

        let rect = Rect {
            x: area.x,
            y: area.y,
            width,
            height,
        };

        let split = Layout::vertical(constraints).split(rect);

        frame.render_widget(Clear, rect);

        for (idx, button) in buttons.iter().enumerate() {
            frame.render_widget(button.clone(), split[idx]);
        }

        if target.team_ids.len() > 0 {
            let mut options = vec![];
            for &team_id in target.team_ids.iter() {
                if let Some(team) = world.get_team(team_id) {
                    let mut style = UiStyle::DEFAULT;
                    if team_id == world.own_team_id {
                        style = UiStyle::OWN_TEAM;
                    } else if team.peer_id.is_some() {
                        style = UiStyle::NETWORK;
                    }
                    let text = format!("{:<14} {}", team.name, world.team_rating(team.id).stars());
                    options.push((text, style));
                }
            }
            if self.zoom_level == ZoomLevel::In {
                let list = selectable_list(options, &self.callback_registry);

                frame.render_stateful_widget(
                    list.block(default_block().title("Teams")),
                    split[buttons.len()],
                    &mut ClickableListState::default().with_selected(self.team_index),
                );
            } else {
                frame.render_widget(
                    List::new(
                        options
                            .iter()
                            .map(|(text, style)| {
                                ListItem::new(Span::styled(format!(" {}", text), *style))
                            })
                            .collect::<Vec<ListItem>>(),
                    )
                    .block(default_block().title("Teams")),
                    split[buttons.len()],
                );
            }
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

    fn select_target(&mut self) -> Option<UiCallbackPreset> {
        let target = self.planets.get(&self.planet_id)?;

        match self.zoom_level {
            ZoomLevel::In => {
                if self.team_index.is_some() {
                    let team_id = target.team_ids[self.team_index?].clone();
                    return Some(UiCallbackPreset::GoToTeam { team_id });
                }
            }
            ZoomLevel::Out => {
                let planet_id = self.planet_id.clone();
                return Some(UiCallbackPreset::ZoomInToPlanet { planet_id });
            }
        }
        None
    }
}

impl Screen for GalaxyPanel {
    fn name(&self) -> &str {
        "Galaxy"
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        if self.planets.len() < world.planets.len() || world.dirty_ui {
            self.planets = world.planets.clone();
        }
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let planet = world.get_planet_or_err(self.planet_id)?;
        // Ensure that rendering area has even width and odd height for correct rect centering
        let area = Rect {
            x: area.x,
            y: area.y,
            width: area.width - area.width % 2,
            height: area.height - area.height % 2,
        };

        self.render_planet_gif(frame, world, area)?;
        self.render_team_list(frame, planet, world, area)?;

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
                    "".to_string(),
                    UiCallbackPreset::ZoomInToPlanet {
                        planet_id: self.planet_id,
                    },
                    Arc::clone(&self.callback_registry),
                    &mut self.planet_index,
                    idx,
                )
                .set_box_hover_style(UiStyle::NETWORK)
                .set_box_hover_title(planet_name);
                let rect = rects[idx];
                let frame_rect = frame.size();
                if rect.x + rect.width <= frame_rect.width
                    && rect.y + rect.height <= frame_rect.height
                {
                    frame.render_widget(button, rect);
                }
            }
        }
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: KeyEvent,
        _world: &World,
    ) -> Option<UiCallbackPreset> {
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

    fn footer_spans(&self) -> Vec<Span> {
        let spans = vec![
            Span::styled(
                " ↑/↓ ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Select ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " Enter ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Zoom in ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " Backspace ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Zoom out ", Style::default().fg(Color::DarkGray)),
        ];

        spans
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
