use super::{
    galaxy_panel::ZoomLevel,
    my_team_panel::MyTeamView,
    new_team_screen::CreationState,
    player_panel::PlayerView,
    popup_message::PopupMessage,
    swarm_panel::SwarmView,
    team_panel::TeamView,
    traits::{Screen, SplitPanel},
    ui::{UiState, UiTab},
};
use crate::{
    app::App,
    engine::{tactic::Tactic, types::TeamInGame},
    image::color_map::{ColorMap, ColorPreset},
    network::{challenge::Challenge, constants::DEFAULT_PORT, trade::Trade},
    space_adventure::space::Space,
    types::{AppCallback, AppResult, GameId, PlanetId, PlayerId, SystemTimeTick, TeamId, Tick},
    world::{
        constants::*,
        jersey::{Jersey, JerseyStyle},
        player::Trait,
        resources::Resource,
        role::CrewRole,
        skill::MAX_SKILL,
        spaceship::{Spaceship, SpaceshipUpgrade},
        team::Team,
        types::{PlayerLocation, TeamLocation, TrainingFocus},
    },
};
use anyhow::anyhow;
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use log::info;
use rand::{seq::IteratorRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
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
    GoToGame {
        game_id: GameId,
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
    TradeResource {
        resource: Resource,
        amount: i32,
        unit_cost: u32,
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
    CreateTradeProposal {
        proposer_player_id: PlayerId,
        target_player_id: PlayerId,
    },
    AcceptTrade {
        trade: Trade,
    },
    DeclineTrade {
        trade: Trade,
    },
    GoToTrade {
        trade: Trade,
    },
    SetTeamColors {
        color: ColorPreset,
        channel: usize,
    },
    SetTeamTactic {
        tactic: Tactic,
    },
    SetNextTeamTactic,
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
    NextAudioSample,
    SetSwarmPanelView {
        topic: SwarmView,
    },
    SetMyTeamPanelView {
        view: MyTeamView,
    },
    SetPlayerPanelView {
        view: PlayerView,
    },
    SetTeamPanelView {
        view: TeamView,
    },
    HirePlayer {
        player_id: PlayerId,
    },
    PromptReleasePlayer {
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
    Drink {
        player_id: PlayerId,
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
    TogglePitchView,
    TogglePlayerStatusView,
    NextTrainingFocus {
        team_id: TeamId,
    },
    TravelToPlanet {
        planet_id: PlanetId,
    },
    ExploreAroundPlanet {
        duration: Tick,
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
    PushUiPopup {
        popup_message: PopupMessage,
    },
    NameAndAcceptAsteroid {
        name: String,
        filename: String,
    },
    SetUpgradeSpaceship {
        upgrade: SpaceshipUpgrade,
    },
    UpgradeSpaceship {
        upgrade: SpaceshipUpgrade,
    },
    StartSpaceAdventure,
    StopSpaceAdventure,
    MovePlayerLeft,
    MovePlayerRight,
    MovePlayerDown,
    MovePlayerUp,
}

impl UiCallbackPreset {
    fn go_to_team(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.team_panel.reset_view();
            if let Some(index) = app
                .ui
                .team_panel
                .all_teams
                .iter()
                .position(|&x| x == team_id)
            {
                app.ui.team_panel.set_index(index);
                app.ui.team_panel.player_index = 0;
                app.ui.switch_to(super::ui::UiTab::Teams);
            }
            Ok(None)
        })
    }

    fn go_to_player(player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.player_panel.reset_view();
            if let Some(index) = app
                .ui
                .player_panel
                .all_players
                .iter()
                .position(|&x| x == player_id)
            {
                app.ui.player_panel.set_index(index);
                app.ui.switch_to(super::ui::UiTab::Players);
            }

            Ok(None)
        })
    }

    fn go_to_trade(trade: Trade) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.player_panel.update(&app.world)?;
            app.ui.player_panel.reset_view();

            // Display trade differently depending on who is the proposer.
            let (selected_player_id, locked_player_id) =
                if trade.proposer_player.team.expect("Should have a team") == app.world.own_team_id
                {
                    (trade.proposer_player.id, trade.target_player.id)
                } else {
                    (trade.target_player.id, trade.proposer_player.id)
                };

            if let Some(index) = app
                .ui
                .player_panel
                .all_players
                .iter()
                .position(|&x| x == selected_player_id)
            {
                app.ui.player_panel.set_index(index);

                app.ui.player_panel.locked_player_id = Some(locked_player_id);
                app.ui.player_panel.selected_player_id = selected_player_id;
                app.ui.switch_to(super::ui::UiTab::Players);
            }

            Ok(None)
        })
    }

    fn go_to_player_team(player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team_id = app
                .world
                .get_player(player_id)
                .ok_or(anyhow!("Player {:?} not found", player_id))?
                .team
                .ok_or(anyhow!("Player {:?} has no team", player_id))?;
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
                app.ui.switch_to(super::ui::UiTab::Teams);
            }

            Ok(None)
        })
    }

    fn go_to_game(game_id: GameId) -> AppCallback {
        Box::new(move |app: &mut App| {
            if let Some(index) = app.ui.game_panel.games.iter().position(|&x| x == game_id) {
                app.ui.game_panel.set_index(index);
                app.ui.switch_to(super::ui::UiTab::Games);
            }

            Ok(None)
        })
    }

    fn go_to_home_planet(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team = app.world.get_team_or_err(team_id)?;

            let target = app.world.get_planet_or_err(team.home_planet_id)?;

            let team_index = target.team_ids.iter().position(|&x| x == team_id);

            app.ui
                .galaxy_panel
                .go_to_planet(team.home_planet_id, team_index, ZoomLevel::In);
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
                    return Err(anyhow!("Team is travelling"));
                }
                TeamLocation::Exploring { .. } => {
                    return Err(anyhow!("Team is exploring"));
                }
            };

            let team_index = target.team_ids.iter().position(|&x| x == team_id);

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

    fn trade_resource(resource: Resource, amount: i32, unit_cost: u32) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut own_team = app.world.get_own_team()?.clone();
            if amount > 0 {
                own_team.add_resource(resource, amount as u32);
                own_team.remove_resource(Resource::SATOSHI, unit_cost * amount as u32)?;
            } else if amount < 0 {
                own_team.remove_resource(resource, -amount as u32)?;
                own_team.add_resource(Resource::SATOSHI, unit_cost * -amount as u32);
            }
            app.world.teams.insert(own_team.id, own_team);
            app.world.dirty = true;
            app.world.dirty_ui = true;
            Ok(None)
        })
    }

    fn zoom_in_to_planet(planet_id: PlanetId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let target = app.world.get_planet_or_err(planet_id)?;
            let panel = &mut app.ui.galaxy_panel;

            if panel.planet_index == 0 {
                panel.zoom_level = ZoomLevel::In;

                if target.team_ids.len() == 0 {
                    panel.team_index = None;
                } else {
                    panel.team_index = Some(0);
                }
            } else {
                panel.planet_id = target.satellites[panel.planet_index - 1].clone();

                let new_target = panel
                    .planets
                    .get(&panel.planet_id)
                    .ok_or(anyhow!("Planet {:?} not found", panel.planet_id))?;

                panel.planet_index = 0;
                if new_target.satellites.len() == 0 {
                    panel.zoom_level = ZoomLevel::In;
                    if new_target.team_ids.len() == 0 {
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
            let own_team = app.world.get_own_team()?;
            let team = app.world.get_team_or_err(team_id)?;
            own_team.can_challenge_team(team)?;

            if let Some(peer_id) = team.peer_id {
                let challenge = app
                    .network_handler
                    .as_mut()
                    .ok_or(anyhow!("Network handler is not initialized"))?
                    .send_new_challenge(&app.world, peer_id, team.id)?;

                let own_team = app.world.get_own_team_mut()?;
                own_team.add_sent_challenge(challenge);

                return Ok(Some("Challenge sent".to_string()));
            }

            let own_team_id = app.world.own_team_id;
            let (home_team_in_game, away_team_in_game) = match rand::thread_rng().gen_range(0..=1) {
                0 => (
                    TeamInGame::from_team_id(own_team_id, &app.world.teams, &app.world.players)
                        .ok_or(anyhow!("Own team {:?} not found", own_team_id))?,
                    TeamInGame::from_team_id(team_id, &app.world.teams, &app.world.players)
                        .ok_or(anyhow!("Team {:?} not found", team_id))?,
                ),

                _ => (
                    TeamInGame::from_team_id(team_id, &app.world.teams, &app.world.players)
                        .ok_or(anyhow!("Team {:?} not found", team_id))?,
                    TeamInGame::from_team_id(own_team_id, &app.world.teams, &app.world.players)
                        .ok_or(anyhow!("Own team {:?} not found", own_team_id))?,
                ),
            };

            let game_id = app
                .world
                .generate_game(home_team_in_game, away_team_in_game)?;

            app.ui.game_panel.update(&app.world)?;

            let index = app
                .ui
                .game_panel
                .games
                .iter()
                .position(|&x| x == game_id)
                .ok_or(anyhow!("Game {:?} not found", game_id))?;

            app.ui.game_panel.set_index(index);
            app.ui.switch_to(super::ui::UiTab::Games);
            return Ok(Some("Challenge accepted".to_string()));
        })
    }

    fn trade_players(proposer_player_id: PlayerId, target_player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let own_team = app.world.get_own_team()?;

            let target_player = app.world.get_player_or_err(target_player_id)?;
            let target_team = if let Some(team_id) = target_player.team {
                app.world.get_team_or_err(team_id)?
            } else {
                return Err(anyhow!("Target player has no team"));
            };

            let proposer_player = app.world.get_player_or_err(proposer_player_id)?;
            own_team.can_trade_players(proposer_player, target_player, target_team)?;

            if let Some(peer_id) = target_team.peer_id {
                let trade = app
                    .network_handler
                    .as_mut()
                    .ok_or(anyhow!("Network handler is not initialized"))?
                    .send_new_trade(&app.world, peer_id, proposer_player_id, target_player_id)?;
                let own_team = app.world.get_own_team_mut()?;
                own_team.add_sent_trade(trade);
                return Ok(Some("Trade offer sent".to_string()));
            }

            if proposer_player.bare_value() >= target_player.bare_value() {
                app.world
                    .swap_players_team(proposer_player_id, target_player_id)?;

                let locked_id = app.ui.player_panel.locked_player_id;
                let selected_id = app.ui.player_panel.selected_player_id;
                app.ui.player_panel.locked_player_id = Some(selected_id);
                if let Some(player_id) = locked_id {
                    app.ui.player_panel.selected_player_id = player_id;
                }

                return Ok(Some("Trade accepted".to_string()));
            }
            return Ok(Some("Trade Rejected".to_string()));
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

    fn generate_own_team(
        name: String,
        home_planet: PlanetId,
        jersey_style: JerseyStyle,
        jersey_colors: ColorMap,
        players: Vec<PlayerId>,
        balance: u32,
        spaceship: Spaceship,
    ) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.world.generate_own_team(
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

    fn cancel_generate_own_team() -> AppCallback {
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

    fn next_training_focus(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut team = app.world.get_team_or_err(team_id)?.clone();
            if team.current_game.is_some() {
                return Err(anyhow!("Cannot change training focus:\nTeam is playing"));
            }

            let new_focus = match team.training_focus {
                Some(focus) => focus.next(),
                None => Some(TrainingFocus::default()),
            };
            team.training_focus = new_focus;
            app.world.teams.insert(team.id, team);
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
                        return Err(anyhow!("Already on planet"));
                    }
                    app.world.get_planet_or_err(current_planet_id)?.clone()
                }
                TeamLocation::Travelling { .. } => return Err(anyhow!("Team is travelling")),
                TeamLocation::Exploring { .. } => return Err(anyhow!("Team is exploring")),
            };

            let duration = app
                .world
                .travel_time_to_planet(own_team.id, target_planet.id)?;
            own_team.can_travel_to_planet(&target_planet, duration)?;
            let distance = app
                .world
                .distance_between_planets(current_planet.id, target_planet.id)?;
            own_team.current_location = TeamLocation::Travelling {
                from: current_planet.id,
                to: planet_id,
                started: Tick::now(),
                duration,
                distance,
            };

            // For simplicity we just subtract the fuel upfront, maybe would be nicer on UI to
            // show the fuel consumption as the team travels in world.tick_travel,
            // but this would require more operations and checks in the tick function.
            let fuel_consumed =
                (duration as f32 * own_team.spaceship_fuel_consumption()).max(1.0) as u32;
            own_team.remove_resource(Resource::FUEL, fuel_consumed)?;

            info!(
                "Team {:?} is travelling from {:?} to {:?}, consuming {:.2} fuel",
                own_team.id,
                current_planet.id,
                target_planet.id,
                duration as f32 * own_team.spaceship_fuel_consumption()
            );

            current_planet.team_ids.retain(|&x| x != own_team.id);
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

    fn explore_around_planet(duration: Tick) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut own_team = app.world.get_own_team()?.clone();

            let planet_id = match own_team.current_location {
                TeamLocation::OnPlanet { planet_id } => planet_id,
                TeamLocation::Travelling { .. } => return Err(anyhow!("Team is travelling")),
                TeamLocation::Exploring { .. } => return Err(anyhow!("Team is already exploring")),
            };

            let mut around_planet = app.world.get_planet_or_err(planet_id)?.clone();
            own_team.can_explore_around_planet(&around_planet, duration)?;

            own_team.current_location = TeamLocation::Exploring {
                around: planet_id,
                started: Tick::now(),
                duration,
            };

            // For simplicity we just subtract the fuel upfront, maybe would be nicer on UI to
            // show the fuel consumption as the team travels in world.tick_travel,
            // but this would require more operations and checks in the tick function.
            own_team.remove_resource(
                Resource::FUEL,
                (duration as f32 * own_team.spaceship_fuel_consumption()).max(1.0) as u32,
            )?;

            around_planet.team_ids.retain(|&x| x != own_team.id);
            app.world.planets.insert(around_planet.id, around_planet);

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
                x if x == "seed".to_string() => app
                    .network_handler
                    .as_ref()
                    .ok_or(anyhow!("Network handler is not initialized"))?
                    .seed_address
                    .clone(),
                _ => format!("/ip4/{address}/tcp/{DEFAULT_PORT}")
                    .as_str()
                    .parse()?,
            };
            app.network_handler
                .as_mut()
                .ok_or(anyhow!("Network handler is not initialized"))?
                .dial(multiaddr)
                .map_err(|e| anyhow!(e.to_string()))?;
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
                .ok_or(anyhow!("Network handler is not initialized"))?
                .send_msg(message.clone())?;

            Ok(None)
        })
    }

    fn name_and_accept_asteroid(name: String, filename: String) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut team = app.world.get_own_team()?.clone();
            if team.asteroid_ids.len() > MAX_NUM_ASTEROID_PER_TEAM {
                return Err(anyhow!("Team has reached max number of asteroids."));
            }

            match team.current_location {
                TeamLocation::OnPlanet { planet_id } => {
                    let asteroid_id = app.world.generate_team_asteroid(
                        name.clone(),
                        filename.clone(),
                        planet_id,
                    )?;
                    team.current_location = TeamLocation::OnPlanet {
                        planet_id: asteroid_id,
                    };

                    let mut asteroid = app.world.get_planet_or_err(asteroid_id)?.clone();
                    asteroid.team_ids.push(team.id);
                    asteroid.version += 1;

                    team.asteroid_ids.push(asteroid_id);
                    team.version += 1;

                    app.world.planets.insert(asteroid.id, asteroid);
                    app.world.teams.insert(team.id, team);
                }
                _ => return Err(anyhow!("Invalid team location when accepting asteroid.")),
            }
            app.world.dirty = true;
            app.world.dirty_network = true;
            app.world.dirty_ui = true;

            app.ui.close_popup();

            Ok(None)
        })
    }

    fn set_upgrade_spaceship(upgrade: SpaceshipUpgrade) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut team = app.world.get_own_team()?.clone();
            team.can_set_upgrade_spaceship(upgrade.clone())?;

            for (resource, amount) in upgrade.cost.clone() {
                team.remove_resource(resource, amount)?;
            }

            team.spaceship.pending_upgrade = Some(upgrade.clone());
            app.world.teams.insert(team.id, team);

            app.world.dirty = true;
            app.world.dirty_network = true;
            app.world.dirty_ui = true;

            Ok(None)
        })
    }

    fn upgrade_spaceship(upgrade: SpaceshipUpgrade) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut team = app.world.get_own_team()?.clone();
            if let Some(u_hull) = upgrade.hull {
                team.spaceship.hull = u_hull;
            }
            if let Some(u_engine) = upgrade.engine {
                team.spaceship.engine = u_engine;
            }
            if let Some(u_storage) = upgrade.storage {
                team.spaceship.storage = u_storage;
            }
            team.spaceship.pending_upgrade = None;

            app.world.teams.insert(team.id, team);

            app.ui.push_popup(PopupMessage::Ok(
                "Spaceship upgrade completed!".into(),
                true,
                Tick::now(),
            ));

            app.world.dirty = true;
            app.world.dirty_network = true;
            app.world.dirty_ui = true;

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
            UiCallbackPreset::GoToGame { game_id } => Self::go_to_game(*game_id)(app),
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
            UiCallbackPreset::TradeResource {
                resource,
                amount,
                unit_cost,
            } => Self::trade_resource(*resource, *amount, *unit_cost)(app),
            UiCallbackPreset::SetTeamColors { color, channel } => {
                app.ui
                    .new_team_screen
                    .set_team_colors(color.clone(), channel.clone());
                Ok(None)
            }
            UiCallbackPreset::SetTeamTactic { tactic } => {
                let own_team = app.world.get_own_team()?;
                let mut team = own_team.clone();
                team.game_tactic = tactic.clone();
                app.world.teams.insert(team.id, team);
                app.world.dirty = true;
                app.world.dirty_ui = true;
                app.world.dirty_network = true;
                Ok(None)
            }
            UiCallbackPreset::SetNextTeamTactic => {
                let own_team = app.world.get_own_team()?;
                let mut team = own_team.clone();
                team.game_tactic = team.game_tactic.next();
                app.world.teams.insert(team.id, team);
                app.world.dirty = true;
                app.world.dirty_ui = true;
                app.world.dirty_network = true;
                Ok(None)
            }
            UiCallbackPreset::TogglePitchView => {
                app.ui.game_panel.toggle_pitch_view();
                Ok(None)
            }
            UiCallbackPreset::TogglePlayerStatusView => {
                app.ui.game_panel.toggle_player_status_view();
                Ok(None)
            }
            UiCallbackPreset::ChallengeTeam { team_id } => Self::challenge_team(*team_id)(app),
            UiCallbackPreset::AcceptChallenge { challenge } => {
                app.network_handler
                    .as_mut()
                    .ok_or(anyhow!("Network handler is not initialized"))?
                    .accept_challenge(&&app.world, challenge.clone())?;

                let own_team = app.world.get_own_team_mut()?;
                own_team.remove_challenge(
                    challenge.home_team_in_game.team_id,
                    challenge.away_team_in_game.team_id,
                );
                Ok(None)
            }
            UiCallbackPreset::DeclineChallenge { challenge } => {
                app.network_handler
                    .as_mut()
                    .ok_or(anyhow!("Network handler is not initialized"))?
                    .decline_challenge(challenge.clone())?;
                let own_team = app.world.get_own_team_mut()?;
                own_team.remove_challenge(
                    challenge.home_team_in_game.team_id,
                    challenge.away_team_in_game.team_id,
                );
                Ok(None)
            }
            UiCallbackPreset::CreateTradeProposal {
                proposer_player_id,
                target_player_id,
            } => Self::trade_players(*proposer_player_id, *target_player_id)(app),
            UiCallbackPreset::AcceptTrade { trade } => {
                app.network_handler
                    .as_mut()
                    .ok_or(anyhow!("Network handler is not initialized"))?
                    .accept_trade(&&app.world, trade.clone())?;

                let own_team = app.world.get_own_team_mut()?;
                own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);
                Ok(None)
            }
            UiCallbackPreset::DeclineTrade { trade } => {
                app.network_handler
                    .as_mut()
                    .ok_or(anyhow!("Network handler is not initialized"))?
                    .decline_trade(trade.clone())?;
                let own_team = app.world.get_own_team_mut()?;
                own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);
                Ok(None)
            }
            UiCallbackPreset::GoToTrade { trade } => Self::go_to_trade(trade.clone())(app),
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
                app.toggle_audio_player()?;
                Ok(None)
            }
            UiCallbackPreset::NextAudioSample => {
                app.next_sample_audio_player()?;
                Ok(None)
            }
            UiCallbackPreset::SetSwarmPanelView { topic } => {
                app.ui.swarm_panel.set_view(*topic);
                Ok(None)
            }
            UiCallbackPreset::SetMyTeamPanelView { view } => {
                app.ui.my_team_panel.set_view(*view);
                Ok(None)
            }
            UiCallbackPreset::SetPlayerPanelView { view } => {
                app.ui.player_panel.set_view(*view);
                Ok(None)
            }
            UiCallbackPreset::SetTeamPanelView { view } => {
                app.ui.team_panel.set_view(*view);
                Ok(None)
            }
            UiCallbackPreset::HirePlayer { player_id } => {
                app.world
                    .hire_player_for_team(*player_id, app.world.own_team_id)?;

                Ok(None)
            }
            UiCallbackPreset::PromptReleasePlayer { player_id } => {
                let player = app.world.get_player_or_err(*player_id)?;
                let player_name = format!("{} {}", player.info.first_name, player.info.last_name);
                app.ui.push_popup(PopupMessage::ReleasePlayer(
                    player_name,
                    *player_id,
                    Tick::now(),
                ));
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

            UiCallbackPreset::Drink { player_id } => {
                let mut player = app.world.get_player_or_err(*player_id)?.clone();
                player.can_drink(&app.world)?;

                let morale_bonus = if matches!(player.special_trait, Some(Trait::Spugna)) {
                    MAX_SKILL
                } else {
                    MORALE_DRINK_BONUS
                };

                player.add_morale(morale_bonus);
                player.add_tiredness(TIREDNESS_DRINK_MALUS);

                let mut team = app
                    .world
                    .get_team_or_err(player.team.expect("Player should have team"))?
                    .clone();

                team.remove_resource(Resource::RUM, 1)?;

                //If player is a spugna and pilot and team is travelling or exploring and player was already maxxed in morale,
                // there is a chance that the player enters a portal to a random planet.
                let rng = &mut ChaCha8Rng::from_entropy();
                if matches!(player.special_trait, Some(Trait::Spugna))
                    && player.info.crew_role == CrewRole::Pilot
                    && rng.gen_bool(PORTAL_DISCOVERY_PROBABILITY)
                {
                    let portal_target_id = match team.current_location {
                        TeamLocation::OnPlanet { .. } => None,
                        TeamLocation::Travelling { from, to, .. } => app
                            .world
                            .planets
                            .iter()
                            .filter(|(&id, p)| {
                                id != from
                                    && id != to
                                    && p.total_population() > 0
                                    && p.peer_id.is_none()
                            })
                            .choose(rng)
                            .map(|(&id, _)| id.clone()),

                        TeamLocation::Exploring { around, .. } => app
                            .world
                            .planets
                            .iter()
                            .filter(|(&id, p)| {
                                id != around && p.total_population() > 0 && p.peer_id.is_none()
                            })
                            .choose(rng)
                            .map(|(&id, _)| id.clone()),
                    };
                    if let Some(to) = portal_target_id {
                        let portal_target = app.world.get_planet_or_err(to)?;
                        // We set the new target to the portal_target
                        let from = match team.current_location {
                            TeamLocation::OnPlanet { .. } => {
                                panic!("Should not get into this branch")
                            }
                            TeamLocation::Travelling { from, .. } => from,
                            TeamLocation::Exploring { around, .. } => around,
                        };

                        let distance = app.world.distance_between_planets(from, to)?;
                        // Notice that the team will arrive when  world.current_timestamp > started + duration.
                        team.current_location = TeamLocation::Travelling {
                            from,
                            to,
                            started: Tick::now(),
                            duration: 10 * SECONDS,
                            distance,
                        };

                        app.ui.push_popup(PopupMessage::PortalFound(
                            player.info.shortened_name(),
                            portal_target.name.clone(),
                            Tick::now(),
                        ));
                    }
                }

                app.world.players.insert(player_id.clone(), player);
                app.world.teams.insert(team.id, team);
                app.world.dirty_network = true;
                app.world.dirty_ui = true;
                app.world.dirty = true;

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
            } => Self::generate_own_team(
                name.clone(),
                *home_planet,
                *jersey_style,
                *jersey_colors,
                players.clone(),
                *balance,
                spaceship.clone(),
            )(app),
            UiCallbackPreset::CancelGeneratePlayerTeam => Self::cancel_generate_own_team()(app),
            UiCallbackPreset::AssignBestTeamPositions => Self::assign_best_team_positions()(app),
            UiCallbackPreset::SwapPlayerPositions {
                player_id,
                position,
            } => Self::swap_player_positions(*player_id, *position)(app),
            UiCallbackPreset::NextTrainingFocus { team_id } => {
                Self::next_training_focus(*team_id)(app)
            }
            UiCallbackPreset::TravelToPlanet { planet_id } => {
                Self::travel_to_planet(*planet_id)(app)
            }
            UiCallbackPreset::ExploreAroundPlanet { duration } => {
                Self::explore_around_planet(duration.clone())(app)
            }
            UiCallbackPreset::ZoomInToPlanet { planet_id } => {
                Self::zoom_in_to_planet(*planet_id)(app)
            }
            UiCallbackPreset::Dial { address } => Self::dial(address.clone())(app),
            UiCallbackPreset::Sync => Self::sync()(app),
            UiCallbackPreset::SendMessage { message } => Self::send(message.clone())(app),
            UiCallbackPreset::PushUiPopup { popup_message } => {
                app.ui.push_popup(popup_message.clone());
                Ok(None)
            }
            UiCallbackPreset::NameAndAcceptAsteroid { name, filename } => {
                Self::name_and_accept_asteroid(name.clone(), filename.clone())(app)
            }
            UiCallbackPreset::SetUpgradeSpaceship { upgrade } => {
                Self::set_upgrade_spaceship(upgrade.clone())(app)
            }
            UiCallbackPreset::UpgradeSpaceship { upgrade } => {
                Self::upgrade_spaceship(upgrade.clone())(app)
            }
            UiCallbackPreset::StartSpaceAdventure => {
                app.ui.set_state(UiState::SpaceAdventure);
                let spaceship = &app.world.get_own_team()?.spaceship;
                let space = Space::new()?.with_spaceship(spaceship)?;
                app.world.space = Some(space);
                Ok(None)
            }
            UiCallbackPreset::StopSpaceAdventure => {
                app.ui.set_state(UiState::Main);
                app.world.space = None;
                Ok(None)
            }
            UiCallbackPreset::MovePlayerLeft => {
                if let Some(space) = app.world.space.as_mut() {
                    space.player_mut().set_accelleration([-1.0, 0.0]);
                }

                Ok(None)
            }
            UiCallbackPreset::MovePlayerRight => {
                if let Some(space) = app.world.space.as_mut() {
                    space.player_mut().set_accelleration([1.0, 0.0]);
                }

                Ok(None)
            }
            UiCallbackPreset::MovePlayerDown => {
                if let Some(space) = app.world.space.as_mut() {
                    space.player_mut().set_accelleration([0.0, 1.0]);
                }

                Ok(None)
            }
            UiCallbackPreset::MovePlayerUp => {
                if let Some(space) = app.world.space.as_mut() {
                    space.player_mut().set_accelleration([0.0, -1.0]);
                }

                Ok(None)
            }
        }
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct CallbackRegistry {
    mouse_callbacks: HashMap<MouseEventKind, HashMap<Option<Rect>, UiCallbackPreset>>,
    keyboard_callbacks: HashMap<KeyCode, UiCallbackPreset>,
    hovering: (u16, u16),
    max_layer: u8,
}

