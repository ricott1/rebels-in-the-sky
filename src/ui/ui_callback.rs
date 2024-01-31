use super::{
    galaxy_panel::ZoomLevel,
    new_team_screen::CreationState,
    player_panel::PlayerFilter,
    swarm_panel::EventTopic,
    team_panel::TeamFilter,
    traits::{Screen, SplitPanel},
    ui::{UiState, UiTab},
};
use crate::{
    app::App,
    engine::{tactic::OffenseTactic, types::TeamInGame},
    image::color_map::{ColorMap, ColorPreset},
    network::{constants::DEFAULT_PORT, types::Challenge},
    types::{
        AppCallback, AppResult, GameId, IdSystem, PlanetId, PlayerId, SystemTimeTick, TeamId, Tick,
        SECONDS,
    },
    world::{
        jersey::{Jersey, JerseyStyle},
        role::CrewRole,
        spaceship::Spaceship,
        team::Team,
        types::{PlayerLocation, TeamLocation, TrainingFocus},
    },
};
use crossterm::event::{MouseEvent, MouseEventKind};
use rand::Rng;
use ratatui::layout::Rect;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum UiCallbackPreset {
    None,
    SetPanelIndex {
        index: usize,
    },
    GoToTeam {
        team_id: TeamId,
    },
    GoToPlayer {
        player_id: PlayerId,
    },
    GoToPlayerTeam {
        player_id: PlayerId,
    },
    GoToHomePlanet {
        team_id: TeamId,
    },
    GoToCurrentTeamPlanet {
        team_id: TeamId,
    },
    GoToCurrentPlayerPlanet {
        player_id: PlayerId,
    },
    GoToPlanetZoomIn {
        planet_id: PlanetId,
    },
    GoToPlanetZoomOut {
        planet_id: PlanetId,
    },
    ChallengeTeam {
        team_id: TeamId,
    },
    AcceptChallenge {
        challenge: Challenge,
    },
    DeclineChallenge {
        challenge: Challenge,
    },
    SetTeamColors {
        color: ColorPreset,
        channel: usize,
    },
    SetTeamOffenseTactic {
        tactic: OffenseTactic,
    },
    SetNextTeamOffenseTactic,
    NextUiTab,
    PreviousUiTab,
    SetUiTab {
        ui_tab: UiTab,
    },
    NextPanelIndex,
    PreviousPanelIndex,
    CloseUiPopup,
    NewGame,
    ContinueGame,
    QuitGame,
    ToggleAudio,
    SetSwarmPanelTopic {
        topic: EventTopic,
    },
    SetPlayerPanelFilter {
        filter: PlayerFilter,
    },
    SetTeamPanelFilter {
        filter: TeamFilter,
    },
    HirePlayer {
        player_id: PlayerId,
    },
    ReleasePlayer {
        player_id: PlayerId,
    },
    LockPlayerPanel {
        player_id: PlayerId,
    },
    SetCrewRole {
        player_id: PlayerId,
        role: CrewRole,
    },
    GeneratePlayerTeam {
        name: String,
        home_planet: PlanetId,
        jersey_style: JerseyStyle,
        jersey_colors: ColorMap,
        players: Vec<PlayerId>,
        balance: u32,
        spaceship: Spaceship,
    },
    CancelGeneratePlayerTeam,
    AssignBestTeamPositions,
    SwapPlayerPositions {
        player_id: PlayerId,
        position: usize,
    },
    NextTrainingFocus {
        player_id: PlayerId,
    },
    TravelToPlanet {
        planet_id: PlanetId,
    },
    ZoomInToPlanet {
        planet_id: PlanetId,
    },
    Dial {
        address: String,
    },
    Sync,
    SendMessage {
        message: String,
    },
}

