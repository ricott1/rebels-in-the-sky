use super::ui_frame::UiFrame;
use super::{traits::Screen, ui_callback::UiCallback};
use crate::game_engine::TournamentType;
use crate::image::utils::ExtraImageUtils;
use crate::image::utils::{open_image, LightMaskStyle};
use crate::types::{PlanetId, TeamId};
use crate::ui::button::Button;
use crate::ui::clickable_list::ClickableListState;
use crate::ui::traits::SplitPanel;
use crate::ui::ui_screen::{render_help_block, UiTab};
use crate::ui::utils::img_to_lines;
use crate::ui::widgets::{default_block, go_to_planet_button, selectable_list};
use crate::ui::{constants::*, ui_key};
use crate::{core::*, types::AppResult};
use core::fmt::Debug;
use image::RgbaImage;
use itertools::Itertools;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::prelude::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use std::collections::HashSet;
use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SpaceCoveView {
    #[default]
    OwnCove,
    AllCoves,
}

impl SpaceCoveView {
    pub const fn next(&self) -> Self {
        match self {
            Self::OwnCove => Self::AllCoves,
            Self::AllCoves => Self::OwnCove,
        }
    }

    pub const fn previous(&self) -> Self {
        match self {
            Self::OwnCove => Self::AllCoves,
            Self::AllCoves => Self::OwnCove,
        }
    }
}

impl Display for SpaceCoveView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnCove => write!(f, "Space cove"),
            Self::AllCoves => write!(f, "All coves"),
        }
    }
}

#[derive(Debug, Default)]
pub struct SpaceCovePanel {
    tick: usize,
    view: SpaceCoveView,
    cove_index: Option<usize>,
    cove_entries: Vec<(TeamId, PlanetId)>,
    cached_teams_len: usize,
    visiting_team_ids: Vec<TeamId>,
    cove_image_widgets: [Paragraph<'static>; 4], // no blinking, left, right, both
}

impl SpaceCovePanel {
    pub fn new() -> Self {
        let widgets = Self::build_image_widgets(&[]).expect("Should be able to create cove image");
        Self {
            cove_image_widgets: widgets,
            ..Default::default()
        }
    }

    pub fn set_view(&mut self, view: SpaceCoveView) {
        self.view = view;
    }

    fn get_cove_images(
        teams: &[&Team],
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
        teams: &[&Team],
        is_blinking_left: bool,
        is_blinking_right: bool,
    ) -> AppResult<Paragraph<'a>> {
        let img = Self::get_cove_images(teams, is_blinking_left, is_blinking_right)?;
        let cove_image_lines = img_to_lines(&img);
        Ok(Paragraph::new(cove_image_lines))
    }

    fn build_image_widgets(teams: &[&Team]) -> AppResult<[Paragraph<'static>; 4]> {
        Ok([
            Self::get_cove_image_widgets(teams, false, false)?,
            Self::get_cove_image_widgets(teams, true, false)?,
            Self::get_cove_image_widgets(teams, false, true)?,
            Self::get_cove_image_widgets(teams, true, true)?,
        ])
    }

    fn render_view_buttons(
        &self,
        frame: &mut UiFrame,
        world: &World,
        own_cove_area: Rect,
        other_coves_area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let own_cove_label = match own_team.has_space_cove_on() {
            Some(cove_planet) => {
                let asteroid_name = world
                    .planets
                    .get(&cove_planet)
                    .map(|p| p.name.as_str())
                    .unwrap_or("???");
                format!("Space cove on {}", asteroid_name)
            }
            None => "No space cove".to_string(),
        };

        let mut own_button = Button::new(
            own_cove_label,
            UiCallback::SetSpaceCovePanelView {
                view: SpaceCoveView::OwnCove,
            },
        )
        .bold()
        .set_hover_text("Manage your own space cove.");
        if own_team.space_cove.is_none() {
            own_button.disable(Some("You don't have a space cove yet".to_string()));
        }

        let mut other_button = Button::new(
            SpaceCoveView::AllCoves.to_string(),
            UiCallback::SetSpaceCovePanelView {
                view: SpaceCoveView::AllCoves,
            },
        )
        .bold()
        .set_hover_text("Browse coves owned by other crews.");

        match self.view {
            SpaceCoveView::OwnCove => own_button.select(),
            SpaceCoveView::AllCoves => other_button.select(),
        }

        frame.render_interactive_widget(own_button, own_cove_area);
        frame.render_interactive_widget(other_button, other_coves_area);
        Ok(())
    }

