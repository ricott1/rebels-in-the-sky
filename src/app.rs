use crate::app_version;
use crate::args::AppArgs;
#[cfg(feature = "audio")]
use crate::audio::music_player::{MusicPlayer, MusicPlayerEvent};
use crate::network::handler::NetworkHandler;
use crate::{
    core::*,
    crossterm_event_handler,
    store::{get_world_size, load_world, reset_store, save_world, world_file_data},
    tick_event_handler,
    tui::{TerminalEvent, Tui, WriterProxy},
    types::{AppResult, ResourceMap, StorableResourceMap, SystemTimeTick, Tick},
    ui::{
        PopupMessage, {UiScreen, UiState},
    },
};
use libp2p::identity::Keypair;
use libp2p::{gossipsub, swarm::SwarmEvent};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ratatui::crossterm;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
    AudioEvent(MusicPlayerEvent),
}

#[derive(Debug)]
pub struct App {
    args: AppArgs,
    event_sender: mpsc::Sender<AppEvent>,
    event_receiver: mpsc::Receiver<AppEvent>,
    pub world: World,
    pub state: AppState,
    pub ui: UiScreen,
    #[cfg(feature = "audio")]
    pub audio_player: Option<MusicPlayer>,
    pub network_handler: NetworkHandler,
    new_version_notified: bool,
    cancellation_token: CancellationToken,
}

impl App {
    pub fn get_event_sender(&self) -> mpsc::Sender<AppEvent> {
        self.event_sender.clone()
    }

