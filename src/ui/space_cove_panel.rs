use super::ui_frame::UiFrame;
use super::{traits::Screen, ui_callback::UiCallback};
use crate::image::utils::ExtraImageUtils;
use crate::image::utils::{open_image, LightMaskStyle};
use crate::types::{HashMapWithResult, SystemTimeTick, TeamId, Tick};
use crate::ui::button::Button;
use crate::ui::clickable_list::ClickableListState;
use crate::ui::traits::SplitPanel;
use crate::ui::utils::img_to_lines;
use crate::ui::widgets::{default_block, selectable_list, teleport_button};
use crate::ui::{constants::*, ui_key};
use crate::{core::*, types::AppResult};
use anyhow::anyhow;
use core::fmt::Debug;
use image::RgbaImage;
use itertools::Itertools;
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::prelude::Rect;
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct SpaceCovePanel {
    tick: usize,
    teams_index: Option<usize>,
    team_ids: Vec<TeamId>,
    cove_image_widgets: [Paragraph<'static>; 4], // no blinking, left, right, both
}

impl SpaceCovePanel {
    pub fn new() -> Self {
        let cove_image_widget = Self::get_cove_image_widgets(&vec![], false, false)
            .expect("Should be able to create cove image");
        let cove_image_widget_blinking_left = Self::get_cove_image_widgets(&vec![], true, false)
            .expect("Should be able to create cove image");
        let cove_image_widget_blinking_right = Self::get_cove_image_widgets(&vec![], false, true)
            .expect("Should be able to create cove image");
        let cove_image_widget_blinking_both = Self::get_cove_image_widgets(&vec![], true, true)
            .expect("Should be able to create cove image");

        Self {
            cove_image_widgets: [
                cove_image_widget,
                cove_image_widget_blinking_left,
                cove_image_widget_blinking_right,
                cove_image_widget_blinking_both,
            ],
            ..Default::default()
        }
    }

    fn get_asteroid(world: &World) -> AppResult<&Planet> {
        let own_team = world.get_own_team()?;
        let asteroid_id = match own_team.space_cove {
            SpaceCoveState::Ready { planet_id } => planet_id,
            state => {
                return Err(anyhow!(
                    "Space cove panel should not exist for space cove state {state}."
                ))
            }
        };
        world.planets.get_or_err(&asteroid_id)
    }

    fn get_cove_images(
        teams: &Vec<&Team>,
        is_blinking_left: bool,
        is_blinking_right: bool,
    ) -> AppResult<RgbaImage> {
        let mut base = open_image("cove/base.png").expect("Cove image base.png should exist");

        const SKULL_POSITION: (u32, u32) = (98, 1);
        const LEFT_EYE_POSITION: (u32, u32) = (SKULL_POSITION.0 + 4, SKULL_POSITION.1 + 11);
        const RIGHT_EYE_POSITION: (u32, u32) = (SKULL_POSITION.0 + 13, SKULL_POSITION.1 + 11);

        let skull = open_image("cove/skull.png").expect("Cove image skull.png should exist");
        base.copy_non_trasparent_from(&skull, SKULL_POSITION.0, SKULL_POSITION.1)?;

        if is_blinking_left {
            let left_eye = open_image("cove/left_eye_mask.png")
                .expect("Cove image left_eye_mask.png should exist");
            base.copy_non_trasparent_from(&left_eye, LEFT_EYE_POSITION.0, LEFT_EYE_POSITION.1)?;
        }

        if is_blinking_right {
            let right_eye = open_image("cove/right_eye_mask.png")
                .expect("Cove image right_eye_mask.png should exist");
            base.copy_non_trasparent_from(&right_eye, RIGHT_EYE_POSITION.0, RIGHT_EYE_POSITION.1)?;
        }

        let mut x = 5;
        for team in teams.iter().take(4) {
            let ship_img = &team.spaceship.compose_image_in_shipyard()?[0];
            let y = 40;
            base.copy_non_trasparent_from(ship_img, x, y)?;
            x += ship_img.width();
            if x + ship_img.width() > base.width() {
                break;
            }
        }

        if !is_blinking_left {
            base.apply_light_mask(&LightMaskStyle::skull_eye((
                LEFT_EYE_POSITION.0 + 2,
                LEFT_EYE_POSITION.1 + 2,
            )));
        }

        if !is_blinking_right {
            base.apply_light_mask(&LightMaskStyle::skull_eye((
                RIGHT_EYE_POSITION.0 + 2,
                RIGHT_EYE_POSITION.1 + 2,
            )));
        }

        let outer =
            open_image("cove/base_outer.png").expect("Cove image base_outer.png should exist");
        base.copy_non_trasparent_from(&outer, 0, 0)?;
        Ok(base)
    }

    fn get_cove_image_widgets<'a>(
        teams: &Vec<&Team>,
        is_blinking_left: bool,
        is_blinking_right: bool,
    ) -> AppResult<Paragraph<'a>> {
        let img = Self::get_cove_images(teams, is_blinking_left, is_blinking_right)?;
        let cove_image_lines = img_to_lines(&img);
        Ok(Paragraph::new(cove_image_lines))
    }

    fn render_visiting_teams(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if !self.team_ids.is_empty() {
            let mut options = vec![];
            for team_id in self.team_ids.iter() {
                let team = if let Some(team) = world.teams.get(team_id) {
                    team
                } else {
                    continue;
                };
                let mut style = UiStyle::DEFAULT;
                if team.id == world.own_team_id {
                    style = UiStyle::OWN_TEAM;
                } else if team.peer_id.is_some() {
                    style = UiStyle::NETWORK;
                }
                let text = format!(
                    "{:<MAX_NAME_LENGTH$} {}",
                    team.name,
                    world.team_rating(&team.id).unwrap_or_default().stars()
                );
                options.push((text, style));
            }
            let list = selectable_list(options);

            frame.render_stateful_interactive_widget(
                list.block(default_block().title("Teams ↓/↑")),
                area,
                &mut ClickableListState::default().with_selected(self.teams_index),
            );
        } else {
            frame.render_widget(default_block().title("No visiting teams"), area);
        }

        Ok(())
    }

    fn render_tournament_buttons(
        &self,
        frame: &mut UiFrame,
        asteroid: &Planet,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::vertical([3, 3]).split(area);
        let own_team = world.get_own_team()?;
        let mut tournament_button = Button::new(
            "Organize quick tournament",
            UiCallback::OrganizeNewTournament {
                max_participants: 4,
                registrations_closing_at: Tick::now() + 5 * MINUTES,
            },
        )
        .set_hotkey(ui_key::ORGANIZE_QUICK_TOURNAMENT)
        .set_hover_text(format!("Organize a quick tournament on {}. Registrations close in 5 minutes, max 4 participants.", asteroid.name));

        if let Err(err) = own_team.can_organize_tournament() {
            tournament_button.disable(Some(err.to_string()));
        }

        frame.render_interactive_widget(tournament_button, split[0]);

        let mut tournament_button = Button::new(
            "Organize big tournament",
            UiCallback::OrganizeNewTournament {
                max_participants: 8,
                registrations_closing_at: Tick::now() + HOURS,
            },
        )
        .set_hotkey(ui_key::ORGANIZE_BIG_TOURNAMENT)
        .set_hover_text(format!(
            "Organize a big tournament on {}. Registrations close in 1 hour, max 8 participants.",
            asteroid.name
        ));

        if let Err(err) = own_team.can_organize_tournament() {
            tournament_button.disable(Some(err.to_string()));
        }

        frame.render_interactive_widget(tournament_button, split[1]);

        Ok(())
    }
}

