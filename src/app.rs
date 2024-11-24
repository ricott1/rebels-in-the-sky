use crate::audio;
use crate::audio::music_player::MusicPlayer;
use crate::network::handler::NetworkHandler;
use crate::store::{get_world_size, load_world, reset, save_world};
use crate::tui::{EventHandler, TerminalEvent};
use crate::tui::{Tui, WriterProxy};
use crate::types::{AppResult, ResourceMap, SystemTimeTick, Tick};
use crate::ui::popup_message::PopupMessage;
use crate::ui::ui::{Ui, UiState};
use crate::ui::utils::SwarmPanelEvent;
use crate::world::constants::{TickInterval, SECONDS};
use crate::world::types::TeamLocation;
use crate::world::world::World;
use crossterm::event::{KeyCode, KeyModifiers};
use futures::StreamExt;
use libp2p::{gossipsub, swarm::SwarmEvent};
use log::{error, info, warn};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::StreamDownload;
use tokio::select;

const NETWORK_HANDLER_INIT_INTERVAL: Tick = 10 * SECONDS;

#[derive(Debug, PartialEq)]
pub enum AppState {
    Started,
    Simulating,
    Quitting,
}

#[derive(Debug)]
pub struct App {
    pub world: World,
    pub state: AppState,
    pub ui: Ui,
    pub audio_player: Option<MusicPlayer>,
    generate_local_world: bool,
    pub network_handler: Option<NetworkHandler>,
    seed_ip: Option<String>,
    network_port: Option<u16>,
    store_prefix: String,
    pub new_version_notified: bool,
}