    fn render_cove_list(&self, frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        if self.cove_entries.is_empty() {
            frame.render_widget(default_block().title("No known coves"), area);
            return Ok(());
        }

        let mut options = vec![];
        for (team_id, asteroid_id) in self.cove_entries.iter() {
            let team = match world.teams.get(team_id) {
                Some(t) => t,
                None => continue,
            };
            let style = if team.id == world.own_team_id {
                UiStyle::OWN_TEAM
            } else if team.peer_id.is_some() {
                UiStyle::NETWORK
            } else {
                UiStyle::DEFAULT
            };
            let asteroid_name = world
                .planets
                .get(asteroid_id)
                .map(|p| p.name.as_str())
                .unwrap_or("???");
            // Pad team-name + parenthesised asteroid so stars align across rows.
            let label = format!("{} ({})", team.name, asteroid_name);
            let text = format!(
                "{:<width$} {}",
                label,
                world.team_rating(&team.id).unwrap_or_default().stars(),
                width = MAX_NAME_LENGTH * 2,
            );
            options.push((text, style));
        }

        let list = selectable_list(options);
        frame.render_stateful_interactive_widget(
            list.block(default_block().title("Coves ↓/↑")),
            area,
            &mut ClickableListState::default().with_selected(self.cove_index),
        );
        Ok(())
    }

    fn render_visiting_teams(
        &self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        if !self.visiting_team_ids.is_empty() {
            let mut options = vec![];
            for team_id in self.visiting_team_ids.iter() {
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
                list.block(default_block().title("Visiting teams")),
                area,
                &mut ClickableListState::default(),
            );
        } else {
            frame.render_widget(default_block().title("No visiting teams"), area);
        }

        Ok(())
    }

    fn render_tournament_button(
        &self,
        frame: &mut UiFrame,
        own_team: &Team,
        asteroid: Option<&Planet>,
        tournament_type: TournamentType,
        area: Rect,
    ) {
        let (label, hotkey, blurb) = match tournament_type {
            TournamentType::Cup => (
                "Organize quick tournament",
                ui_key::ORGANIZE_QUICK_TOURNAMENT,
                "Registrations close in 5 minutes, max 4 participants.",
            ),
            TournamentType::Supercup => (
                "Organize big tournament",
                ui_key::ORGANIZE_BIG_TOURNAMENT,
                "Registrations close in 1 hour, max 8 participants.",
            ),
        };

        let hover = match asteroid {
            Some(asteroid) => format!("Organize on {}. {blurb}", asteroid.name),
            None => blurb.to_string(),
        };

        let mut button = Button::new(label, UiCallback::OrganizeNewTournament { tournament_type })
            .set_hotkey(hotkey)
            .set_hover_text(hover);

        if let Err(err) = own_team.can_organize_tournament() {
            button.disable(Some(err.to_string()));
        }

        frame.render_interactive_widget(button, area);
    }

    fn render_go_to_planet_slot(
        &self,
        frame: &mut UiFrame,
        world: &World,
        asteroid: Option<&Planet>,
        area: Rect,
    ) -> AppResult<()> {
        let asteroid = match asteroid {
            Some(a) => a,
            None => {
                let mut button = Button::new("Go to planet", UiCallback::None)
                    .set_hover_text("Select a cove first");
                button.disable(Some("No cove selected".to_string()));
                frame.render_interactive_widget(button, area);
                return Ok(());
            }
        };

        frame.render_interactive_widget(go_to_planet_button(world, asteroid.id)?, area);
        Ok(())
    }

