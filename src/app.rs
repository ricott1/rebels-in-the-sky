use crossterm::event::{KeyCode, KeyModifiers};

use libp2p::{gossipsub, swarm::SwarmEvent};
#[cfg(feature = "audio")]
use log::warn;
use log::{error, info};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "audio")]
use stream_download::{storage::temp::TempStorageProvider, StreamDownload};

#[cfg(feature = "audio")]
use crate::audio::music_player::MusicPlayer;

use crate::network::handler::NetworkHandler;
use crate::{
    crossterm_event_handler,
    store::{get_world_size, load_world, reset, save_world, world_file_data},
    tick_event_handler,
    tui::{TerminalEvent, Tui, WriterProxy},
    types::{AppResult, ResourceMap, StorableResourceMap, SystemTimeTick, Tick},
    ui::{
        popup_message::PopupMessage,
        ui::{Ui, UiState},
    },
    world::*,
};

#[derive(Debug, PartialEq)]
pub enum AppState {
    Running,
    Simulating,
    Quitting,
}

#[derive(Debug)]
pub enum AppEvent {
    SlowTick(Tick),
    FastTick(Tick),
    TerminalEvent(TerminalEvent),
    NetworkEvent(SwarmEvent<gossipsub::Event>),
    #[cfg(feature = "audio")]
    AudioEvent(StreamDownload<TempStorageProvider>),
}

#[derive(Debug)]
pub struct App {
    event_sender: mpsc::Sender<AppEvent>,
    event_receiver: mpsc::Receiver<AppEvent>,
    pub world: World,
    pub state: AppState,
    pub ui: Ui,
    #[cfg(feature = "audio")]
    pub audio_player: Option<MusicPlayer>,
    generate_local_world: bool,

    pub network_handler: NetworkHandler,
    store_prefix: String,
    new_version_notified: bool,

    cancellation_token: CancellationToken,

    app_version: [usize; 3],
}

impl App {
    pub fn app_version(&self) -> [usize; 3] {
        self.app_version
    }
    pub fn get_event_sender(&self) -> mpsc::Sender<AppEvent> {
        self.event_sender.clone()
    }

