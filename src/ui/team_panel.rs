use super::button::Button;
use super::clickable_list::ClickableListState;
use super::gif_map::GifMap;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::widgets::{
    go_to_team_current_planet_button, render_challenge_button, render_spaceship_description,
};
use super::{
    constants::*,
    traits::{Screen, SplitPanel},
    utils::img_to_lines,
    widgets::{default_block, selectable_list},
};
use crate::core::constants::MIN_PLAYERS_PER_GAME;
use crate::core::team::Team;
use crate::image::spaceship::{SPACESHIP_IMAGE_HEIGHT, SPACESHIP_IMAGE_WIDTH};
use crate::types::AppResult;
use crate::ui::ui_key;
use crate::{
    core::{
        position::{GamePosition, GamePositionUtils},
        skill::Rated,
        world::World,
    },
    image::game::floor_from_size,
    image::player::{PLAYER_IMAGE_HEIGHT, PLAYER_IMAGE_WIDTH},
    types::{PlayerId, TeamId},
};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use ratatui::layout::Margin;
use ratatui::style::{Styled, Stylize};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    prelude::Rect,
    widgets::Paragraph,
};
use std::fmt::Display;

const IMG_FRAME_WIDTH: u16 = 80;

#[derive(Debug, Clone, Copy, Default, PartialEq, Hash)]
pub enum TeamView {
    #[default]
    All,
    OpenToChallenge,
    Peers,
}

impl TeamView {
    fn next(&self) -> Self {
        match self {
            TeamView::All => TeamView::OpenToChallenge,
            TeamView::OpenToChallenge => TeamView::Peers,
            TeamView::Peers => TeamView::All,
        }
    }

    fn rule(&self, team: &Team, own_team: &Team) -> bool {
        match self {
            TeamView::All => true,
            TeamView::OpenToChallenge => {
                own_team.can_challenge_local_team(team).is_ok()
                    || own_team.can_challenge_network_team(team).is_ok()
            }
            TeamView::Peers => team.peer_id.is_some(),
        }
    }
}

impl Display for TeamView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::OpenToChallenge => write!(f, "Open to challenge"),
            Self::Peers => write!(f, "From swarm"),
        }
    }
}

#[derive(Debug, Default)]
pub struct TeamListPanel {
    pub index: Option<usize>,
    pub player_index: usize,
    pub selected_player_id: PlayerId,
    pub selected_team_id: TeamId,
    pub team_ids: Vec<TeamId>,
    pub all_team_ids: Vec<TeamId>,
    view: TeamView,
    update_view: bool,
    current_team_players_length: usize,
    tick: usize,
    gif_map: GifMap,
}

impl TeamListPanel {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_player_index(&mut self) {
        if self.current_team_players_length == 0 {
            return;
        }
        let current_index = self.player_index;
        self.player_index = (current_index + 1) % self.current_team_players_length;
    }

    fn previous_player_index(&mut self) {
        if self.current_team_players_length == 0 {
            return;
        }
        let current_index = self.player_index;
        self.player_index = (current_index + self.current_team_players_length - 1)
            % self.current_team_players_length;
    }