impl Screen for SpaceCovePanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;

        let asteroid = Self::get_asteroid(world)?;
        if world.dirty_ui || self.team_ids.len() != asteroid.team_ids.len() {
            // Update team_ids but maintain team_ids existing order.
            let new_set: HashSet<TeamId> = asteroid.team_ids.iter().copied().collect();
            self.team_ids.retain(|id| new_set.contains(id));
            for id in &asteroid.team_ids {
                if !self.team_ids.contains(id) {
                    self.team_ids.push(*id);
                }
            }

            let teams = self
                .team_ids
                .iter()
                .take(4)
                .filter(|id| world.teams.contains_key(id))
                .map(|id| world.teams.get(id).unwrap())
                .collect_vec();

            let cove_image_widget = Self::get_cove_image_widgets(&teams, false, false)
                .expect("Should be able to create cove image");
            let cove_image_widget_blinking_left = Self::get_cove_image_widgets(&teams, true, false)
                .expect("Should be able to create cove image");
            let cove_image_widget_blinking_right =
                Self::get_cove_image_widgets(&teams, false, true)
                    .expect("Should be able to create cove image");
            let cove_image_widget_blinking_both = Self::get_cove_image_widgets(&teams, true, true)
                .expect("Should be able to create cove image");
            self.cove_image_widgets = [
                cove_image_widget,
                cove_image_widget_blinking_left,
                cove_image_widget_blinking_right,
                cove_image_widget_blinking_both,
            ];
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
        let asteroid = Self::get_asteroid(world)?;

        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
            .split(area);

        frame.render_widget(default_block(), split[1]);

        let t = self.tick % 60;
        let left_eye_blinking = [2, 3, 5, 13, 33].contains(&t);
        let right_eye_blinking = [2, 3, 6, 7, 41].contains(&t);

        let widget = if !left_eye_blinking && !right_eye_blinking {
            &self.cove_image_widgets[0]
        } else if left_eye_blinking && !right_eye_blinking {
            &self.cove_image_widgets[1]
        } else if !left_eye_blinking && right_eye_blinking {
            &self.cove_image_widgets[2]
        } else {
            //if left_eye_blinking && right_eye_blinking
            &self.cove_image_widgets[3]
        };

        let area_image = split[1].inner(Margin::new(1, 1));

        frame.render_widget(widget, area_image);

        let side_split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Fill(1),
        ])
        .split(split[0]);
        frame.render_widget(
            Paragraph::new(format!("Space Cove on {}", asteroid.name))
                .centered()
                .bold()
                .block(default_block()),
            side_split[0],
        );

        frame.render_interactive_widget(teleport_button(world, asteroid.id)?, side_split[1]);
        self.render_tournament_buttons(frame, asteroid, world, side_split[2])?;

        self.render_visiting_teams(frame, world, side_split[3])?;

        frame.render_widget(
            default_block().title("No available upgrades"),
            side_split[4],
        );

        Ok(())
    }

    fn handle_key_events(
        &mut self,
        _key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallback> {
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![]
    }
}

impl SplitPanel for SpaceCovePanel {
    fn index(&self) -> Option<usize> {
        self.teams_index
    }

    fn max_index(&self) -> usize {
        self.team_ids.len()
    }

    fn set_index(&mut self, index: usize) {
        self.teams_index = Some(index % self.max_index());
    }
}
