use super::ui_frame::UiFrame;
use super::{traits::Screen, ui_callback::UiCallback};
use crate::image::utils::open_image;
use crate::image::utils::ExtraImageUtils;
use crate::types::TeamId;
use crate::ui::clickable_list::ClickableListState;
use crate::ui::constants::*;
use crate::ui::traits::SplitPanel;
use crate::ui::utils::img_to_lines;
use crate::ui::widgets::{default_block, selectable_list, teleport_button};
use crate::{types::AppResult, world::*};
use anyhow::anyhow;
use core::fmt::Debug;
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::prelude::Rect;
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;

#[derive(Debug, Default)]
pub struct SpaceCovePanel {
    tick: usize,
    teams_index: Option<usize>,
    team_ids: Vec<TeamId>,
    cove_image_widget: Paragraph<'static>,
    cove_image_widget_size: u16,
}

impl SpaceCovePanel {
    pub fn new() -> Self {
        let image = open_image("cove/base.png").expect("Cove image base.png should exist");
        let cove_image_widget_size = image.width() as u16 + 2;
        let cove_image_lines = img_to_lines(&image);
        let cove_image_widget = Paragraph::new(cove_image_lines);
        Self {
            cove_image_widget,
            cove_image_widget_size,
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
        world.get_planet_or_err(&asteroid_id)
    }

    fn get_cove_image_widget<'a>(&self, world: &World) -> AppResult<Paragraph<'a>> {
        let mut base = open_image("cove/base.png").expect("Cove image base.png should exist");

        let mut x = 7;
        for team_id in self.team_ids.iter().take(4) {
            let team = if let Some(team) = world.get_team(team_id) {
                team
            } else {
                continue;
            };

            let ship_img = &team.spaceship.compose_image_in_shipyard()?[0];
            let y = 40;
            base.copy_non_trasparent_from(ship_img, x, y)?;
            x += ship_img.width() + 2;
        }

        let outer =
            open_image("cove/base_outer.png").expect("Cove image base_outer.png should exist");
        base.copy_non_trasparent_from(&outer, 0, 0)?;
        let cove_image_lines = img_to_lines(&base);
        Ok(Paragraph::new(cove_image_lines))
    }

    fn render_visiting_teams(
        &self,
        frame: &mut UiFrame,
        asteroid: &Planet,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if !asteroid.team_ids.is_empty() {
            let mut options = vec![];
            for team_id in asteroid.team_ids.iter() {
                let team = if let Some(team) = world.get_team(team_id) {
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
}

impl Screen for SpaceCovePanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;

        let asteroid = Self::get_asteroid(world)?;
        if world.dirty_ui || self.team_ids.len() != asteroid.team_ids.len() {
            self.team_ids = asteroid.team_ids.clone();
            self.cove_image_widget = self.get_cove_image_widget(world)?;
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

        let split = Layout::horizontal([
            Constraint::Length(self.cove_image_widget_size),
            Constraint::Fill(1),
        ])
        .split(area);

        frame.render_widget(default_block(), split[0]);

        frame.render_widget(&self.cove_image_widget, split[0].inner(Margin::new(1, 1)));

        let right_split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Fill(1),
        ])
        .split(split[1]);
        frame.render_widget(
            Paragraph::new(format!("Space Cove on {}", asteroid.name))
                .centered()
                .bold()
                .block(default_block()),
            right_split[0],
        );

        frame.render_interactive_widget(teleport_button(world, asteroid.id)?, right_split[1]);

        self.render_visiting_teams(frame, asteroid, world, right_split[2])?;

        frame.render_widget(
            default_block().title("No available upgrades"),
            right_split[3],
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
