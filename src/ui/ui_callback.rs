use super::swarm_panel::SwarmView;
use super::{
    galaxy_panel::ZoomLevel,
    my_team_panel::MyTeamView,
    new_team_screen::CreationState,
    player_panel::PlayerView,
    popup_message::PopupMessage,
    team_panel::TeamView,
    traits::{Screen, SplitPanel},
    ui::{UiState, UiTab},
};

use crate::network::{challenge::Challenge, trade::Trade};
use crate::{
    app::App,
    game_engine::{tactic::Tactic, types::TeamInGame},
    image::color_map::{ColorMap, ColorPreset},
    space_adventure::{ControllableSpaceship, PlayerInput, SpaceAdventure},
    types::{
        AppCallback, AppResult, GameId, PlanetId, PlayerId, ResourceMap, StorableResourceMap,
        SystemTimeTick, TeamId, Tick,
    },
    world::{
        constants::*,
        jersey::{Jersey, JerseyStyle},
        planet::AsteroidUpgrade,
        player::Trait,
        resources::Resource,
        role::CrewRole,
        skill::{GameSkill, MAX_SKILL},
        spaceship::{Spaceship, SpaceshipUpgrade, SpaceshipUpgradeTarget},
        team::Team,
        types::{PlayerLocation, TeamBonus, TeamLocation, TrainingFocus},
    },
};
use anyhow::anyhow;
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind};
use log::info;
use rand::{seq::IteratorRandom, Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ratatui::layout::Rect;
use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum UiCallback {
    #[default]
    None,
    PromptQuit,
    PushTutorialPage {
        index: usize,
    },
    ToggleUiDebugMode,
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
    GoToPlanet {
        planet_id: PlanetId,
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
    #[cfg(feature = "audio")]
    ToggleAudio,
    #[cfg(feature = "audio")]
    PreviousRadio,
    #[cfg(feature = "audio")]
    NextRadio,

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
    PromptAbandonAsteroid {
        asteroid_id: PlanetId,
    },
    ConfirmAbandonAsteroid {
        asteroid_id: PlanetId,
    },
    HirePlayer {
        player_id: PlayerId,
    },
    PromptReleasePlayer {
        player_id: PlayerId,
    },
    ConfirmReleasePlayer {
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
    TogglePlayerWidgetView,
    NextTrainingFocus {
        team_id: TeamId,
    },
    TravelToPlanet {
        planet_id: PlanetId,
    },
    ExploreAroundPlanet {
        duration: Tick,
    },
    ZoomToPlanet {
        planet_id: PlanetId,
        zoom_level: ZoomLevel,
    },
    Ping,
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
    SetSpaceshipUpgrade {
        upgrade: SpaceshipUpgrade,
    },
    UpgradeSpaceship {
        upgrade: SpaceshipUpgrade,
    },
    SetAsteroidUpgrade {
        asteroid_id: PlanetId,
        upgrade: AsteroidUpgrade,
    },
    UpgradeAsteroid {
        asteroid_id: PlanetId,
        upgrade: AsteroidUpgrade,
    },
    StartSpaceAdventure,
    StopSpaceAdventure,
    ReturnFromSpaceAdventure,
    SpaceMovePlayerLeft,
    SpaceMovePlayerRight,
    SpaceMovePlayerDown,
    SpaceMovePlayerUp,
    SpaceToggleAutofire,
    SpaceShoot,
    SpaceReleaseScraps,
    ToggleTeamAutonomousStrategyForLocalChallenges,
    ToggleTeamAutonomousStrategyForNetworkChallenges,
}

impl UiCallback {
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
                app.ui.switch_to(super::ui::UiTab::Crews);
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
                app.ui.switch_to(super::ui::UiTab::Pirates);
            }

            Ok(None)
        })
    }

    fn go_to_trade(trade: Trade) -> AppCallback {
        Box::new(move |app: &mut App| {
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
                app.ui.switch_to(super::ui::UiTab::Pirates);
            }

            Ok(None)
        })
    }

    fn go_to_player_team(player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team_id = app
                .world
                .get_player_or_err(&player_id)?
                .team
                .ok_or(anyhow!("Player {:?} has no team", player_id))?;

            app.ui.team_panel.reset_view();

            if let Some(index) = app
                .ui
                .team_panel
                .all_teams
                .iter()
                .position(|&x| x == team_id)
            {
                app.ui.team_panel.set_index(index);
                let player_index = app
                    .world
                    .get_team_or_err(&team_id)?
                    .player_ids
                    .iter()
                    .position(|&x| x == player_id)
                    .unwrap_or_default();
                app.ui.team_panel.player_index = player_index;
                app.ui.switch_to(super::ui::UiTab::Crews);
            }

            Ok(None)
        })
    }

    fn go_to_game(game_id: GameId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.game_panel.set_active_game(game_id)?;
            app.ui.switch_to(super::ui::UiTab::Games);
            Ok(None)
        })
    }

    fn go_to_planet(planet_id: PlanetId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.galaxy_panel.go_to_planet(planet_id, ZoomLevel::In);
            app.ui.switch_to(super::ui::UiTab::Galaxy);

            Ok(None)
        })
    }

    fn go_to_home_planet(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team = app.world.get_team_or_err(&team_id)?;
            app.ui
                .galaxy_panel
                .go_to_planet(team.home_planet_id, ZoomLevel::In);
            app.ui.switch_to(super::ui::UiTab::Galaxy);

            Ok(None)
        })
    }

    fn go_to_current_team_planet(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team = app.world.get_team_or_err(&team_id)?;

            let target = match team.current_location {
                TeamLocation::OnPlanet {
                    planet_id: current_planet_id,
                } => app.world.get_planet_or_err(&current_planet_id)?,
                TeamLocation::Travelling { .. } => {
                    return Err(anyhow!("Team is travelling"));
                }
                TeamLocation::Exploring { .. } => {
                    return Err(anyhow!("Team is exploring"));
                }
                TeamLocation::OnSpaceAdventure { .. } => {
                    return Err(anyhow!("Team is on a space adventure"))
                }
            };

            app.ui.galaxy_panel.go_to_planet(target.id, ZoomLevel::In);
            app.ui.switch_to(super::ui::UiTab::Galaxy);

            Ok(None)
        })
    }

    fn go_to_current_player_planet(player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let player = app.world.get_player_or_err(&player_id)?;

            match player.current_location {
                PlayerLocation::OnPlanet {
                    planet_id: current_planet_id,
                } => {
                    let target = app.world.get_planet_or_err(&current_planet_id)?;
                    app.ui.galaxy_panel.go_to_planet(target.id, ZoomLevel::In);
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
            app.ui.galaxy_panel.go_to_planet(planet_id, ZoomLevel::In);
            app.ui.switch_to(super::ui::UiTab::Galaxy);
            Ok(None)
        })
    }

    fn go_to_planet_zoom_out(planet_id: PlanetId) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.ui.galaxy_panel.go_to_planet(planet_id, ZoomLevel::Out);
            app.ui.switch_to(super::ui::UiTab::Galaxy);
            Ok(None)
        })
    }

    fn trade_resource(resource: Resource, amount: i32, unit_cost: u32) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut own_team = app.world.get_own_team()?.clone();
            if amount > 0 {
                own_team.add_resource(resource, amount as u32)?;
                own_team.sub_resource(Resource::SATOSHI, unit_cost * amount as u32)?;
            } else if amount < 0 {
                own_team.sub_resource(resource, (-amount) as u32)?;
                own_team.add_resource(Resource::SATOSHI, unit_cost * (-amount) as u32)?;
            }
            app.world.teams.insert(own_team.id, own_team);
            app.world.dirty = true;
            app.world.dirty_ui = true;
            Ok(None)
        })
    }

    fn zoom_to_planet(planet_id: PlanetId, zoom_level: ZoomLevel) -> AppCallback {
        Box::new(move |app: &mut App| {
            let panel = &mut app.ui.galaxy_panel;
            panel.set_zoom_level(zoom_level);
            panel.set_planet_index(0);
            panel.set_planet_id(planet_id);

            Ok(None)
        })
    }

    fn challenge_team(team_id: TeamId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let own_team = app.world.get_own_team()?;
            let team = app.world.get_team_or_err(&team_id)?;

            // Challenge to network team.
            if let Some(peer_id) = team.peer_id {
                own_team.can_challenge_network_team(team)?;
                let challenge = app.network_handler.send_new_challenge(
                    &app.world,
                    peer_id,
                    team.id,
                    app.app_version(),
                )?;

                let own_team = app.world.get_own_team_mut()?;
                own_team.add_sent_challenge(challenge);

                return Ok(Some("Challenge sent".to_string()));
            }

            // Else, challenge to local team.
            own_team.can_challenge_local_team(team)?;
            // If other team is local, reject challenge if team is too tired
            let average_tiredness = team.average_tiredness(&app.world);
            if average_tiredness > MAX_AVG_TIREDNESS_PER_CHALLENGED_GAME {
                return Err(anyhow!("{} is too tired", team.name));
            }
            let own_team_id = app.world.own_team_id;
            let (home_team_in_game, away_team_in_game) = match ChaCha8Rng::from_os_rng()
                .random_range(0..=1)
            {
                0 => (
                    TeamInGame::from_team_id(&own_team_id, &app.world.teams, &app.world.players)
                        .ok_or(anyhow!("Own team {:?} not found", own_team_id))?,
                    TeamInGame::from_team_id(&team_id, &app.world.teams, &app.world.players)
                        .ok_or(anyhow!("Team {:?} not found", team_id))?,
                ),

                1 => (
                    TeamInGame::from_team_id(&team_id, &app.world.teams, &app.world.players)
                        .ok_or(anyhow!("Team {:?} not found", team_id))?,
                    TeamInGame::from_team_id(&own_team_id, &app.world.teams, &app.world.players)
                        .ok_or(anyhow!("Own team {:?} not found", own_team_id))?,
                ),
                _ => unreachable!(),
            };

            let game_id = app
                .world
                .generate_local_game(home_team_in_game, away_team_in_game)?;

            app.ui.game_panel.update(&app.world)?;
            app.ui.game_panel.set_active_game(game_id)?;
            app.ui.switch_to(super::ui::UiTab::Games);
            return Ok(Some("Challenge accepted".to_string()));
        })
    }

    fn trade_players(proposer_player_id: PlayerId, target_player_id: PlayerId) -> AppCallback {
        Box::new(move |app: &mut App| {
            let own_team = app.world.get_own_team()?;

            let target_player = app.world.get_player_or_err(&target_player_id)?;
            let target_team = if let Some(team_id) = target_player.team {
                app.world.get_team_or_err(&team_id)?
            } else {
                return Err(anyhow!("Target player has no team"));
            };

            let proposer_player = app.world.get_player_or_err(&proposer_player_id)?;
            own_team.can_trade_players(proposer_player, target_player, target_team)?;

            // Network trade
            if let Some(peer_id) = target_team.peer_id {
                let trade = app.network_handler.send_new_trade(
                    &app.world,
                    peer_id,
                    proposer_player_id,
                    target_player_id,
                )?;
                let own_team = app.world.get_own_team_mut()?;
                own_team.add_sent_trade(trade);
                return Ok(Some("Trade offer sent".to_string()));
            }

            // Local trade
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
        spaceship: Spaceship,
    ) -> AppCallback {
        Box::new(move |app: &mut App| {
            app.world.generate_own_team(
                name.clone(),
                home_planet,
                jersey_style,
                jersey_colors,
                players.clone(),
                spaceship.clone(),
            )?;
            app.ui.set_state(UiState::Main);
            app.ui.push_popup(PopupMessage::Tutorial {
                index: 0,
                tick: Tick::now(),
            });
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
            let mut team = app.world.get_team_or_err(&team_id)?.clone();
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
            let target_planet = app.world.get_planet_or_err(&planet_id)?;

            let mut current_planet = match own_team.current_location {
                TeamLocation::OnPlanet {
                    planet_id: current_planet_id,
                } => {
                    if current_planet_id == planet_id {
                        return Err(anyhow!("Already on planet"));
                    }
                    app.world.get_planet_or_err(&current_planet_id)?.clone()
                }
                TeamLocation::Travelling { .. } => return Err(anyhow!("Team is travelling")),
                TeamLocation::Exploring { .. } => return Err(anyhow!("Team is exploring")),
                TeamLocation::OnSpaceAdventure { .. } => {
                    return Err(anyhow!("Team is on a space adventure"))
                }
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

            let is_teleporting = if duration <= TELEPORT_MAX_DURATION {
                true
            } else {
                false
            };

            if is_teleporting {
                let rum_consumed = own_team.player_ids.len() as u32;
                own_team.sub_resource(Resource::RUM, rum_consumed)?;
            } else {
                // For simplicity we just subtract the fuel upfront, maybe would be nicer on UI to
                // show the fuel consumption as the team travels in world.tick_travel,
                // but this would require more operations and checks in the tick function.
                // FIXME: centralize fuel cost calculation
                let fuel_consumed = app
                    .world
                    .fuel_consumption_to_planet(own_team.id, planet_id)?;
                own_team.sub_resource(Resource::FUEL, fuel_consumed)?;
            }

            info!(
                "Team {:?} is travelling from {:?} to {:?}, consuming {:.2} fuel",
                own_team.id,
                current_planet.id,
                target_planet.id,
                duration as f32 * own_team.spaceship_fuel_consumption_per_tick()
            );

            current_planet.team_ids.retain(|&x| x != own_team.id);
            app.world.planets.insert(current_planet.id, current_planet);

            let pirate_jersey = Jersey {
                style: JerseyStyle::Pirate,
                color: own_team.jersey.color.clone(),
            };

            for player in own_team.player_ids.iter() {
                let mut player = app.world.get_player_or_err(player)?.clone();
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
                TeamLocation::OnSpaceAdventure { .. } => {
                    return Err(anyhow!("Team is on a space adventure"))
                }
            };

            let mut around_planet = app.world.get_planet_or_err(&planet_id)?.clone();
            own_team.can_explore_around_planet(&around_planet, duration)?;

            own_team.current_location = TeamLocation::Exploring {
                around: planet_id,
                started: Tick::now(),
                duration,
            };

            // For simplicity we just subtract the fuel upfront, maybe would be nicer on UI to
            // show the fuel consumption as the team travels in world.tick_travel,
            // but this would require more operations and checks in the tick function.
            own_team.sub_resource(
                Resource::FUEL,
                (duration as f32 * own_team.spaceship_fuel_consumption_per_tick()).max(1.0) as u32,
            )?;

            around_planet.team_ids.retain(|&x| x != own_team.id);
            app.world.planets.insert(around_planet.id, around_planet);

            let pirate_jersey = Jersey {
                style: JerseyStyle::Pirate,
                color: own_team.jersey.color.clone(),
            };

            for player_id in own_team.player_ids.iter() {
                let mut player = app.world.get_player_or_err(player_id)?.clone();
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

    fn ping() -> AppCallback {
        Box::new(move |app: &mut App| {
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
            app.network_handler.send_message(message.clone())?;

            Ok(None)
        })
    }

    fn name_and_accept_asteroid(name: String, filename: String) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut own_team = app.world.get_own_team()?.clone();
            if own_team.asteroid_ids.len() > MAX_NUM_ASTEROID_PER_TEAM {
                return Err(anyhow!("Team has reached max number of asteroids."));
            }

            match own_team.current_location {
                TeamLocation::OnPlanet { planet_id } => {
                    let asteroid_id = app.world.generate_team_asteroid(
                        name.clone(),
                        filename.clone(),
                        planet_id,
                    )?;
                    let mut current_planet = app.world.get_planet_or_err(&planet_id)?.clone();
                    current_planet.team_ids.retain(|&x| x != own_team.id);
                    app.world.planets.insert(current_planet.id, current_planet);

                    own_team.current_location = TeamLocation::OnPlanet {
                        planet_id: asteroid_id,
                    };

                    let mut asteroid = app.world.get_planet_or_err(&asteroid_id)?.clone();
                    asteroid.team_ids.push(own_team.id);
                    asteroid.version += 1;

                    own_team.asteroid_ids.push(asteroid_id);
                    own_team.version += 1;

                    app.world.planets.insert(asteroid.id, asteroid);
                    app.world.teams.insert(own_team.id, own_team);
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

    fn set_spaceship_upgrade(upgrade: SpaceshipUpgrade) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut team = app.world.get_own_team()?.clone();
            team.can_upgrade_spaceship(&upgrade)?;

            for (resource, amount) in &upgrade.cost() {
                team.sub_resource(*resource, *amount)?;
            }

            team.spaceship.pending_upgrade = Some(upgrade);
            app.world.teams.insert(team.id, team);

            app.world.dirty = true;
            app.world.dirty_network = true;
            app.world.dirty_ui = true;

            Ok(None)
        })
    }

    fn upgrade_spaceship(upgrade: SpaceshipUpgrade) -> AppCallback {
        Box::new(move |app: &mut App| {
            let team = app.world.get_own_team_mut()?;

            match upgrade.target {
                SpaceshipUpgradeTarget::Hull { component } => team.spaceship.hull = component,
                SpaceshipUpgradeTarget::Engine { component } => team.spaceship.engine = component,
                SpaceshipUpgradeTarget::Storage { component } => team.spaceship.storage = component,
                SpaceshipUpgradeTarget::Shooter { component } => team.spaceship.shooter = component,
                SpaceshipUpgradeTarget::Repairs { .. } => {}
            };

            // In any case, fully repair ship.
            team.spaceship.reset_durability();
            team.spaceship.pending_upgrade = None;

            let message = match upgrade.target {
                SpaceshipUpgradeTarget::Repairs { .. } => {
                    "Spaceship repairs completed!".to_string()
                }
                _ => "Spaceship upgrade completed!".to_string(),
            };

            app.ui.push_popup(PopupMessage::Ok {
                message,
                is_skippable: true,
                tick: Tick::now(),
            });

            app.world.dirty = true;
            app.world.dirty_network = true;
            app.world.dirty_ui = true;

            Ok(None)
        })
    }

    fn set_asteroid_upgrade(asteroid_id: PlanetId, upgrade: AsteroidUpgrade) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut team = app.world.get_own_team()?.clone();
            let asteroid = app.world.get_planet_or_err(&asteroid_id)?;
            team.can_upgrade_asteroid(asteroid, &upgrade)?;

            for (resource, amount) in &upgrade.cost() {
                team.sub_resource(*resource, *amount)?;
            }
            app.world.teams.insert(team.id, team);

            let mut asteroid = app.world.get_planet_or_err(&asteroid_id)?.clone();
            asteroid.pending_upgrade = Some(upgrade);
            app.world.planets.insert(asteroid.id, asteroid);

            app.world.dirty = true;
            app.world.dirty_network = true;
            app.world.dirty_ui = true;

            Ok(None)
        })
    }

    fn upgrade_asteroid(asteroid_id: PlanetId, upgrade: AsteroidUpgrade) -> AppCallback {
        Box::new(move |app: &mut App| {
            let mut asteroid = app.world.get_planet_or_err(&asteroid_id)?.clone();
            asteroid.pending_upgrade = None;
            asteroid.version += 1;
            asteroid.upgrades.push(upgrade.target);

            let message = format!(
                "{} construction on {} completed!",
                upgrade.target.name(),
                asteroid.name
            );
            app.world.planets.insert(asteroid.id, asteroid);

            app.ui.push_popup(PopupMessage::Ok {
                message,
                is_skippable: true,
                tick: Tick::now(),
            });

            app.world.dirty = true;
            app.world.dirty_ui = true;

            Ok(None)
        })
    }

    pub fn call(&self, app: &mut App) -> AppResult<Option<String>> {
        match self {
            UiCallback::None => Ok(None),
            UiCallback::PromptQuit => {
                if app.ui.state == UiState::Splash {
                    app.quit()?;
                    return Ok(None);
                }
                let during_space_adventure = app.world.space_adventure.is_some();
                app.ui.push_popup(PopupMessage::PromptQuit {
                    during_space_adventure,
                    tick: Tick::now(),
                });

                Ok(None)
            }
            UiCallback::PushTutorialPage { index } => {
                app.ui.close_popup();
                app.ui.push_popup_to_top(PopupMessage::Tutorial {
                    index: *index,
                    tick: Tick::now(),
                });
                Ok(None)
            }
            UiCallback::ToggleUiDebugMode => {
                app.ui.toggle_data_view();
                Ok(None)
            }
            UiCallback::SetPanelIndex { index } => {
                if let Some(panel) = app.ui.get_active_panel() {
                    panel.set_index(*index);
                }
                Ok(None)
            }
            UiCallback::GoToTeam { team_id } => Self::go_to_team(*team_id)(app),
            UiCallback::GoToPlayer { player_id } => Self::go_to_player(*player_id)(app),
            UiCallback::GoToPlayerTeam { player_id } => Self::go_to_player_team(*player_id)(app),
            UiCallback::GoToGame { game_id } => Self::go_to_game(*game_id)(app),
            UiCallback::GoToPlanet { planet_id } => Self::go_to_planet(*planet_id)(app),
            UiCallback::GoToHomePlanet { team_id } => Self::go_to_home_planet(*team_id)(app),
            UiCallback::GoToCurrentTeamPlanet { team_id } => {
                Self::go_to_current_team_planet(*team_id)(app)
            }
            UiCallback::GoToCurrentPlayerPlanet { player_id } => {
                Self::go_to_current_player_planet(*player_id)(app)
            }

            UiCallback::GoToPlanetZoomIn { planet_id } => {
                Self::go_to_planet_zoom_in(*planet_id)(app)
            }
            UiCallback::GoToPlanetZoomOut { planet_id } => {
                Self::go_to_planet_zoom_out(*planet_id)(app)
            }
            UiCallback::TradeResource {
                resource,
                amount,
                unit_cost,
            } => Self::trade_resource(*resource, *amount, *unit_cost)(app),
            UiCallback::SetTeamColors { color, channel } => {
                app.ui.new_team_screen.set_team_colors(*color, *channel);
                Ok(None)
            }
            UiCallback::SetTeamTactic { tactic } => {
                let own_team = app.world.get_own_team()?;
                let mut team = own_team.clone();
                team.game_tactic = tactic.clone();
                app.world.teams.insert(team.id, team);
                app.world.dirty = true;
                app.world.dirty_ui = true;
                app.world.dirty_network = true;
                Ok(None)
            }
            UiCallback::SetNextTeamTactic => {
                let own_team = app.world.get_own_team()?;
                let mut team = own_team.clone();
                team.game_tactic = team.game_tactic.next();
                app.world.teams.insert(team.id, team);
                app.world.dirty = true;
                app.world.dirty_ui = true;
                app.world.dirty_network = true;
                Ok(None)
            }
            UiCallback::TogglePitchView => {
                app.ui.game_panel.toggle_pitch_view();
                Ok(None)
            }
            UiCallback::TogglePlayerStatusView => {
                app.ui.game_panel.toggle_player_status_view();
                Ok(None)
            }
            UiCallback::TogglePlayerWidgetView => {
                app.ui.player_panel.toggle_player_widget_view();
                Ok(None)
            }
            UiCallback::ChallengeTeam { team_id } => Self::challenge_team(*team_id)(app),
            UiCallback::AcceptChallenge { challenge } => {
                if let Err(e) = app
                    .network_handler
                    .accept_challenge(&app.world, challenge.clone())
                {
                    let own_team = app.world.get_own_team_mut()?;
                    own_team.remove_challenge(
                        challenge.home_team_in_game.team_id,
                        challenge.away_team_in_game.team_id,
                    );
                    return Err(e);
                }

                let own_team = app.world.get_own_team_mut()?;
                own_team.remove_challenge(
                    challenge.home_team_in_game.team_id,
                    challenge.away_team_in_game.team_id,
                );

                Ok(None)
            }
            UiCallback::DeclineChallenge { challenge } => {
                app.network_handler.decline_challenge(challenge.clone())?;
                let own_team = app.world.get_own_team_mut()?;
                own_team.remove_challenge(
                    challenge.home_team_in_game.team_id,
                    challenge.away_team_in_game.team_id,
                );
                Ok(None)
            }
            UiCallback::CreateTradeProposal {
                proposer_player_id,
                target_player_id,
            } => Self::trade_players(*proposer_player_id, *target_player_id)(app),
            UiCallback::AcceptTrade { trade } => {
                if let Err(e) = app.network_handler.accept_trade(&&app.world, trade.clone()) {
                    let own_team = app.world.get_own_team_mut()?;
                    own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);
                    return Err(e);
                }

                let own_team = app.world.get_own_team_mut()?;
                own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);
                Ok(None)
            }
            UiCallback::DeclineTrade { trade } => {
                app.network_handler.decline_trade(trade.clone())?;
                let own_team = app.world.get_own_team_mut()?;
                own_team.remove_trade(trade.proposer_player.id, trade.target_player.id);
                Ok(None)
            }
            UiCallback::GoToTrade { trade } => Self::go_to_trade(trade.clone())(app),
            UiCallback::NextUiTab => Self::next_ui_tab()(app),
            UiCallback::PreviousUiTab => Self::previous_ui_tab()(app),
            UiCallback::SetUiTab { ui_tab } => Self::set_ui_tab(*ui_tab)(app),
            UiCallback::NextPanelIndex => Self::next_panel_index()(app),
            UiCallback::PreviousPanelIndex => Self::previous_panel_index()(app),
            UiCallback::CloseUiPopup => {
                app.ui.close_popup();
                Ok(None)
            }
            UiCallback::NewGame => {
                app.ui.set_state(UiState::NewTeam);
                app.new_world();
                Ok(None)
            }
            UiCallback::ContinueGame => {
                app.ui.splash_screen.set_index(0);
                app.load_world();
                Ok(None)
            }
            UiCallback::QuitGame => {
                app.quit()?;
                Ok(None)
            }
            #[cfg(feature = "audio")]
            UiCallback::ToggleAudio => {
                if let Some(player) = app.audio_player.as_mut() {
                    player.toggle_state()?;
                } else {
                    info!("No audio player, cannot toggle it");
                }

                Ok(None)
            }
            #[cfg(feature = "audio")]
            UiCallback::PreviousRadio => {
                if let Some(player) = app.audio_player.as_mut() {
                    player.previous_radio_stream()?;
                } else {
                    info!("No audio player, cannot select previous sample");
                }
                Ok(None)
            }
            #[cfg(feature = "audio")]
            UiCallback::NextRadio => {
                if let Some(player) = app.audio_player.as_mut() {
                    player.next_radio_stream()?;
                } else {
                    info!("No audio player, cannot select next sample");
                }
                Ok(None)
            }
            UiCallback::SetSwarmPanelView { topic } => {
                app.ui.swarm_panel.set_view(*topic);
                Ok(None)
            }
            UiCallback::SetMyTeamPanelView { view } => {
                app.ui.my_team_panel.set_view(*view);
                Ok(None)
            }
            UiCallback::SetPlayerPanelView { view } => {
                app.ui.player_panel.set_view(*view);
                Ok(None)
            }
            UiCallback::SetTeamPanelView { view } => {
                app.ui.team_panel.set_view(*view);
                Ok(None)
            }
            UiCallback::PromptAbandonAsteroid { asteroid_id } => {
                let asteroid = app.world.get_planet_or_err(asteroid_id)?;

                app.ui.push_popup(PopupMessage::AbandonAsteroid {
                    asteroid_name: asteroid.name.clone(),
                    asteroid_id: *asteroid_id,
                    tick: Tick::now(),
                });
                Ok(None)
            }
            UiCallback::ConfirmAbandonAsteroid { asteroid_id } => {
                let own_team = app.world.get_own_team_mut()?;
                own_team.asteroid_ids.retain(|&id| id != *asteroid_id);
                app.ui.close_popup();
                Ok(None)
            }
            UiCallback::HirePlayer { player_id } => {
                let own_team_id = app.world.own_team_id;
                app.world.hire_player_for_team(player_id, &own_team_id)?;

                Ok(None)
            }
            UiCallback::PromptReleasePlayer { player_id } => {
                let player = app.world.get_player_or_err(player_id)?;
                let own_team = app.world.get_own_team()?;
                let not_enough_players_for_game =
                    if own_team.player_ids.len() - 1 < MIN_PLAYERS_PER_GAME {
                        true
                    } else {
                        false
                    };
                app.ui.push_popup(PopupMessage::ReleasePlayer {
                    player_name: player.info.full_name(),
                    player_id: *player_id,
                    not_enough_players_for_game,
                    tick: Tick::now(),
                });
                Ok(None)
            }
            UiCallback::ConfirmReleasePlayer { player_id } => {
                app.world.release_player_from_team(*player_id)?;
                app.ui.close_popup();
                app.ui.swarm_panel.remove_player_from_ranking(*player_id);
                Ok(None)
            }
            UiCallback::LockPlayerPanel { player_id } => {
                if app.ui.player_panel.locked_player_id.is_some()
                    && app.ui.player_panel.locked_player_id.unwrap() == *player_id
                {
                    app.ui.player_panel.locked_player_id = None;
                } else {
                    app.ui.player_panel.locked_player_id = Some(*player_id);
                }
                Ok(None)
            }
            UiCallback::SetCrewRole { player_id, role } => {
                app.world.set_team_crew_role(*role, *player_id)?;
                Ok(None)
            }

            UiCallback::Drink { player_id } => {
                let mut player = app.world.get_player_or_err(player_id)?.clone();
                player.can_drink(&app.world)?;

                let morale_bonus = if matches!(player.special_trait, Some(Trait::Spugna)) {
                    MAX_SKILL
                } else {
                    MORALE_DRINK_BONUS
                };

                let tiredness_malus = if matches!(player.special_trait, Some(Trait::Spugna)) {
                    TIREDNESS_DRINK_MALUS_SPUGNA
                } else {
                    TIREDNESS_DRINK_MALUS
                };

                player.add_morale(morale_bonus);
                player.add_tiredness(tiredness_malus);

                let mut team = app
                    .world
                    .get_team_or_err(&player.team.expect("Player should have team"))?
                    .clone();

                team.sub_resource(Resource::RUM, 1)?;

                //If player is a spugna and pilot and team is travelling or exploring and player was already maxxed in morale,
                // there is a chance that the player enters a portal to a random planet.
                let rng = &mut ChaCha8Rng::from_os_rng();

                let discovery_probability = (PORTAL_DISCOVERY_PROBABILITY
                    * TeamBonus::Exploration.current_player_bonus(&player)? as f64)
                    .min(1.0);
                if matches!(player.special_trait, Some(Trait::Spugna))
                    && player.info.crew_role == CrewRole::Pilot
                    && rng.random_bool(discovery_probability)
                {
                    let portal_target_id = match team.current_location {
                        TeamLocation::OnPlanet { .. } | TeamLocation::OnSpaceAdventure { .. } => {
                            None
                        }
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
                        let portal_target = app.world.get_planet_or_err(&to)?;
                        // We set the new target to the portal_target
                        let from = match team.current_location {
                            TeamLocation::OnPlanet { .. }
                            | TeamLocation::OnSpaceAdventure { .. } => {
                                unreachable!()
                            }
                            TeamLocation::Travelling { from, .. } => from,
                            TeamLocation::Exploring { around, .. } => around,
                        };

                        let distance = app.world.distance_between_planets(from, to)?;
                        // Notice that the team will arrive when  world.last_tick_short_interval > started + duration.
                        team.current_location = TeamLocation::Travelling {
                            from,
                            to,
                            started: Tick::now(),
                            duration: PORTAL_TRAVEL_DURATION,
                            distance,
                        };

                        app.ui.push_popup(PopupMessage::PortalFound {
                            player_name: player.info.short_name(),
                            portal_target: portal_target.name.to_string(),
                            tick: Tick::now(),
                        });
                    }
                }

                app.world.players.insert(player.id, player);
                app.world.teams.insert(team.id, team);
                app.world.dirty_network = true;
                app.world.dirty_ui = true;
                app.world.dirty = true;

                Ok(None)
            }
            UiCallback::GeneratePlayerTeam {
                name,
                home_planet,
                jersey_style,
                jersey_colors,
                players,
                spaceship,
            } => Self::generate_own_team(
                name.clone(),
                *home_planet,
                *jersey_style,
                *jersey_colors,
                players.clone(),
                spaceship.clone(),
            )(app),
            UiCallback::CancelGeneratePlayerTeam => Self::cancel_generate_own_team()(app),
            UiCallback::AssignBestTeamPositions => Self::assign_best_team_positions()(app),
            UiCallback::SwapPlayerPositions {
                player_id,
                position,
            } => Self::swap_player_positions(*player_id, *position)(app),
            UiCallback::NextTrainingFocus { team_id } => Self::next_training_focus(*team_id)(app),
            UiCallback::TravelToPlanet { planet_id } => Self::travel_to_planet(*planet_id)(app),
            UiCallback::ExploreAroundPlanet { duration } => {
                Self::explore_around_planet(duration.clone())(app)
            }
            UiCallback::ZoomToPlanet {
                planet_id,
                zoom_level,
            } => Self::zoom_to_planet(*planet_id, *zoom_level)(app),
            UiCallback::Ping => Self::ping()(app),
            UiCallback::Sync => Self::sync()(app),
            UiCallback::SendMessage { message } => Self::send(message.clone())(app),
            UiCallback::PushUiPopup { popup_message } => {
                app.ui.push_popup(popup_message.clone());
                Ok(None)
            }
            UiCallback::NameAndAcceptAsteroid { name, filename } => {
                Self::name_and_accept_asteroid(name.clone(), filename.clone())(app)
            }
            UiCallback::SetSpaceshipUpgrade { upgrade } => {
                Self::set_spaceship_upgrade(upgrade.clone())(app)
            }
            UiCallback::UpgradeSpaceship { upgrade } => {
                Self::upgrade_spaceship(upgrade.clone())(app)
            }
            UiCallback::SetAsteroidUpgrade {
                asteroid_id,
                upgrade,
            } => Self::set_asteroid_upgrade(*asteroid_id, upgrade.clone())(app),
            UiCallback::UpgradeAsteroid {
                asteroid_id,
                upgrade,
            } => Self::upgrade_asteroid(*asteroid_id, upgrade.clone())(app),
            UiCallback::StartSpaceAdventure => {
                app.ui.set_state(UiState::SpaceAdventure);
                let mut own_team = app.world.get_own_team()?.clone();
                own_team.can_start_space_adventure()?;

                let should_spawn_asteroid = match own_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => {
                        let current_planet = app.world.get_planet_or_err(&planet_id)?;
                        current_planet.asteroid_probability > 0.0
                            && own_team.asteroid_ids.len() < MAX_NUM_ASTEROID_PER_TEAM
                    }
                    _ => unreachable!(),
                };

                let speed_bonus =
                    TeamBonus::SpaceshipSpeed.current_team_bonus(&app.world, &own_team.id)?;
                let weapons_bonus =
                    TeamBonus::Weapons.current_team_bonus(&app.world, &own_team.id)?;

                let gold_fragment_probability = match own_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => {
                        let current_planet = app.world.get_planet_or_err(&planet_id)?;
                        0.001
                            + 0.075 * (current_planet.resources.value(&Resource::GOLD) as f64)
                                / MAX_SKILL as f64
                    }
                    _ => unreachable!(),
                };

                let space = SpaceAdventure::new(should_spawn_asteroid, gold_fragment_probability)?
                    .with_player(
                        &own_team.spaceship,
                        own_team.resources.clone(),
                        speed_bonus,
                        weapons_bonus,
                        own_team.fuel(),
                    )?;

                match own_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => {
                        own_team.current_location = TeamLocation::OnSpaceAdventure {
                            around: planet_id.clone(),
                        }
                    }
                    _ => {
                        return Err(anyhow!(
                            "Team should be on a planet to start a space adventure."
                        ));
                    }
                }
                app.world.teams.insert(own_team.id, own_team);
                app.world.space_adventure = Some(space);
                Ok(None)
            }

            UiCallback::StopSpaceAdventure => {
                if let Some(space) = app.world.space_adventure.as_mut() {
                    space.stop_space_adventure();
                }

                Ok(None)
            }

            UiCallback::ReturnFromSpaceAdventure => {
                app.ui.set_state(UiState::Main);
                let mut own_team = app.world.get_own_team()?.clone();

                let space = app
                    .world
                    .space_adventure
                    .take()
                    .ok_or(anyhow!("World should have a space adventure"))?;

                let player = space
                    .get_player()
                    .ok_or(anyhow!("Space adventure should have a player entity."))?;
                let player_control: &dyn ControllableSpaceship = player
                    .as_trait_ref()
                    .expect("Player should implement ControllableSpaceship.");

                // Update team space adventure data
                own_team.number_of_space_adventures += 1;
                let mut resources_gathered_text = "".to_string();
                let mut new_resources = ResourceMap::new();

                for (&resource, &amount) in player_control.resources().iter() {
                    let current_amount = own_team.resources.value(&resource);
                    // If durability is zero, the cargo (and fuel) has been lost (not the satoshi or fuel).
                    if (resource != Resource::SATOSHI || resource != Resource::FUEL)
                        && player_control.current_durability() == 0
                    {
                        continue;
                    }
                    // The player_control.resources are the sum of the resources the plahyer had
                    // at the beginning of the adventure + the resources gathered.
                    new_resources.insert(resource, amount);
                    // Gathered amount should always be larger equal to amount, apart from fuel.
                    let gathered_amount = amount.saturating_sub(current_amount);

                    if gathered_amount > 0 {
                        let current_gathered = own_team.resources_gathered.value(&resource);
                        own_team
                            .resources_gathered
                            .insert(resource, current_gathered + gathered_amount);
                        resources_gathered_text.push_str(
                            format!(
                                "  {} {}\n",
                                gathered_amount,
                                resource.to_string().to_lowercase()
                            )
                            .as_str(),
                        );
                    }
                }

                if resources_gathered_text.len() == 0 {
                    resources_gathered_text.push_str("No resources collected!")
                } else {
                    resources_gathered_text.push_str("collected.")
                }

                own_team.resources = new_resources;
                own_team
                    .spaceship
                    .set_current_durability(player_control.current_durability());

                match own_team.current_location {
                    TeamLocation::OnSpaceAdventure { around } => {
                        own_team.current_location = TeamLocation::OnPlanet { planet_id: around }
                    }
                    _ => {
                        return Err(anyhow!("Team should be on a space adventure."));
                    }
                }

                own_team.reputation =
                    (own_team.reputation + ReputationModifier::SMALL_BONUS).bound();
                app.world.teams.insert(own_team.id, own_team);

                if let Some(asteroid_type) = space.asteroid_planet_found() {
                    app.ui.push_popup(PopupMessage::AsteroidNameDialog {
                        tick: Tick::now(),
                        asteroid_type,
                    });
                }

                return Ok(Some(format!(
                    "Team returned from space adventure:\n{}",
                    resources_gathered_text
                )));
            }
            UiCallback::SpaceMovePlayerLeft => {
                if let Some(space) = app.world.space_adventure.as_mut() {
                    space.handle_player_input(PlayerInput::MoveLeft)?;
                }

                Ok(None)
            }
            UiCallback::SpaceMovePlayerRight => {
                if let Some(space) = app.world.space_adventure.as_mut() {
                    space.handle_player_input(PlayerInput::MoveRight)?;
                }

                Ok(None)
            }
            UiCallback::SpaceMovePlayerDown => {
                if let Some(space) = app.world.space_adventure.as_mut() {
                    space.handle_player_input(PlayerInput::MoveDown)?;
                }

                Ok(None)
            }
            UiCallback::SpaceMovePlayerUp => {
                if let Some(space) = app.world.space_adventure.as_mut() {
                    space.handle_player_input(PlayerInput::MoveUp)?;
                }

                Ok(None)
            }
            UiCallback::SpaceToggleAutofire => {
                if let Some(space) = app.world.space_adventure.as_mut() {
                    space.handle_player_input(PlayerInput::ToggleAutofire)?;
                }

                Ok(None)
            }
            UiCallback::SpaceShoot => {
                if let Some(space) = app.world.space_adventure.as_mut() {
                    space.handle_player_input(PlayerInput::Shoot)?;
                }

                Ok(None)
            }

            UiCallback::SpaceReleaseScraps => {
                if let Some(space) = app.world.space_adventure.as_mut() {
                    space.handle_player_input(PlayerInput::ReleaseScraps)?;
                }

                Ok(None)
            }

            UiCallback::ToggleTeamAutonomousStrategyForLocalChallenges => {
                let own_team = app.world.get_own_team_mut()?;
                own_team.autonomous_strategy.challenge_local =
                    !own_team.autonomous_strategy.challenge_local;
                Ok(None)
            }

            UiCallback::ToggleTeamAutonomousStrategyForNetworkChallenges => {
                let own_team = app.world.get_own_team_mut()?;
                own_team.autonomous_strategy.challenge_network =
                    !own_team.autonomous_strategy.challenge_network;
                Ok(None)
            }
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CallbackRegistry {
    mouse_callbacks: HashMap<MouseEventKind, HashMap<Option<Rect>, UiCallback>>,
    keyboard_callbacks: HashMap<KeyCode, UiCallback>,
    hovering: (u16, u16),
    max_layer: usize,
}