impl CallbackRegistry {
    fn contains(rect: &Rect, x: u16, y: u16) -> bool {
        rect.x <= x && x < rect.x + rect.width && rect.y <= y && y < rect.y + rect.height
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_max_layer(&mut self, layer: u8) {
        self.max_layer = layer;
    }

    pub fn get_max_layer(&mut self) -> u8 {
        self.max_layer
    }

    pub fn register_mouse_callback(
        &mut self,
        event_kind: MouseEventKind,
        rect: Option<Rect>,
        callback: UiCallbackPreset,
    ) {
        self.mouse_callbacks
            .entry(event_kind)
            .or_insert_with(HashMap::new)
            .insert(rect, callback);
    }

    pub fn register_keyboard_callback(&mut self, key_code: KeyCode, callback: UiCallbackPreset) {
        self.keyboard_callbacks.insert(key_code, callback);
    }

    pub fn clear(&mut self) {
        self.mouse_callbacks.clear();
        self.keyboard_callbacks.clear();
        self.max_layer = 0;
    }

    pub fn is_hovering(&self, rect: Rect) -> bool {
        Self::contains(&rect, self.hovering.0, self.hovering.1)
    }

    pub fn set_hovering(&mut self, event: MouseEvent) {
        self.hovering = (event.column, event.row);
    }

    pub fn handle_mouse_event(&self, event: &MouseEvent) -> Option<UiCallbackPreset> {
        if let Some(mouse_callbacks) = self.mouse_callbacks.get(&event.kind) {
            for (rect, callback) in mouse_callbacks.iter() {
                if let Some(r) = rect {
                    if Self::contains(r, event.column, event.row) {
                        return Some(callback.clone());
                    }
                } else {
                    // Callbacks with no rect are global callbacks.
                    return Some(callback.clone());
                }
            }
        }
        None
    }

    pub fn handle_keyboard_event(&self, key_code: &KeyCode) -> Option<UiCallbackPreset> {
        self.keyboard_callbacks.get(key_code).cloned()
    }
}