    fn build_left_panel(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

        let mut filter_all_button = Button::new(
            TeamView::All.to_string(),
            UiCallback::SetTeamPanelView {
                view: TeamView::All,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View all teams.");

        let mut filter_challenge_button = Button::new(
            TeamView::OpenToChallenge.to_string(),
            UiCallback::SetTeamPanelView {
                view: TeamView::OpenToChallenge,
            },
        )
        .bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text("View all teams that can be currently challenged to a game.");

        let mut filter_peers_button = Button::new(
            TeamView::Peers.to_string(),
            UiCallback::SetTeamPanelView {
                view: TeamView::Peers,
            },
        ).bold()
        .set_hotkey(ui_key::CYCLE_VIEW)
        .set_hover_text(
            "View all teams received from the network (i.e. teams controlled by other players online)."
                ,
        );
        match self.view {
            TeamView::All => filter_all_button.select(),
            TeamView::OpenToChallenge => filter_challenge_button.select(),
            TeamView::Peers => filter_peers_button.select(),
        }

        frame.render_interactive_widget(filter_all_button, split[0]);
        frame.render_interactive_widget(filter_challenge_button, split[1]);
        frame.render_interactive_widget(filter_peers_button, split[2]);

        if !self.team_ids.is_empty() {
            let mut options = vec![];
            for team_id in self.team_ids.iter() {
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
                split[3],
                &mut ClickableListState::default().with_selected(self.index),
            );
        } else {
            frame.render_widget(default_block().title("Teams"), split[3]);
        }
    }

    fn build_right_panel(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let index = if let Some(index) = self.index {
            index
        } else {
            frame.render_widget(default_block(), area);
            return Ok(());
        };

        let team = world.get_team_or_err(&self.team_ids[index])?;
        self.current_team_players_length = team.player_ids.len();
        let vertical_split = Layout::vertical([
            Constraint::Length(PLAYER_IMAGE_HEIGHT as u16 / 2), //players
            Constraint::Length(1),                              //floor
            Constraint::Length(1),                              //name
            Constraint::Length(2),                              //rating
            Constraint::Min(1),                                 //bottom
        ])
        .split(area);

        let floor = floor_from_size(area.width as u32, 2);
        frame.render_widget(
            Paragraph::new(img_to_lines(&floor)).centered(),
            vertical_split[1].inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );

        let side_length = if area.width > (PLAYER_IMAGE_WIDTH as u16 + 2) * 5 {
            (area.width - (PLAYER_IMAGE_WIDTH as u16 + 2) * 5) / 2
        } else {
            0
        };

        let constraints = [
            Constraint::Min(side_length),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(PLAYER_IMAGE_WIDTH as u16 + 2),
            Constraint::Min(side_length),
        ];

        let player_img_split = Layout::horizontal(constraints).split(vertical_split[0]);
        let player_name_split = Layout::horizontal(constraints).split(vertical_split[2]);
        let player_rating_split = Layout::horizontal(constraints).split(vertical_split[3]);

        for i in 0..MIN_PLAYERS_PER_GAME {
            if i >= team.player_ids.len() {
                break;
            }

            // recalculate button area: to offset the missing box of the radiobutton
            // we add an extra row to top and bottom
            let button_area = Rect::new(
                player_img_split[i + 1].x,
                player_img_split[i + 1].y,
                player_img_split[i + 1].width,
                player_img_split[i + 1].height + 1,
            );

            let mut button = Button::no_box(
                "",
                UiCallback::GoToPlayer {
                    player_id: team.player_ids[i],
                },
            )
            .set_hover_style(UiStyle::SELECTED);
            if self.player_index == i {
                button = button.set_style(UiStyle::SELECTED);
            }

            frame.render_interactive_widget(button, button_area);

            let player = world.get_player_or_err(&team.player_ids[i])?;

            if let Ok(lines) = self.gif_map.player_frame_lines(player, self.tick) {
                frame.render_widget(Paragraph::new(lines).centered(), player_img_split[i + 1]);
            }

            frame.render_widget(
                Paragraph::new(player.info.short_name()).centered(),
                player_name_split[i + 1],
            );

            frame.render_widget(
                Paragraph::new(format!(
                    "{} {}",
                    (i as GamePosition).as_str(),
                    (i as GamePosition)
                        .player_rating(player.current_skill_array())
                        .stars()
                ))
                .centered(),
                player_rating_split[i + 1],
            );
        }

        let bottom_split = Layout::horizontal([
            Constraint::Length(44),
            Constraint::Min(SPACESHIP_IMAGE_WIDTH as u16 + 2 + 34),
        ])
        .split(vertical_split[4]);

        let bench_out_split =
            Layout::vertical([Constraint::Length(6), Constraint::Min(0)]).split(bottom_split[0]);

        frame.render_widget(default_block().title("Bench"), bench_out_split[0]);
        frame.render_widget(default_block().title("Out"), bench_out_split[1]);

        if team.player_ids.len() > MIN_PLAYERS_PER_GAME {
            let out_row_split = Layout::vertical([
                Constraint::Length(4),
                Constraint::Length(4),
                Constraint::Min(0),
            ])
            .split(bench_out_split[1].inner(Margin {
                horizontal: 2,
                vertical: 1,
            }));

            let row_splits = [
                Layout::horizontal([Constraint::Length(20), Constraint::Length(20)]).split(
                    bench_out_split[0].inner(Margin {
                        horizontal: 2,
                        vertical: 1,
                    }),
                ),
                Layout::horizontal([Constraint::Length(20), Constraint::Length(20)])
                    .split(out_row_split[0]),
                Layout::horizontal([Constraint::Length(20), Constraint::Length(20)])
                    .split(out_row_split[1]),
            ];

            for (i, player_id) in team
                .player_ids
                .iter()
                .skip(MIN_PLAYERS_PER_GAME)
                .enumerate()
            {
                if let Some(player) = world.get_player(player_id) {
                    let info = format!("{}\n", player.info.short_name());
                    let skills = player.current_skill_array();
                    let best_role = GamePosition::best(skills);

                    let role_info = format!(
                        "{:<2} {:<5}",
                        best_role.as_str(),
                        best_role.player_rating(skills).stars()
                    );
                    let mut button = Button::new(
                        format!("{info}{role_info}"),
                        UiCallback::GoToPlayer {
                            player_id: team.player_ids[i + MIN_PLAYERS_PER_GAME],
                        },
                    );

                    if self.player_index == i + MIN_PLAYERS_PER_GAME {
                        button = button
                            .set_style(UiStyle::SELECTED)
                            .set_hover_style(UiStyle::SELECTED);
                    }

                    let row = i / 2;
                    let column = i % 2;
                    let area = row_splits[row][column];

                    frame.render_interactive_widget(button, area);
                }
            }
        }

        let ship_buttons_split = Layout::vertical([
            Constraint::Min(SPACESHIP_IMAGE_HEIGHT as u16 / 2 + 1), // ship
            Constraint::Length(3),                                  //button
        ])
        .split(bottom_split[1].inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        let button_split = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(ship_buttons_split[1]);

        match go_to_team_current_planet_button(world, &team.id) {
            Ok(go_to_team_current_planet_button) => {
                frame.render_interactive_widget(go_to_team_current_planet_button, button_split[0])
            }
            Err(e) => log::error!("go_to_team_current_planet_button error: {e} "),
        }

        if team.id != world.own_team_id {
            render_challenge_button(world, team, true, frame, button_split[1])?;
        }

        render_spaceship_description(
            team,
            world,
            world.team_rating(&team.id).unwrap_or_default(),
            false,
            world.get_team(&team.id).is_some(),
            &mut self.gif_map,
            self.tick,
            frame,
            bottom_split[1],
        );

        let box_split = Layout::vertical([
            Constraint::Length(PLAYER_IMAGE_HEIGHT as u16 / 2 + 4),
            Constraint::Min(0),
        ])
        .split(area);

        frame.render_widget(
            default_block()
                .title(format!(
                    "{} {}/{}",
                    team.name,
                    ui_key::NEXT_SELECTION,
                    ui_key::PREVIOUS_SELECTION
                ))
                .title_alignment(Alignment::Left),
            box_split[0],
        );

        Ok(())
    }

    pub fn set_view(&mut self, filter: TeamView) {
        self.view = filter;
        self.update_view = true;
    }

    pub fn reset_view(&mut self) {
        self.set_view(TeamView::All);
    }
}

impl Screen for TeamListPanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        if world.dirty_ui || self.all_team_ids.len() != world.teams.len() {
            self.all_team_ids = world.teams.keys().copied().collect();
            self.all_team_ids.sort_by(|a, b| {
                let a = world.get_team_or_err(a).unwrap();
                let b = world.get_team_or_err(b).unwrap();
                world
                    .team_rating(&b.id)
                    .unwrap_or_default()
                    .partial_cmp(&world.team_rating(&a.id).unwrap_or_default())
                    .expect("Rating should exists")
            });
            self.update_view = true;
        }

        if self.update_view {
            self.team_ids = self
                .all_team_ids
                .iter()
                .filter(|&team_id| {
                    let team = world.get_team_or_err(team_id).unwrap();
                    self.view.rule(team, world.get_own_team().unwrap())
                })
                .copied()
                .collect();
            self.update_view = false;
        }

        if let Some(index) = self.index {
            if self.team_ids.is_empty() {
                self.index = None;
            } else if index >= self.team_ids.len() && !self.team_ids.is_empty() {
                self.set_index(self.team_ids.len() - 1);
            }

            if index < self.team_ids.len() {
                self.selected_team_id = self.team_ids[index];
                let players = world
                    .get_team_or_err(&self.selected_team_id)?
                    .player_ids
                    .clone();
                if self.player_index < players.len() {
                    self.selected_player_id = players[self.player_index];
                }
            }
        } else if !self.team_ids.is_empty() {
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
        if self.all_team_ids.is_empty() {
            frame.render_widget(
                Paragraph::new(" No team yet!"),
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                }),
            );
            return Ok(());
        }

        // Split into left and right panels
        let left_right_split = Layout::horizontal([
            Constraint::Length(LEFT_PANEL_WIDTH),
            Constraint::Min(IMG_FRAME_WIDTH),
        ])
        .split(area);
        self.build_left_panel(frame, world, left_right_split[0]);
        self.build_right_panel(frame, world, left_right_split[1])?;
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            ui_key::NEXT_SELECTION => self.next_player_index(),
            ui_key::PREVIOUS_SELECTION => self.previous_player_index(),
            ui_key::CYCLE_VIEW => {
                return Some(UiCallback::SetTeamPanelView {
                    view: self.view.next(),
                });
            }
            KeyCode::Enter => {
                let player_id = self.selected_player_id;
                return Some(UiCallback::GoToPlayer { player_id });
            }
            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" {} ", ui_key::CYCLE_VIEW.to_string()),
            " Next tab ".to_string(),
            format!(
                " {}/{} ",
                ui_key::PREVIOUS_SELECTION.to_string(),
                ui_key::NEXT_SELECTION.to_string()
            ),
            " Select player ".to_string(),
        ]
    }
}

impl SplitPanel for TeamListPanel {
    fn index(&self) -> Option<usize> {
        self.index
    }

    fn max_index(&self) -> usize {
        self.team_ids.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = Some(index);
    }
}