    pub async fn simulate_loaded_world<W: WriterProxy>(&mut self, tui: &mut Tui<W>) {
        let mut callbacks = vec![];
        let mut last_tui_update = Tick::now();
        info!(
            "Simulation started, must simulate {}",
            (Tick::now() - self.world.last_tick_short_interval).formatted()
        );

        // If team is on a space adventure, bring it back to base planet.
        // This is an ad-hoc fix to avoid problems when the game is closed during a space adventure,
        // since the space property of the world is not serialized and stored.
        let own_team = self
            .world
            .get_own_team_mut()
            .expect("There should be an own team when simulating.");
        if let TeamLocation::OnSpaceAdventure { around } = own_team.current_location {
            // The team loses all resources but satoshis, we told you so!
            let current_treasury = own_team.resources.value(&Resource::SATOSHI);
            own_team.resources = ResourceMap::default();
            own_team
                .add_resource(Resource::SATOSHI, current_treasury)
                .expect("It should always be possible to add satoshis");
            own_team.spaceship.set_current_durability(0);
            own_team.current_location = TeamLocation::OnPlanet { planet_id: around };

            self.ui
                .push_popup(PopupMessage::Ok{
                   message: "The game was closed during a space adventure.\nAll the cargo and fuel have been lost.\nNext time go back to the base first!".to_string(), is_skippable:false,tick: Tick::now()});
        }

        const SIMULATION_UPDATE_INTERVAL: Tick = 250 * MILLISECONDS;
        while self.world.is_simulating() {
            // Give a visual feedback by drawing.
            let now = Tick::now();

            if now.saturating_sub(last_tui_update) > SIMULATION_UPDATE_INTERVAL {
                last_tui_update = now;
                if let Err(e) = self.ui.update(
                    &self.world,
                    #[cfg(feature = "audio")]
                    self.audio_player.as_ref(),
                ) {
                    error!("Error updating TUI during simulation: {e}")
                };
                self.draw(tui).await;
            }

            let mut cb = match self
                .world
                .handle_slow_tick_events(self.world.last_tick_short_interval + TickInterval::SHORT)
            {
                Ok(callbacks) => callbacks,
                Err(e) => panic!("Failed to simulate world: {e}"),
            };
            callbacks.append(&mut cb);
        }

        self.world.serialized_size =
            get_world_size(&self.store_prefix).expect("Failed to get world size");

        self.state = AppState::Running;
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
                    panic!("Failed to simulate world: {e}");
                }
            }
        }
    }

    pub fn test_default() -> AppResult<Self> {
        let mut app = App::new(
            Some(0),
            true,
            #[cfg(feature = "audio")]
            true,
            true,
            false,
            None,
            None,
        )?;
        app.new_world();
        let home_planet_id = *app
            .world
            .planets
            .keys()
            .next()
            .expect("There should be at elast one planet");
        app.world.own_team_id = app.world.generate_random_team(
            &mut ChaCha8Rng::from_os_rng(),
            home_planet_id,
            "own team".into(),
            "ship_name".into(),
            None,
        )?;

        Ok(app)
    }

    pub fn test_with_network_handler() -> AppResult<Self> {
        let mut app = App::new(
            Some(0),
            true,
            #[cfg(feature = "audio")]
            true,
            true,
            false,
            None,
            None,
        )?;
        app.new_world();
        let home_planet_id = *app
            .world
            .planets
            .keys()
            .next()
            .expect("There should be at elast one planet");
        app.world.own_team_id = app.world.generate_random_team(
            &mut ChaCha8Rng::from_os_rng(),
            home_planet_id,
            "own team".into(),
            "ship_name".into(),
            None,
        )?;

        {
            app.network_handler = NetworkHandler::test_default();
        }

        Ok(app)
    }

    pub fn new(
        seed: Option<u64>,
        disable_network: bool,
        #[cfg(feature = "audio")] disable_audio: bool,
        generate_local_world: bool,
        reset_world: bool,
        seed_ip: Option<String>,
        store_prefix: Option<String>,
    ) -> AppResult<Self> {
        // If the reset_world flag is set, reset the world.
        if reset_world {
            reset().expect("Failed to reset world");
        }

        let store_prefix = store_prefix.unwrap_or("local".to_string());

        let ui = Ui::new(store_prefix.as_str(), disable_network);

        let (event_sender, event_receiver) = mpsc::channel(64);

        #[cfg(feature = "audio")]
        let audio_player = {
            {
                if disable_audio {
                    None
                } else if let Ok(player) = MusicPlayer::new(event_sender.clone()) {
                    info!("Audio player created succesfully");
                    Some(player)
                } else {
                    warn!("Could not create audio player");
                    None
                }
            }
        };

        let network_handler = NetworkHandler::new(seed_ip)?;

        Ok(Self {
            event_sender,
            event_receiver,
            world: World::new(seed),
            state: AppState::Running,
            ui,
            #[cfg(feature = "audio")]
            audio_player,
            generate_local_world,

            network_handler,
            store_prefix: store_prefix.to_string(),
            new_version_notified: false,
            cancellation_token: CancellationToken::new(),
            app_version: [
                env!("CARGO_PKG_VERSION_MAJOR").parse()?,
                env!("CARGO_PKG_VERSION_MINOR").parse()?,
                env!("CARGO_PKG_VERSION_PATCH").parse()?,
            ],
        })
    }

    pub async fn run<W: WriterProxy>(
        &mut self,
        mut tui: Tui<W>,
        network_port: Option<u16>,
    ) -> AppResult<()> {
        crossterm_event_handler::start_event_handler(
            self.get_event_sender(),
            self.cancellation_token.clone(),
        );

        let mut network_started = false;

        tick_event_handler::start_tick_event_loop(
            self.get_event_sender(),
            self.cancellation_token.clone(),
        );

        while self.state != AppState::Quitting {
            if self.state == AppState::Simulating {
                info!("Starting world simulation...");
                self.simulate_loaded_world(&mut tui).await;
                info!("...Done");
            }

            if !network_started && self.world.has_own_team() {
                if let Some(tcp_port) = network_port {
                    self.network_handler.start_polling_events(
                        self.get_event_sender(),
                        self.cancellation_token.clone(),
                        tcp_port,
                    );
                }
                network_started = true;
            }

            if let Some(app_event) = self.event_receiver.recv().await {
                match app_event {
                    AppEvent::SlowTick(tick) => {
                        self.handle_slow_tick_events(tick);
                        self.draw(&mut tui).await;
                    }
                    AppEvent::FastTick(tick) => {
                        if self.should_draw_fast_tick_events(tick) {
                            self.draw(&mut tui).await
                        }
                    }

                    AppEvent::TerminalEvent(terminal_event) => match terminal_event {
                        TerminalEvent::Key(key_event) => {
                            if self.should_draw_key_events(key_event)? {
                                self.draw(&mut tui).await;
                            }
                        }
                        TerminalEvent::Mouse(mouse_event) => {
                            if self.should_draw_mouse_events(mouse_event)? {
                                self.draw(&mut tui).await;
                            }
                        }
                        TerminalEvent::Resize(w, h) => {
                            tui.resize((w, h))?;
                            self.draw(&mut tui).await;
                        }
                        TerminalEvent::Quit => self.quit()?,
                    },

                    AppEvent::NetworkEvent(swarm_event) => {
                        self.handle_network_events(swarm_event)?;
                    }

                    #[cfg(feature = "audio")]
                    AppEvent::AudioEvent(streaming_data) => {
                        self.handle_audio_streaming_data(streaming_data)?;
                    }
                }
            }
        }
        self.cancellation_token.cancel();
        info!("Game loop closed");
        tui.exit().await?;
        Ok(())
    }

    pub fn notify_seed_version(&mut self, seed_version: [usize; 3]) -> AppResult<()> {
        if !self.new_version_notified {
            let [own_version_major, own_version_minor, own_version_patch] = self.app_version;
            let [version_major, version_minor, version_patch] = seed_version;
            if version_major > own_version_major
                || (version_major == own_version_major && version_minor > own_version_minor)
                || (version_major == own_version_major
                    && version_minor == own_version_minor
                    && version_patch > own_version_patch)
            {
                let message = format!(
                    "New version {version_major}.{version_minor}.{version_patch} available. Download at https://rebels.frittura.org",
                );
                self.ui.push_popup(PopupMessage::Ok {
                    message,
                    is_skippable: false,
                    tick: Tick::now(),
                });
                self.new_version_notified = true;
            }
        }
        Ok(())
    }

    pub fn new_world(&mut self) {
        if let Err(e) = self.world.initialize(self.generate_local_world) {
            panic!("Failed to initialize world: {e}");
        }
    }

    pub fn load_world(&mut self) {
        // Try to load an existing world.
        match load_world(&self.store_prefix) {
            Ok(w) => self.world = w,
            Err(e) => panic!("Failed to load world: {e}"),
        }

        let own_team = self
            .world
            .get_own_team_mut()
            .expect("Loaded world should have an own team.");

        if own_team.creation_time == Tick::default() {
            let mut creation_time = Tick::now();
            if let Ok(data) = world_file_data(&self.store_prefix) {
                if let Ok(time) = data.created() {
                    creation_time = Tick::from_system_time(time);
                }
            }
            own_team.creation_time = creation_time;
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

        Ok(())
    }

    async fn draw<W>(&mut self, tui: &mut Tui<W>)
    where
        W: WriterProxy,
    {
        if let Err(e) = tui
            .draw(
                &mut self.ui,
                &self.world,
                #[cfg(feature = "audio")]
                self.audio_player.as_ref(),
            )
            .await
        {
            error!("Error drawing TUI: {e}")
        };
    }

    fn should_draw_fast_tick_events(&mut self, current_tick: Tick) -> bool {
        match self.world.handle_fast_tick_events(current_tick) {
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
                    message: format!("Tick error\n{e}"),
                    tick: Tick::now(),
                });
            }
        }

        // FIXME: should get this info from the world, not hardcoded
        self.world.space_adventure.is_some()
    }

    fn handle_slow_tick_events(&mut self, current_tick: Tick) {
        // If there was a callback, or ui was updated --> draw.
        match self.world.handle_slow_tick_events(current_tick) {
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
                    message: format!("Tick error\n{e}"),
                    tick: Tick::now(),
                });
            }
        }

        match self.ui.update(
            &self.world,
            #[cfg(feature = "audio")]
            self.audio_player.as_ref(),
        ) {
            Ok(_) => {}
            Err(e) => {
                // We push to Logs rather than Error popup since otherwise it would spam too much
                self.ui
                    .push_log_event(Tick::now(), None, format!("Ui update error\n{e}"))
            }
        }
        self.world.dirty_ui = false;

        if !self.world.has_own_team() {
            return;
        }

        if self.world.dirty {
            self.world.dirty = false;
            save_world(&self.world, false, &self.store_prefix).expect("Failed to save world");
            self.world.serialized_size =
                get_world_size(&self.store_prefix).expect("Failed to get world size");

            self.ui.push_log_event(
                Tick::now(),
                None,
                format!("World saved, size: {} bytes", self.world.serialized_size),
            );
        }

        // Send own team to peers if dirty
        if self.world.dirty_network {
            self.world.dirty_network = false;
            if self.network_handler.connected_peers_count > 0 {
                if let Err(e) = self.network_handler.send_own_team(&self.world) {
                    self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Failed to send own team to peers: {e}"),
                    );
                }

                if let Err(e) = self.network_handler.send_open_trades(&self.world) {
                    self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Failed to send open trades to peers: {e}"),
                    );
                }
            } else if let Err(e) = self.network_handler.dial_seed() {
                self.ui
                    .push_log_event(Tick::now(), None, format!("Failed to dial seed: {e}"));
            }
        }
    }

    fn should_draw_key_events(&mut self, key_event: crossterm::event::KeyEvent) -> AppResult<bool> {
        let mut should_draw = false;
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

                    // Don't redraw during space adventure to keep consistent fps.
                    if self.world.space_adventure.is_none() {
                        should_draw = true;
                    }
                }
            }
        }
        Ok(should_draw)
    }

    fn should_draw_mouse_events(
        &mut self,
        mouse_event: crossterm::event::MouseEvent,
    ) -> AppResult<bool> {
        let mut should_draw = false;
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
            should_draw = true;
        }
        Ok(should_draw)
    }

    fn handle_network_events(
        &mut self,
        swarm_event: SwarmEvent<gossipsub::Event>,
    ) -> AppResult<()> {
        if let Some(callback) = self.network_handler.handle_network_events(swarm_event) {
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
                    self.ui.push_log_event(Tick::now(), None, e.to_string());
                }
            }
        }
        Ok(())
    }

    #[cfg(feature = "audio")]
    fn handle_audio_streaming_data(
        &mut self,
        data: StreamDownload<TempStorageProvider>,
    ) -> AppResult<()> {
        if let Some(audio_player) = &mut self.audio_player {
            audio_player.handle_streaming_ready(data)?;
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}