impl App {
    pub async fn simulate_loaded_world<W: WriterProxy, E: EventHandler>(
        &mut self,
        tui: &mut Tui<W, E>,
    ) {
        let mut callbacks = vec![];
        let mut last_tui_update = Tick::now();
        info!(
            "Simulation started, must simulate {}",
            (Tick::now() - self.world.last_tick_short_interval).formatted()
        );

        // If team is on a space adventure, bring it back to base planet.
        // This is an ad-hoc fix to avoid problems when the game is closed during a space adventure,
        // since the space property of the world is not serialized and stored.
        let mut own_team = self
            .world
            .get_own_team()
            .expect("There should be an own team.")
            .clone();
        match own_team.current_location {
            TeamLocation::OnSpaceAdventure { around } => {
                // The team loses all resources and fuel, tough shit.
                own_team.resources = ResourceMap::default();
                own_team.spaceship.set_current_durability(0);
                own_team.current_location = TeamLocation::OnPlanet { planet_id: around };

                self.world.teams.insert(own_team.id, own_team);
                self.ui
                    .push_popup(PopupMessage::Ok{
                       message: "The game was closed during a space adventure.\nAll the cargo and fuel have been lost.\nNext time go back to the base first!".to_string(), is_skippable:false,tick: Tick::now()});
            }
            _ => {}
        }

        while self.world.is_simulating() {
            // Give a visual feedback by drawing.
            let now = Tick::now();
            if now - last_tui_update > tui.simulation_update_interval() {
                last_tui_update = now;
                if let Err(e) = self.ui.update(&self.world, self.audio_player.as_ref()) {
                    error!("Error updating TUI during simulation: {e}")
                };
                if let Err(e) = tui
                    .draw(&mut self.ui, &self.world, self.audio_player.as_ref())
                    .await
                {
                    error!("Error drawing TUI during simulation: {e}")
                };
            }

            let mut cb = match self
                .world
                .handle_tick_events(self.world.last_tick_short_interval + TickInterval::SHORT)
            {
                Ok(callbacks) => callbacks,
                Err(e) => panic!("Failed to simulate world: {}", e),
            };
            callbacks.append(&mut cb);
        }

        self.world.serialized_size =
            get_world_size(&self.store_prefix).expect("Failed to get world size");

        self.state = AppState::Started;
        self.ui.set_state(UiState::Main);

        for callback in callbacks.iter() {
            match callback.call(self) {
                Ok(Some(message)) => {
                    self.ui.push_popup(PopupMessage::Ok {
                        message,
                        is_skippable: true,
                        tick: Tick::now(),
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    panic!("Failed to simulate world: {}", e);
                }
            }
        }
    }

    async fn conditional_audio_event(
        audio_player: &Option<MusicPlayer>,
    ) -> Option<StreamDownload<TempStorageProvider>> {
        match audio_player.as_ref() {
            Some(player) => Some(player.next_streaming_event().ok()?),
            None => None,
        }
    }

    pub async fn conditional_network_event(
        network_handler: &mut Option<NetworkHandler>,
    ) -> Option<SwarmEvent<gossipsub::Event>> {
        match network_handler.as_mut() {
            Some(handler) => Some(handler.swarm.select_next_some().await),
            None => None,
        }
    }

    pub fn initialize_network_handler(&mut self) -> AppResult<()> {
        if let Some(tcp_port) = self.network_port {
            let handler = NetworkHandler::new(self.seed_ip.clone(), tcp_port)?;
            self.network_handler = Some(handler);
        } else {
            error!("Cannot initialize network handler: TCP port not set.")
        }
        Ok(())
    }

    pub fn test_default() -> AppResult<Self> {
        let mut app = App::new(None, true, true, true, false, None, None, None);
        app.new_world();
        let home_planet_id = app
            .world
            .planets
            .keys()
            .next()
            .expect("There should be at elast one planet")
            .clone();
        app.world.own_team_id = app.world.generate_random_team(
            &mut ChaCha8Rng::from_entropy(),
            home_planet_id,
            "own team".into(),
            "ship_name".into(),
        )?;

        Ok(app)
    }

    pub fn new(
        seed: Option<u64>,
        disable_network: bool,
        disable_audio: bool,
        generate_local_world: bool,
        reset_world: bool,
        seed_ip: Option<String>,
        network_port: Option<u16>,
        store_prefix: Option<&str>,
    ) -> Self {
        // If the reset_world flag is set, reset the world.
        if reset_world {
            reset().expect("Failed to reset world");
        }

        let store_prefix = store_prefix.unwrap_or("local");

        let ui = Ui::new(store_prefix, disable_network);
        let audio_player = if disable_audio {
            None
        } else {
            if let Ok(player) = audio::music_player::MusicPlayer::new() {
                info!("Audio player created succesfully");
                Some(player)
            } else {
                warn!("Could not create audio player");
                None
            }
        };

        Self {
            world: World::new(seed),
            state: AppState::Started,
            ui,
            audio_player,
            generate_local_world,
            network_handler: None,
            seed_ip,
            network_port,
            store_prefix: store_prefix.to_string(),
            new_version_notified: false,
        }
    }

    pub async fn run<W: WriterProxy, E: EventHandler>(
        &mut self,
        mut tui: Tui<W, E>,
    ) -> AppResult<()> {
        let mut last_network_handler_init = 0;

        while self.state != AppState::Quitting {
            let now = Tick::now();

            if self.state == AppState::Simulating {
                info!("Starting world simulation...");
                self.simulate_loaded_world(&mut tui).await;
            }

            if self.network_port.is_some()
                && self.network_handler.is_none()
                && self.world.has_own_team()
                && now - last_network_handler_init > NETWORK_HANDLER_INIT_INTERVAL
            {
                info!("Initializing network handler...");
                if let Err(e) = self.initialize_network_handler() {
                    error!("Could not initialize network handler: {}", e);
                    last_network_handler_init = now;
                }
            }

            select! {
                Some(streaming_data) = Self::conditional_audio_event(& self.audio_player) =>  self.handle_streaming_data(streaming_data)?,
                Some(swarm_event) = Self::conditional_network_event(&mut self.network_handler) =>  self.handle_network_events(swarm_event)?,
                app_event = tui.events.next() => {
                    match app_event{
                        TerminalEvent::Tick {tick} => {
                                self.handle_tick_events(tick)?;
                            if let Err(e) = tui.draw(&mut self.ui, &self.world, self.audio_player.as_ref()).await {
                                error!("Drawing error: {e}");
                            }
                        }
                        TerminalEvent::Key(key_event) => {
                            self.handle_key_events(key_event)?;
                            if let Err(e) = tui.draw(&mut self.ui, &self.world, self.audio_player.as_ref()).await {
                                error!("Drawing error: {e}");
                            }
                        },
                        TerminalEvent::Mouse(mouse_event) => {
                            self.handle_mouse_events(mouse_event)?;
                            // if let Err(e) = tui.draw(&mut self.ui, &self.world, self.audio_player.as_ref()).await {
                            //     error!("Drawing error: {e}");
                            // }
                        },
                        TerminalEvent::Resize(w, h) => tui.resize((w, h))?,
                        TerminalEvent::Quit => self.quit()?,
                    }
                }
            }
        }
        info!("Game loop closed");
        tui.exit().await?;
        Ok(())
    }

    pub fn new_world(&mut self) {
        if let Err(e) = self.world.initialize(self.generate_local_world) {
            panic!("Failed to initialize world: {}", e);
        }
    }

    pub fn load_world(&mut self) {
        // Try to load an existing world.
        match load_world(&self.store_prefix) {
            Ok(w) => self.world = w,
            Err(e) => panic!("Failed to load world: {}", e),
        }
        self.state = AppState::Simulating;
    }

    /// Set running to false to quit the application.
    pub fn quit(&mut self) -> AppResult<()> {
        self.state = AppState::Quitting;

        // save world and backup
        if self.world.has_own_team() {
            save_world(&self.world, true, &self.store_prefix)?;
        }

        // close network connections
        if let Some(network_handler) = &mut self.network_handler {
            network_handler.quit();
        }

        Ok(())
    }

    pub fn render(
        ui: &mut Ui,
        world: &World,
        audio_player: Option<&audio::music_player::MusicPlayer>,
        frame: &mut ratatui::Frame,
    ) {
        ui.render(frame, world, audio_player);
    }

    /// Handles the tick event of the terminal.
    pub fn handle_tick_events(&mut self, current_tick: Tick) -> AppResult<()> {
        if self.world.has_own_team() {
            match self.world.handle_tick_events(current_tick) {
                Ok(callbacks) => {
                    for callback in callbacks.iter() {
                        match callback.call(self) {
                            Ok(Some(message)) => {
                                self.ui.push_popup(PopupMessage::Ok {
                                    message,
                                    is_skippable: true,
                                    tick: Tick::now(),
                                });
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.ui.push_popup(PopupMessage::Error {
                                    message: e.to_string(),
                                    tick: Tick::now(),
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    self.ui.push_popup(PopupMessage::Error {
                        message: format!("Tick error\n{}", e.to_string()),
                        tick: Tick::now(),
                    });
                }
            }
        }

        match self.ui.update(&self.world, self.audio_player.as_ref()) {
            Ok(_) => {}
            Err(e) => {
                // We push to Logs rather than Error popup since otherwise it would spam too much
                self.ui.swarm_panel.push_log_event(SwarmPanelEvent {
                    timestamp: Tick::now(),
                    peer_id: None,
                    text: format!("Ui update error\n{}", e.to_string()),
                })
            }
        }
        self.world.dirty_ui = false;

        if self.world.dirty && self.world.has_own_team() {
            self.world.dirty = false;
            save_world(&self.world, false, &self.store_prefix).expect("Failed to save world");
            self.world.serialized_size =
                get_world_size(&self.store_prefix).expect("Failed to get world size");

            self.ui.swarm_panel.push_log_event(SwarmPanelEvent {
                timestamp: Tick::now(),
                peer_id: None,
                text: format!("World saved, size: {} bytes", self.world.serialized_size),
            });
        }

        // Send own team to peers if dirty
        if self.world.dirty_network && self.world.has_own_team() {
            self.world.dirty_network = false;
            if let Some(network_handler) = &mut self.network_handler {
                if network_handler.swarm.connected_peers().count() > 0 {
                    if let Err(e) = network_handler.send_own_team(&self.world) {
                        self.ui.swarm_panel.push_log_event(SwarmPanelEvent {
                            timestamp: Tick::now(),
                            peer_id: None,
                            text: format!("Failed to send own team to peers: {}", e),
                        });
                    }
                } else if let Err(e) = network_handler.dial_seed() {
                    self.ui.swarm_panel.push_log_event(SwarmPanelEvent {
                        timestamp: Tick::now(),
                        peer_id: None,
                        text: format!("Failed to dial seed: {}", e),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn handle_key_events(&mut self, key_event: crossterm::event::KeyEvent) -> AppResult<()> {
        match key_event.code {
            // Exit application directly on `Ctrl-C`. `Esc` asks for confirmation first.
            KeyCode::Char('c') | KeyCode::Char('C')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                self.quit()?;
            }
            _ => {
                if let Some(callback) = self.ui.handle_key_events(key_event, &self.world) {
                    match callback.call(self) {
                        Ok(Some(message)) => {
                            self.ui.push_popup(PopupMessage::Ok {
                                message,
                                is_skippable: true,
                                tick: Tick::now(),
                            });
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.ui.push_popup(PopupMessage::Error {
                                message: e.to_string(),
                                tick: Tick::now(),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn handle_mouse_events(
        &mut self,
        mouse_event: crossterm::event::MouseEvent,
    ) -> AppResult<()> {
        if let Some(callback) = self.ui.handle_mouse_events(mouse_event) {
            match callback.call(self) {
                Ok(Some(message)) => {
                    self.ui.push_popup(PopupMessage::Ok {
                        message,
                        is_skippable: true,
                        tick: Tick::now(),
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    self.ui.push_popup(PopupMessage::Error {
                        message: e.to_string(),
                        tick: Tick::now(),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn handle_network_events(
        &mut self,
        network_event: SwarmEvent<gossipsub::Event>,
    ) -> AppResult<()> {
        if let Some(network_handler) = &mut self.network_handler {
            if let Some(callback) = network_handler.handle_network_events(network_event) {
                match callback.call(self) {
                    Ok(Some(message)) => {
                        self.ui.push_popup(PopupMessage::Ok {
                            message,
                            is_skippable: true,
                            tick: Tick::now(),
                        });
                    }
                    Ok(None) => {}
                    Err(e) => {
                        // Append error to swarm log
                        self.ui.swarm_panel.push_log_event(SwarmPanelEvent {
                            timestamp: Tick::now(),
                            peer_id: None,
                            text: e.to_string(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_streaming_data(
        &mut self,
        data: StreamDownload<TempStorageProvider>,
    ) -> AppResult<()> {
        if let Some(audio_player) = &mut self.audio_player {
            audio_player.handle_streaming_ready(data)?;
        }
        Ok(())
    }
}