    pub fn get_cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub async fn simulate_loaded_world<W: WriterProxy>(&mut self, tui: &mut Tui<W>) {
        let mut callbacks = vec![];
        let mut last_tui_update = Tick::now();
        log::info!(
            "Simulation started, must simulate {}",
            (Tick::now() - self.world.last_tick_short_interval).formatted()
        );

        let own_team = self
            .world
            .get_own_team()
            .expect("There should be an own team when simulating.");

        if let TeamLocation::OnSpaceAdventure { around } = own_team.current_location {
            // If team is on a space adventure, bring it back to base planet.
            // This is an ad-hoc fix to avoid problems when the game is closed during a space adventure,
            // since the space property of the world is not serialized and stored.
            let own_team = self
                .world
                .get_own_team_mut()
                .expect("There should be an own team when simulating.");
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
                    log::error!("Error updating TUI during simulation: {e}")
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
            get_world_size(self.args.store_prefix()).expect("Failed to get world size");

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
        let mut app = App::new(AppArgs::test())?;
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
        let mut app = App::test_default()?;
        app.network_handler = NetworkHandler::test_default();

        Ok(app)
    }

    pub fn new(args: AppArgs) -> AppResult<Self> {
        // If the reset_world flag is set, reset the world.
        if args.reset_world {
            reset_store().expect("Failed to reset world");
        }

        let ui = UiScreen::new(args.store_prefix(), args.is_network_disabled());
        let (event_sender, event_receiver) = mpsc::channel(64);

        #[cfg(feature = "audio")]
        let audio_player = {
            if args.is_audio_disabled() {
                log::info!("Audio disabled, skipping music player creation.");
                None
            } else {
                match MusicPlayer::new() {
                    Ok(player) => {
                        log::info!("Music player created succesfully.");
                        Some(player)
                    }

                    Err(err) => {
                        log::warn!("Could not create music player: {err}.");
                        None
                    }
                }
            }
        };

        let network_handler = NetworkHandler::new(args.seed_node_ip.as_ref())?;
        let random_seed = args.random_seed;

        Ok(Self {
            args,
            event_sender,
            event_receiver,
            world: World::new(random_seed),
            state: AppState::Running,
            ui,
            #[cfg(feature = "audio")]
            audio_player,
            network_handler,
            new_version_notified: false,
            cancellation_token: CancellationToken::new(),
        })
    }

    pub async fn run<W: WriterProxy>(&mut self, mut tui: Tui<W>) -> AppResult<()> {
        if self.args.is_ui_disabled() {
            // With no UI, world must be loaded from file.
            self.continue_game();
        }

        crossterm_event_handler::start_event_handler(
            self.get_event_sender(),
            self.get_cancellation_token(),
        );

        tick_event_handler::start_tick_event_loop(
            self.get_event_sender(),
            self.get_cancellation_token(),
        );

        #[cfg(feature = "audio")]
        {
            let cancellation_token = self.get_cancellation_token();
            if let Some(player) = self.audio_player.as_mut() {
                if let Err(err) = player.start_audio_event_loop(cancellation_token) {
                    self.audio_player = None;
                    log::error!("Error starting audio event loop: {err}");
                }
            }
        }

        let mut last_user_input = Instant::now();
        let mut network_started = false;

        while self.state != AppState::Quitting {
            if self.state == AppState::Simulating {
                log::info!("Starting world simulation...");
                self.simulate_loaded_world(&mut tui).await;
                log::info!("...Done");
            }

            if !network_started && self.world.has_own_team() {
                if let Some(tcp_port) = self.args.network_port() {
                    // If world keypair bytes are set --> restore the network handler keypair
                    if let Some(bytes) = self.world.network_keypair.as_ref() {
                        if let Ok(keypair) = Keypair::from_protobuf_encoding(bytes) {
                            self.network_handler.set_keypair(keypair);
                            log::info!("Network keypair restored.")
                        } else {
                            log::error!("Could not restore network keypair.")
                        }
                    }
                    // Else do the opposite: store the new random keypair in the world
                    else {
                        self.world.network_keypair = Some(self.network_handler.keypair_bytes()?);
                        log::info!("Network keypair persisted.")
                    }
                    self.network_handler.start_polling_events(
                        self.get_event_sender(),
                        self.get_cancellation_token(),
                        tcp_port,
                        self.args.use_ipv4(),
                        self.args.use_ipv6(),
                    );
                }
                network_started = true;
            }

            if let Some(duration_in_seconds) = self.args.auto_quit_after {
                let duration = Duration::from_secs(duration_in_seconds);
                if last_user_input.elapsed() >= duration {
                    self.quit()?;
                }
            }

            if let Some(app_event) = self.event_receiver.recv().await {
                match app_event {
                    AppEvent::SlowTick(tick) => {
                        self.handle_world_slow_tick_events(tick);
                        self.draw(&mut tui).await;
                    }
                    AppEvent::FastTick(tick) => {
                        if self.should_draw_world_fast_tick_events(tick) {
                            self.draw(&mut tui).await
                        }
                    }

                    AppEvent::TerminalEvent(terminal_event) => {
                        match terminal_event {
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
                        };
                        last_user_input = Instant::now()
                    }

                    AppEvent::NetworkEvent(swarm_event) => {
                        self.handle_network_events(swarm_event)?;
                    }

                    #[cfg(feature = "audio")]
                    AppEvent::AudioEvent(audio_event) => match audio_event {
                        MusicPlayerEvent::StreamOk => {}
                        MusicPlayerEvent::StreamErr { error_message } => {
                            self.ui.push_popup(PopupMessage::Error {
                                message: format!("Music player error: {error_message}"),
                                tick: Tick::now(),
                            });
                        }
                    },
                }
            }
        }
        self.cancellation_token.cancel();
        log::info!("Game loop closed");
        tui.exit().await?;
        Ok(())
    }

    pub fn notify_seed_version(&mut self, seed_version: [usize; 3]) -> AppResult<()> {
        if !self.new_version_notified {
            let [own_version_major, own_version_minor, own_version_patch] = app_version();
            let [version_major, version_minor, version_patch] = seed_version;
            if version_major > own_version_major
                || (version_major == own_version_major && version_minor > own_version_minor)
                || (version_major == own_version_major
                    && version_minor == own_version_minor
                    && version_patch > own_version_patch)
            {
                let message = format!(
                    "New version {version_major}.{version_minor}.{version_patch} available. \nDownload at https://rebels.frittura.org",
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
        if let Err(e) = self.world.initialize(self.args.generate_local_world) {
            panic!("Failed to initialize world: {e}");
        }
    }

    pub fn continue_game(&mut self) {
        // Try to load an existing world.
        match load_world(self.args.store_prefix()) {
            Ok(mut w) => {
                w.dirty_network = true;
                w.dirty_ui = true;
                self.world = w;
            }
            Err(e) => panic!("Failed to load world: {e}"),
        }

        let own_team = self
            .world
            .get_own_team_mut()
            .expect("Loaded world should have an own team.");

        if own_team.creation_time == Tick::default() {
            let mut creation_time = Tick::now();
            if let Ok(data) = world_file_data(self.args.store_prefix()) {
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
            save_world(
                &self.world,
                self.args.store_prefix(),
                true,
                self.args.store_uncompressed,
            )?;
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
            log::error!("Error drawing TUI: {e}")
        };
    }

    fn should_draw_world_fast_tick_events(&mut self, current_tick: Tick) -> bool {
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

    fn handle_world_slow_tick_events(&mut self, current_tick: Tick) {
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
                self.ui.push_log_event(
                    Tick::now(),
                    None,
                    format!("UiScreen update error: {e}"),
                    log::Level::Error,
                )
            }
        }
        self.world.dirty_ui = false;

        if !self.world.has_own_team() {
            return;
        }

        if self.world.dirty {
            self.world.dirty = false;
            if let Err(e) = save_world(&self.world, self.args.store_prefix(), false, false) {
                log::error!("Failed to save world: {e}");
            }
            self.world.serialized_size =
                get_world_size(self.args.store_prefix()).expect("Failed to get world size");

            self.ui.push_log_event(
                Tick::now(),
                None,
                format!("World saved ({} KB)", self.world.serialized_size / 1024),
                log::Level::Info,
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
                        log::Level::Error,
                    );
                }

                if let Err(err) = self.network_handler.resend_tournaments(&self.world) {
                    self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Cannot send tournament: {err}"),
                        log::Level::Error,
                    );
                }

                if let Err(e) = self.network_handler.resend_open_trades(&self.world) {
                    self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Failed to send open trades to peers: {e}"),
                        log::Level::Error,
                    );
                }

                if let Err(e) = self.network_handler.resend_open_challenges(&self.world) {
                    self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Failed to send open challenges to peers: {e}"),
                        log::Level::Error,
                    );
                }
            } else if let Err(e) = self.network_handler.dial_seed() {
                self.ui.push_log_event(
                    Tick::now(),
                    None,
                    format!("Failed to dial seed: {e}"),
                    log::Level::Error,
                );
            }
        }
    }

    fn should_draw_key_events(&mut self, key_event: crossterm::event::KeyEvent) -> AppResult<bool> {
        let mut should_draw = false;
        match key_event.code {
            // Exit application directly on `Ctrl-C`. `Esc` asks for confirmation first.
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
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
                    self.ui
                        .push_log_event(Tick::now(), None, e.to_string(), log::Level::Error);
                }
            }
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}