    fn render_raid_placeholder(&self, frame: &mut UiFrame, area: Rect) {
        let mut button =
            Button::new("Raid", UiCallback::None).set_hover_text("Raids are coming soon");
        button.disable(Some("Coming soon".to_string()));
        frame.render_interactive_widget(button, area);
    }
}

impl Screen for SpaceCovePanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;

        let own_team_id = world.own_team_id;

        // Rebuild the cove entries only when the team set or contents may have shifted.
        let mut entries_changed = false;
        if world.dirty_ui || world.teams.len() != self.cached_teams_len {
            let mut decorated: Vec<(TeamId, PlanetId, &str)> = world
                .teams
                .values()
                .filter_map(|team| {
                    team.space_cove
                        .as_ref()
                        .filter(|cove| cove.state == SpaceCoveState::Ready)
                        .map(|cove| (team.id, cove.planet_id, team.name.as_str()))
                })
                .collect();

            decorated.sort_by(|a, b| {
                let a_own = a.0 == own_team_id;
                let b_own = b.0 == own_team_id;
                match (a_own, b_own) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.2.cmp(b.2),
                }
            });

            let new_entries: Vec<(TeamId, PlanetId)> =
                decorated.into_iter().map(|(t, p, _)| (t, p)).collect();

            entries_changed = new_entries != self.cove_entries;
            self.cove_entries = new_entries;
            self.cached_teams_len = world.teams.len();
        }

        let prev_index = self.cove_index;
        self.cove_index = if self.cove_entries.is_empty() {
            None
        } else {
            Some(
                self.cove_index
                    .unwrap_or(0)
                    .min(self.cove_entries.len() - 1),
            )
        };
        let index_changed = prev_index != self.cove_index;

        let own_team = world.get_own_team()?;
        let selected_asteroid_id = match self.view {
            SpaceCoveView::OwnCove => own_team.has_space_cove_on(),
            SpaceCoveView::AllCoves => self
                .cove_index
                .and_then(|i| self.cove_entries.get(i).map(|(_, p)| *p)),
        };

        match selected_asteroid_id.and_then(|id| world.planets.get(&id)) {
            Some(asteroid) => {
                let previous_visitors = std::mem::take(&mut self.visiting_team_ids);
                let new_set: HashSet<TeamId> = asteroid.team_ids.iter().copied().collect();
                self.visiting_team_ids = previous_visitors
                    .iter()
                    .copied()
                    .filter(|id| new_set.contains(id))
                    .collect();
                for id in &asteroid.team_ids {
                    if !self.visiting_team_ids.contains(id) {
                        self.visiting_team_ids.push(*id);
                    }
                }

                let visitors_changed = previous_visitors != self.visiting_team_ids;
                if world.dirty_ui || entries_changed || index_changed || visitors_changed {
                    let teams = self
                        .visiting_team_ids
                        .iter()
                        .take(4)
                        .filter(|id| world.teams.contains_key(id))
                        .map(|id| world.teams.get(id).unwrap())
                        .collect_vec();
                    self.cove_image_widgets = Self::build_image_widgets(&teams)?;
                }
            }
            None => {
                let was_populated = !self.visiting_team_ids.is_empty();
                self.visiting_team_ids.clear();
                if entries_changed || index_changed || was_populated {
                    self.cove_image_widgets = Self::build_image_widgets(&[])?;
                }
            }
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
        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
            .split(area);

        frame.render_widget(default_block(), split[1]);
        let t = self.tick % 60;
        let left_eye_blinking = [2, 3, 5, 13, 33].contains(&t);
        let right_eye_blinking = [2, 3, 6, 7, 41].contains(&t);
        let widget = match (left_eye_blinking, right_eye_blinking) {
            (false, false) => &self.cove_image_widgets[0],
            (true, false) => &self.cove_image_widgets[1],
            (false, true) => &self.cove_image_widgets[2],
            (true, true) => &self.cove_image_widgets[3],
        };
        frame.render_widget(widget, split[1].inner(Margin::new(1, 1)));

        let own_team = world.get_own_team()?;

        let col = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(split[0]);

        self.render_view_buttons(frame, world, col[0], col[1])?;

        match self.view {
            SpaceCoveView::OwnCove => {
                frame.render_widget(default_block().title("Upgrades"), col[2]);
                self.render_visiting_teams(frame, world, col[3])?;

                let asteroid = own_team
                    .has_space_cove_on()
                    .and_then(|id| world.planets.get(&id));
                self.render_tournament_button(
                    frame,
                    own_team,
                    asteroid,
                    TournamentType::Cup,
                    col[4],
                );
                self.render_tournament_button(
                    frame,
                    own_team,
                    asteroid,
                    TournamentType::Supercup,
                    col[5],
                );
            }
            SpaceCoveView::AllCoves => {
                self.render_cove_list(frame, world, col[2])?;
                self.render_visiting_teams(frame, world, col[3])?;

                let selected_asteroid = self
                    .cove_index
                    .and_then(|i| self.cove_entries.get(i).map(|(_, p)| *p))
                    .and_then(|id| world.planets.get(&id));
                self.render_go_to_planet_slot(frame, world, selected_asteroid, col[4])?;
                self.render_raid_placeholder(frame, col[5]);
            }
        }

        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, _world: &World) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up if self.view == SpaceCoveView::AllCoves => self.next_index(),
            KeyCode::Down if self.view == SpaceCoveView::AllCoves => self.previous_index(),
            ui_key::CYCLE_VIEW => {
                return Some(UiCallback::SetSpaceCovePanelView {
                    view: self.view.next(),
                });
            }
            ui_key::CYCLE_VIEW_BACK => {
                return Some(UiCallback::SetSpaceCovePanelView {
                    view: self.view.previous(),
                });
            }
            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" {} ", ui_key::CYCLE_VIEW),
            " Cycle view ".to_string(),
        ]
    }

    fn render_help_widget(
        &self,
        frame: &mut UiFrame,
        _world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        render_help_block(
            frame,
            area,
            vec![
                Line::from(" Manage your own space cove and browse other crews' coves."),
                Line::from(" Use the two buttons at the top to switch view: yours"),
                Line::from(" (tournaments + upgrades) or other coves (list + travel)."),
            ],
            vec![
                (
                    " Manage the asteroid that hosts your cove from ",
                    "My Team",
                    UiTab::MyTeam,
                    ".",
                ),
                (
                    " Inspect visiting crews directly, or browse all in ",
                    "Crews",
                    UiTab::Crews,
                    ".",
                ),
                (
                    " To find another asteroid candidate, explore the ",
                    "Galaxy",
                    UiTab::Galaxy,
                    ".",
                ),
            ],
            vec![
                Line::from(" Controls:"),
                Line::from(format!(
                    "   {}        Cycle between Own cove and Other coves view",
                    ui_key::CYCLE_VIEW
                )),
                Line::from("   ↑/↓        Move highlight in the cove list (Other coves view)"),
                Line::from(format!(
                    "   {}          Teleport / Travel to the selected cove asteroid",
                    ui_key::TRAVEL
                )),
                Line::from(format!(
                    "   {}          Organize a quick tournament (own cove only)",
                    ui_key::ORGANIZE_QUICK_TOURNAMENT
                )),
                Line::from(format!(
                    "   {}          Organize a big tournament (own cove only)",
                    ui_key::ORGANIZE_BIG_TOURNAMENT
                )),
            ],
        );
        Ok(())
    }
}

impl SplitPanel for SpaceCovePanel {
    fn index(&self) -> Option<usize> {
        self.cove_index
    }

    fn max_index(&self) -> usize {
        self.cove_entries.len()
    }

    fn set_index(&mut self, index: usize) {
        if self.max_index() > 0 {
            self.cove_index = Some(index % self.max_index());
        }
    }
}