impl CallbackRegistry {
    fn contains(rect: &Rect, x: u16, y: u16) -> bool {
        rect.x <= x && x < rect.x + rect.width && rect.y <= y && y < rect.y + rect.height
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_max_layer(&mut self, layer: usize) {
        self.max_layer = layer;
    }

    pub fn get_max_layer(&self) -> usize {
        self.max_layer
    }

    pub fn register_mouse_callback(
        &mut self,
        event_kind: MouseEventKind,
        rect: Option<Rect>,
        callback: UiCallback,
    ) {
        self.mouse_callbacks
            .entry(event_kind)
            .or_insert_with(HashMap::new)
            .insert(rect, callback);
    }

    pub fn register_keyboard_callback(&mut self, key_code: KeyCode, callback: UiCallback) {
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

    pub fn hovering(&self) -> (u16, u16) {
        self.hovering
    }

    pub fn set_hovering(&mut self, position: (u16, u16)) {
        self.hovering = position;
    }

    pub fn handle_mouse_event(&self, event: &MouseEvent) -> Option<UiCallback> {
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

    pub fn handle_keyboard_event(&self, key_code: &KeyCode) -> Option<UiCallback> {
        self.keyboard_callbacks.get(key_code).cloned()
    }
}

#[cfg(test)]
mod test {
    use super::UiCallback;
    use crate::{
        app::App,
        space_adventure::{ControllableSpaceship, SpaceCallback},
        types::{AppResult, ResourceMap, StorableResourceMap},
        ui::ui_callback::INITIAL_TEAM_BALANCE,
        world::resources::Resource,
    };

    #[test]
    fn test_resource_gathered_in_space_adventure() -> AppResult<()> {
        let mut app = App::test_default()?;
        let world = &mut app.world;
        let own_team = world.get_own_team_mut()?;
        own_team.add_resource(Resource::FUEL, 100)?;

        println!("Own team resources: {:#?}", own_team.resources);

        assert!(own_team.resources.value(&Resource::GOLD) == 0);
        assert!(own_team.resources.value(&Resource::FUEL) == 100);
        assert!(own_team.resources.value(&Resource::SATOSHI) == INITIAL_TEAM_BALANCE);

        let own_team_resources = own_team.resources.clone();

        UiCallback::StartSpaceAdventure.call(&mut app)?;

        let space = app
            .world
            .space_adventure
            .as_mut()
            .expect("There should be a space adventure");

        let player_id = space.get_player().expect("There should be a player").id();

        let space_callbacks = vec![
            SpaceCallback::CollectFragment {
                id: player_id,
                resource: Resource::GOLD,
                amount: 10,
            },
            SpaceCallback::DamageEntity {
                id: player_id,
                damage: 2000.0,
            },
        ];

        for cb in space_callbacks {
            cb.call(space);
        }

        let player = space.get_player().expect("There should be a player");

        let player_control: &dyn ControllableSpaceship = player
            .as_trait_ref()
            .expect("Player entity should implement ControllableSpaceship");

        assert!(player_control.current_durability() == 0);
        assert!(player_control.resources().value(&Resource::GOLD) == 10);
        assert!(player_control.resources().value(&Resource::SATOSHI) == INITIAL_TEAM_BALANCE);
        assert!(player_control.resources().value(&Resource::FUEL) == 100);

        println!(
            "Player durability: {}/{}",
            player_control.current_durability(),
            player_control.durability()
        );
        println!(
            "After adventure resources: {:#?}",
            player_control.resources()
        );

        let player_control_resources = player_control.resources().clone();

        let mut new_resources = ResourceMap::new();

        for (&resource, &amount) in player_control_resources.iter() {
            let current_amount = own_team_resources.value(&resource);
            // If durability is zero, the cargo (and fuel) has been lost (not the satoshi).
            if resource != Resource::SATOSHI && player_control.current_durability() == 0 {
                continue;
            }
            new_resources.insert(resource, amount);

            assert!(amount >= current_amount);
        }

        println!("Collected {:#?}", new_resources);
        assert!(new_resources.value(&Resource::GOLD) == 0);
        assert!(new_resources.value(&Resource::SATOSHI) == INITIAL_TEAM_BALANCE);
        assert!(new_resources.value(&Resource::FUEL) == 0);

        Ok(())
    }
}