impl UiCallbackPreset {
    fn go_to_team(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.team_panel.reset_filter();
            if let Some(index) = app
                .ui
                .team_panel
                .all_teams
                .iter()
                .position(|&x| x == team_id)
            {
                app.ui.team_panel.set_index(index);
                app.ui.team_panel.player_index = 0;
                app.ui.switch_to(super::ui::UiTab::Team);
            }
            Ok(None)
        })
    }

    fn go_to_player(player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.player_panel.reset_filter();
            //FIXME: sometimes this search fails
            if let Some(index) = app
                .ui
                .player_panel
                .all_players
                .iter()
                .position(|&x| x == player_id)
            {
                app.ui.player_panel.set_index(index);
                app.ui.switch_to(super::ui::UiTab::Player);
            }

            Ok(None)
        })
    }

    fn go_to_player_team(player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team_id = app
                .world
                .get_player(player_id)
                .ok_or(format!("Player {:?} not found", player_id))?
                .team
                .ok_or(format!("Player {:?} has no team", player_id))?;
            if let Some(index) = app.ui.team_panel.teams.iter().position(|&x| x == team_id) {
                app.ui.team_panel.set_index(index);
                let player_index = app
                    .world
                    .get_team_or_err(team_id)?
                    .player_ids
                    .iter()
                    .position(|&x| x == player_id)
                    .unwrap_or_default();
                app.ui.team_panel.player_index = player_index;
                app.ui.switch_to(super::ui::UiTab::Team);
            }

            Ok(None)
        })
    }

    fn go_to_home_planet(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team = app.world.get_team_or_err(team_id)?;

            let target = app.world.get_planet_or_err(team.home_planet)?;

            let team_index = target.teams.iter().position(|&x| x == team_id);

            app.ui
                .galaxy_panel
                .go_to_planet(team.home_planet, team_index, ZoomLevel::In);
            app.ui.switch_to(super::ui::UiTab::Galaxy);

            Ok(None)
        })
    }

    fn go_to_current_team_planet(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team = app.world.get_team_or_err(team_id)?;

            let target = match team.current_location {
                TeamLocation::OnPlanet {
                    planet_id: current_planet_id,
                } => app.world.get_planet_or_err(current_planet_id)?,
                TeamLocation::Travelling { .. } => {
                    return Err("Team is travelling".into());
                }
            };

            let team_index = target.teams.iter().position(|&x| x == team_id);

            app.ui
                .galaxy_panel
                .go_to_planet(target.id, team_index, ZoomLevel::In);
            app.ui.switch_to(super::ui::UiTab::Galaxy);

            Ok(None)
        })
    }

    fn go_to_current_player_planet(player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let player = app.world.get_player_or_err(player_id)?;

            match player.current_location {
                PlayerLocation::OnPlanet {
                    planet_id: current_planet_id,
                } => {
                    let target = app.world.get_planet_or_err(current_planet_id)?;
                    app.ui
                        .galaxy_panel
                        .go_to_planet(target.id, None, ZoomLevel::In);
                    app.ui.switch_to(super::ui::UiTab::Galaxy);
                }
                PlayerLocation::WithTeam => {
                    return Self::go_to_current_team_planet(player.team.unwrap())(app);
                }
            };

            Ok(None)
        })
    }

    fn go_to_planet_zoom_in(planet_id: PlanetId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui
                .galaxy_panel
                .go_to_planet(planet_id, None, ZoomLevel::In);
            app.ui.switch_to(super::ui::UiTab::Galaxy);
            Ok(None)
        })
    }

    fn go_to_planet_zoom_out(planet_id: PlanetId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui
                .galaxy_panel
                .go_to_planet(planet_id, None, ZoomLevel::Out);
            app.ui.switch_to(super::ui::UiTab::Galaxy);
            Ok(None)
        })
    }

    fn zoom_in_to_planet(planet_id: PlanetId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let target = app.world.get_planet_or_err(planet_id)?;
            let panel = &mut app.ui.galaxy_panel;

            if panel.planet_index == 0 {
                // return Err(
                //     format!("SIAMO QUI caso su {} {} ", panel.planet_index, planet_id).into(),
                // );
                panel.zoom_level = ZoomLevel::In;

                if target.teams.len() == 0 {
                    panel.team_index = None;
                } else {
                    panel.team_index = Some(0);
                }
            } else {
                panel.planet_id = target.satellites[panel.planet_index - 1].clone();

                let new_target = panel
                    .planets
                    .get(&panel.planet_id)
                    .ok_or(format!("Planet {:?} not found", panel.planet_id))?;

                panel.planet_index = 0;
                if new_target.satellites.len() == 0 {
                    panel.zoom_level = ZoomLevel::In;
                    if new_target.teams.len() == 0 {
                        panel.team_index = None;
                    } else {
                        panel.team_index = Some(0);
                    }
                } else {
                    panel.zoom_level = ZoomLevel::Out;
                }
            }

            Ok(None)
        })
    }

    fn challenge_team(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            if !app.world.has_own_team() {
                return Err("No own team".into());
            }

            let own_team_id = app.world.own_team_id;
            let own_team = app.world.get_team_or_err(own_team_id)?;

            let team = app.world.get_team_or_err(team_id)?;

            own_team.can_challenge_team(team)?;

            if let Some(peer_id) = team.peer_id {
                // if !app
                //     .network_handler
                //     .as_ref()
                //     .unwrap()
                //     .swarm
                //     .is_connected(&peer_id)
                // {
                //     return Err("Team is not connected".into());
                // }
                app.network_handler
                    .as_mut()
                    .unwrap()
                    .send_new_challenge(&app.world, peer_id)?;
                return Ok(Some("Challenge sent".to_string()));
            }
            let (home_team_in_game, away_team_in_game) = match rand::thread_rng().gen_range(0..=1) {
                0 => (
                    TeamInGame::from_team_id(own_team_id, &app.world.teams, &app.world.players)
                        .ok_or(format!("Own team {:?} not found", own_team_id))?,
                    TeamInGame::from_team_id(team_id, &app.world.teams, &app.world.players)
                        .ok_or(format!("Team {:?} not found", team_id))?,
                ),

                _ => (
                    TeamInGame::from_team_id(team_id, &app.world.teams, &app.world.players)
                        .ok_or(format!("Team {:?} not found", team_id))?,
                    TeamInGame::from_team_id(own_team_id, &app.world.teams, &app.world.players)
                        .ok_or(format!("Own team {:?} not found", own_team_id))?,
                ),
            };

            let game_id = GameId::new();
            app.world.generate_game(
                game_id,
                home_team_in_game,
                away_team_in_game,
                Tick::now() + 30 * SECONDS,
            )?;

            app.ui.game_panel.update(&app.world)?;

            let index = app
                .ui
                .game_panel
                .games
                .iter()
                .position(|&x| x == game_id)
                .ok_or::<String>(format!("Game {:?} not found", game_id).into())?;

            app.ui.game_panel.set_index(index);
            app.ui.switch_to(super::ui::UiTab::Game);
            // if let Some(network_handler) = app.network_handler.as_mut() {
            //     network_handler.decline_all_challenges()?;
            //     app.ui.swarm_panel.remove_all_challenges();
            // }
            return Ok(Some("Challenge accepted".to_string()));
        })
    }

    fn next_ui_tab() -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.next_tab();
            Ok(None)
        })
    }

    fn previous_ui_tab() -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.previous_tab();
            Ok(None)
        })
    }

    fn set_ui_tab(ui_tab: UiTab) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.switch_to(ui_tab);
            Ok(None)
        })
    }

    fn next_panel_index() -> AppCallback {
        Box::new(move |app: &mut App| {
            if let Some(panel) = app.ui.get_active_panel() {
                panel.next_index();
            }
            Ok(None)
        })
    }

    fn previous_panel_index() -> AppCallback {
        Box::new(move |app: &mut App| {
            if let Some(panel) = app.ui.get_active_panel() {
                panel.previous_index();
            }
            Ok(None)
        })
    }

    fn generate_player_team(
        name: String,
        home_planet: PlanetId,
        jersey_style: JerseyStyle,
        jersey_colors: ColorMap,
        players: Vec<PlayerId>,
        balance: u32,
        spaceship: Spaceship,
    ) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.world.generate_player_team(
                name.clone(),
                home_planet,
                jersey_style,
                jersey_colors,
                players.clone(),
                balance,
                spaceship.clone(),
            )?;
            app.ui.set_state(UiState::Main);
            Ok(None)
        })
    }

    fn cancel_generate_player_team() -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.new_team_screen.set_state(CreationState::Players);
            app.ui.new_team_screen.clear_selected_players();
            Ok(None)
        })
    }

    fn assign_best_team_positions() -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut team = app.world.get_own_team()?.clone();
            team.player_ids = Team::best_position_assignment(
                team.player_ids
                    .iter()
                    .map(|id| app.world.players.get(id).unwrap())
                    .collect(),
            );
            app.world.teams.insert(team.id, team);
            app.world.dirty = true;
            app.world.dirty_ui = true;

            Ok(None)
        })
    }

    fn swap_player_positions(player_id: PlayerId, position: usize) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut team = app.world.get_own_team()?.clone();
            let current_player_position = team
                .player_ids
                .iter()
                .position(|&id| id == player_id)
                .unwrap();
            team.player_ids.swap(position, current_player_position);
            app.world.dirty = true;
            app.world.dirty_ui = true;
            app.world.teams.insert(team.id, team);
            Ok(None)
        })
    }

    fn next_training_focus(player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut player = app
                .world
                .get_player(player_id)
                .ok_or("Failed to get player")?
                .clone();
            let team = app.world.get_team_or_err(
                player
                    .team
                    .ok_or("Player has no team. This should not happen".to_string())?,
            )?;

            if team.current_game.is_some() {
                return Err("Cannot change training focus:\nTeam is currently playing".into());
            }

            let new_focus = match player.training_focus {
                Some(focus) => focus.next(),
                None => Some(TrainingFocus::default()),
            };
            player.training_focus = new_focus;
            app.world.players.insert(player.id, player);
            app.world.dirty = true;
            app.world.dirty_ui = true;
            Ok(None)
        })
    }

    fn travel_to_planet(planet_id: PlanetId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut own_team = app.world.get_own_team()?.clone();

            let target_planet = app.world.get_planet_or_err(planet_id)?;

            let mut current_planet = match own_team.current_location {
                TeamLocation::OnPlanet {
                    planet_id: current_planet_id,
                } => {
                    if current_planet_id == planet_id {
                        return Err("Already on planet".into());
                    }
                    app.world.get_planet_or_err(current_planet_id)?.clone()
                }
                _ => return Err("Team is travelling".into()),
            };

            let travel_time = app
                .world
                .travel_time_to_planet(own_team.id, target_planet.id)?;
            own_team.can_travel_to_planet(&target_planet, travel_time)?;

            own_team.current_location = TeamLocation::Travelling {
                from: current_planet.id,
                to: planet_id,
                started: Tick::now(),
                duration: travel_time,
            };

            current_planet.teams.retain(|&x| x != own_team.id);
            app.world.planets.insert(current_planet.id, current_planet);

            let pirate_jersey = Jersey {
                style: JerseyStyle::Pirate,
                color: own_team.jersey.color.clone(),
            };

            for player in own_team.player_ids.iter() {
                let mut player = app.world.get_player_or_err(*player)?.clone();
                player.set_jersey(&pirate_jersey);
                app.world.players.insert(player.id, player);
            }

            app.world.teams.insert(own_team.id, own_team);
            app.world.dirty = true;
            app.world.dirty_network = true;
            app.world.dirty_ui = true;

            Ok(None)
        })
    }

    fn dial(address: String) -> AppCallback {
        Box::new(move |app: &mut App| {
            let multiaddr = match address.clone() {
                x if x == "seed".to_string() => {
                    app.network_handler.as_ref().unwrap().seed_address.clone()
                }
                _ => format!("/ip4/{address}/tcp/{DEFAULT_PORT}")
                    .as_str()
                    .parse()?,
            };
            app.network_handler
                .as_mut()
                .unwrap()
                .dial(multiaddr)
                .map_err(|e| e.to_string())?;
            app.world.dirty_network = true;
            Ok(None)
        })
    }

    fn sync() -> AppCallback {
        Box::new(move |app: &mut App| {
            app.world.dirty_network = true;
            Ok(None)
        })
    }

    fn send(message: String) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.network_handler
                .as_mut()
                .unwrap()
                .send_msg(message.clone())?;

            Ok(None)
        })
    }

    pub fn call(&self, app: &mut App) -> AppResult<Option<String>> {
        match self {
            UiCallbackPreset::None => Ok(None),
            UiCallbackPreset::SetPanelIndex { index } => {
                if let Some(panel) = app.ui.get_active_panel() {
                    panel.set_index(*index);
                }
                Ok(None)
            }
            UiCallbackPreset::GoToTeam { team_id } => Self::go_to_team(*team_id)(app),
            UiCallbackPreset::GoToPlayer { player_id } => Self::go_to_player(*player_id)(app),
            UiCallbackPreset::GoToPlayerTeam { player_id } => {
                Self::go_to_player_team(*player_id)(app)
            }
            UiCallbackPreset::GoToHomePlanet { team_id } => Self::go_to_home_planet(*team_id)(app),
            UiCallbackPreset::GoToCurrentTeamPlanet { team_id } => {
                Self::go_to_current_team_planet(*team_id)(app)
            }
            UiCallbackPreset::GoToCurrentPlayerPlanet { player_id } => {
                Self::go_to_current_player_planet(*player_id)(app)
            }

            UiCallbackPreset::GoToPlanetZoomIn { planet_id } => {
                Self::go_to_planet_zoom_in(*planet_id)(app)
            }
            UiCallbackPreset::GoToPlanetZoomOut { planet_id } => {
                Self::go_to_planet_zoom_out(*planet_id)(app)
            }
            UiCallbackPreset::SetTeamColors { color, channel } => {
                app.ui
                    .new_team_screen
                    .set_team_colors(color.clone(), channel.clone());
                Ok(None)
            }
            UiCallbackPreset::SetTeamOffenseTactic { tactic } => {
                let own_team = app.world.get_own_team()?;
                let mut team = own_team.clone();
                team.game_offense_tactic = tactic.clone();
                app.world.teams.insert(team.id, team);
                app.world.dirty = true;
                app.world.dirty_ui = true;
                app.world.dirty_network = true;
                Ok(None)
            }
            UiCallbackPreset::SetNextTeamOffenseTactic => {
                let own_team = app.world.get_own_team()?;
                let mut team = own_team.clone();
                team.game_offense_tactic = team.game_offense_tactic.next();
                app.world.teams.insert(team.id, team);
                app.world.dirty = true;
                app.world.dirty_ui = true;
                app.world.dirty_network = true;
                Ok(None)
            }
            UiCallbackPreset::ChallengeTeam { team_id } => Self::challenge_team(*team_id)(app),
            UiCallbackPreset::AcceptChallenge { challenge } => {
                app.network_handler
                    .as_mut()
                    .unwrap()
                    .accept_challenge(&&app.world, challenge.clone())?;

                app.ui.swarm_panel.remove_challenge(&challenge.home_peer_id);

                // app.network_handler
                //     .as_mut()
                //     .unwrap()
                //     .decline_all_challenges()?;
                // app.ui.swarm_panel.remove_all_challenges();
                Ok(None)
            }
            UiCallbackPreset::DeclineChallenge { challenge } => {
                app.network_handler
                    .as_mut()
                    .unwrap()
                    .decline_challenge(challenge.clone())?;
                app.ui.swarm_panel.remove_challenge(&challenge.home_peer_id);
                Ok(None)
            }
            UiCallbackPreset::NextUiTab => Self::next_ui_tab()(app),
            UiCallbackPreset::PreviousUiTab => Self::previous_ui_tab()(app),
            UiCallbackPreset::SetUiTab { ui_tab } => Self::set_ui_tab(*ui_tab)(app),
            UiCallbackPreset::NextPanelIndex => Self::next_panel_index()(app),
            UiCallbackPreset::PreviousPanelIndex => Self::previous_panel_index()(app),
            UiCallbackPreset::CloseUiPopup => {
                app.ui.close_popup();
                Ok(None)
            }
            UiCallbackPreset::NewGame => {
                app.ui.set_state(UiState::NewTeam);
                app.new_world();
                Ok(None)
            }
            UiCallbackPreset::ContinueGame => {
                app.load_world();
                if app.world.has_own_team() {
                    app.ui.set_state(UiState::Main);
                } else {
                    app.ui.set_state(UiState::NewTeam);
                }
                Ok(None)
            }
            UiCallbackPreset::QuitGame => {
                app.quit()?;
                Ok(None)
            }
            UiCallbackPreset::ToggleAudio => {
                app.ui.toggle_audio_player();
                Ok(None)
            }
            UiCallbackPreset::SetSwarmPanelTopic { topic } => {
                app.ui.swarm_panel.set_current_topic(*topic);
                Ok(None)
            }
            UiCallbackPreset::SetPlayerPanelFilter { filter } => {
                app.ui.player_panel.set_filter(*filter);
                Ok(None)
            }
            UiCallbackPreset::SetTeamPanelFilter { filter } => {
                app.ui.team_panel.set_filter(*filter);
                Ok(None)
            }
            UiCallbackPreset::HirePlayer { player_id } => {
                app.world
                    .hire_player_for_team(*player_id, app.world.own_team_id)?;

                Ok(None)
            }
            UiCallbackPreset::ReleasePlayer { player_id } => {
                app.world.release_player_from_team(*player_id)?;
                Ok(None)
            }
            UiCallbackPreset::LockPlayerPanel { player_id } => {
                if app.ui.player_panel.locked_player_id.is_some()
                    && app.ui.player_panel.locked_player_id.unwrap() == *player_id
                {
                    app.ui.player_panel.locked_player_id = None;
                } else {
                    app.ui.player_panel.locked_player_id = Some(*player_id);
                }
                Ok(None)
            }
            UiCallbackPreset::SetCrewRole { player_id, role } => {
                app.world.set_team_crew_role(role.clone(), *player_id)?;
                Ok(None)
            }
            UiCallbackPreset::GeneratePlayerTeam {
                name,
                home_planet,
                jersey_style,
                jersey_colors,
                players,
                balance,
                spaceship,
            } => Self::generate_player_team(
                name.clone(),
                *home_planet,
                *jersey_style,
                *jersey_colors,
                players.clone(),
                *balance,
                spaceship.clone(),
            )(app),
            UiCallbackPreset::CancelGeneratePlayerTeam => Self::cancel_generate_player_team()(app),
            UiCallbackPreset::AssignBestTeamPositions => Self::assign_best_team_positions()(app),
            UiCallbackPreset::SwapPlayerPositions {
                player_id,
                position,
            } => Self::swap_player_positions(*player_id, *position)(app),
            UiCallbackPreset::NextTrainingFocus { player_id } => {
                Self::next_training_focus(*player_id)(app)
            }
            UiCallbackPreset::TravelToPlanet { planet_id } => {
                Self::travel_to_planet(*planet_id)(app)
            }
            UiCallbackPreset::ZoomInToPlanet { planet_id } => {
                Self::zoom_in_to_planet(*planet_id)(app)
            }
            UiCallbackPreset::Dial { address } => Self::dial(address.clone())(app),
            UiCallbackPreset::Sync => Self::sync()(app),
            UiCallbackPreset::SendMessage { message } => Self::send(message.clone())(app),
        }
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct CallbackRegistry {
    callbacks: HashMap<MouseEventKind, HashMap<Option<Rect>, UiCallbackPreset>>,
    hovering: (u16, u16),
}

impl CallbackRegistry {
    fn contains(rect: &Rect, x: u16, y: u16) -> bool {
        rect.x <= x && x < rect.x + rect.width && rect.y <= y && y < rect.y + rect.height
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_callback(
        &mut self,
        event_kind: MouseEventKind,
        rect: Option<Rect>,
        callback: UiCallbackPreset,
    ) {
        self.callbacks
            .entry(event_kind)
            .or_insert_with(HashMap::new)
            .insert(rect, callback);
    }

    pub fn clear(&mut self) {
        self.callbacks.clear();
    }

    pub fn is_hovering(&self, rect: Rect) -> bool {
        Self::contains(&rect, self.hovering.0, self.hovering.1)
    }

    pub fn set_hovering(&mut self, event: MouseEvent) {
        self.hovering = (event.column, event.row);
    }

    pub fn handle_event(&self, event: MouseEvent) -> Option<UiCallbackPreset> {
        if let Some(callbacks) = self.callbacks.get(&event.kind) {
            for (rect, callback) in callbacks.iter() {
                if rect.is_none() {
                    return Some(callback.clone());
                } else {
                    let rect = rect.as_ref().unwrap();
                    if Self::contains(rect, event.column, event.row) {
                        return Some(callback.clone());
                    }
                }
            }
        }
        None
    }
}
