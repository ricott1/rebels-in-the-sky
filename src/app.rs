use crate::args::AppArgs;
#[cfg(feature = "audio")]
use crate::audio::music_player::{MusicPlayer, MusicPlayerEvent};
use crate::network::handler::BehaviourEvent;
use crate::network::handler::NetworkHandler;
use crate::{
    core::*,
    crossterm_event_handler,
    store::{get_world_size, load_world, reset_store, save_world},
    tick_event_handler,
    tui::{TerminalEvent, Tui, WriterProxy},
    types::{AppResult, SystemTimeTick, Tick},
    ui::{
        PopupMessage, {UiScreen, UiState},
    },
};
use libp2p::identity::Keypair;
use libp2p::swarm::SwarmEvent;
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
    NetworkEvent(SwarmEvent<BehaviourEvent>),
    SpaceLink(crate::network::space_link::SpaceLinkEvent),
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
    pub space_session: Option<crate::space_adventure::session::SpaceSession>,
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
        let mut last_tui_update = Tick::now();
        log::info!(
            "Simulation started, must simulate {}",
            (Tick::now() - self.world.last_tick_short_interval).formatted()
        );

        let own_team = self
            .world
            .get_own_team()
            .expect("There should be an own team when simulating.");

        if let TeamLocation::OnSpaceAdventure { around, .. } = own_team.current_location {
            // If team is on a space adventure, bring it back to base planet.
            // This is an ad-hoc fix to avoid problems when the game is closed during a space adventure,
            // since the space property of the world is not serialized and stored.
            let own_team = self
                .world
                .get_own_team_mut()
                .expect("There should be an own team when simulating.");

            own_team.current_location = TeamLocation::OnPlanet { planet_id: around };

            self.ui
            .push_popup(PopupMessage::Message{
               message: "The game was closed during a space adventure.\nNext time go back to the base first!".to_string(),
               links: vec![],
               level: log::Level::Info,
               is_skippable:false,
               timestamp: Tick::now()
            });
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

            match self
                .world
                .handle_slow_tick_events(self.world.last_tick_short_interval + TickInterval::SHORT)
            {
                Ok(callbacks) => {
                    for callback in callbacks.iter() {
                        match callback.call(self) {
                            Ok(Some(message)) => self.ui.push_popup(PopupMessage::Message {
                                message,
                                links: vec![],
                                level: log::Level::Info,
                                is_skippable: true,
                                timestamp: self.world.last_tick_short_interval,
                            }),
                            Ok(None) => {}
                            Err(e) => {
                                panic!("Failed to simulate world: {e}");
                            }
                        }
                    }
                }
                Err(e) => panic!("Failed to simulate world: {e}"),
            };
        }

        self.world.serialized_size =
            get_world_size(self.args.store_prefix()).expect("Failed to get world size");

        self.state = AppState::Running;
        self.ui.set_state(UiState::Main);
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
            &mut ChaCha8Rng::from_rng(&mut rand::rng()),
            home_planet_id,
            "own team".into(),
            "ship_name".into(),
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
            space_session: None,
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
                    if let Some(bytes) = self.world.network_store_data.keypair.as_ref() {
                        if let Ok(keypair) = Keypair::from_protobuf_encoding(bytes) {
                            self.network_handler.set_keypair(keypair);
                            log::info!("Network keypair restored.")
                        } else {
                            log::error!("Could not restore network keypair.")
                        }
                    }
                    // Else do the opposite: store the new random keypair in the world
                    else {
                        self.world
                            .network_store_data
                            .set_keypair(self.network_handler.keypair_bytes()?);
                        log::info!("Network keypair persisted.")
                    }
                    self.network_handler.start_polling_events(
                        self.get_event_sender(),
                        self.get_cancellation_token(),
                        tcp_port,
                        self.args.use_ipv4(),
                        self.args.use_ipv6(),
                    );

                    self.ui
                        .swarm_panel
                        .add_peer_id(*self.network_handler.own_peer_id(), self.world.own_team_id);
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
                        let should_draw = self.should_draw_world_fast_tick_events(tick);
                        if let Err(e) = self.tick_space_session() {
                            self.ui.push_log_event(
                                Tick::now(),
                                None,
                                e.to_string(),
                                log::Level::Error,
                            );
                        }
                        if should_draw {
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

                    AppEvent::SpaceLink(event) => {
                        if let Err(e) = self.handle_space_link_event(event) {
                            self.ui.push_log_event(
                                Tick::now(),
                                None,
                                e.to_string(),
                                log::Level::Error,
                            );
                        }
                    }

                    #[cfg(feature = "audio")]
                    AppEvent::AudioEvent(audio_event) => match audio_event {
                        MusicPlayerEvent::StreamOk => {}
                        MusicPlayerEvent::StreamErr { error_message } => {
                            self.ui.push_popup(PopupMessage::error(format!(
                                "Music player error: {error_message}"
                            )));
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

    pub fn new_world(&mut self) {
        if let Err(e) = self.world.initialize(self.args.generate_local_world) {
            panic!("Failed to initialize world: {e}");
        }
    }

    pub fn continue_game(&mut self) {
        // Try to load an existing world.
        let (mut w, pending_callbacks) = match load_world(self.args.store_prefix()) {
            Ok(res) => res,
            Err(e) => panic!("Failed to load world: {e}"),
        };
        w.dirty_network = true;
        w.dirty_ui = true;
        self.world = w;

        if self.args.reset_network_peers {
            self.world.reset_network_store_peers();
        } else {
            let data = &self.world.network_store_data;
            self.ui
                .swarm_panel
                .update_team_ranking(&data.get_top_team_ranking());

            self.ui
                .swarm_panel
                .update_player_ranking(&data.get_top_player_ranking());

            self.ui.push_chat_history(&data.get_recent_chat_history());
        }

        for callback in pending_callbacks {
            let _ = callback.call(self);
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
                            self.ui.push_popup(PopupMessage::Message {
                                message,
                                links: vec![],
                                level: log::Level::Info,
                                is_skippable: true,
                                timestamp: Tick::now(),
                            });
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.ui.push_popup(PopupMessage::error(e.to_string()));
                        }
                    }
                }
            }
            Err(e) => {
                self.ui
                    .push_popup(PopupMessage::error(format!("Tick error\n{e}")));
            }
        }

        // FIXME: should get this info from the world, not hardcoded
        self.world.in_space()
    }

    fn handle_world_slow_tick_events(&mut self, current_tick: Tick) {
        // If there was a callback, or ui was updated --> draw.
        match self.world.handle_slow_tick_events(current_tick) {
            Ok(callbacks) => {
                for callback in callbacks.iter() {
                    match callback.call(self) {
                        Ok(Some(message)) => {
                            self.ui.push_popup(PopupMessage::Message {
                                message,
                                links: vec![],
                                level: log::Level::Info,
                                is_skippable: true,
                                timestamp: Tick::now(),
                            });
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.ui.push_popup(PopupMessage::error(e.to_string()));
                        }
                    }
                }
            }
            Err(e) => {
                self.ui
                    .push_popup(PopupMessage::error(format!("Tick error\n{e}")));
            }
        }

        self.ui.tick();
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

                match self.network_handler.resend_open_trades(&self.world) {
                    Ok(stale) => {
                        if let Ok(own_team) = self.world.get_own_team_mut() {
                            for id in stale {
                                own_team.sent_trades.remove(&id);
                            }
                        }
                    }
                    Err(e) => self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Failed to send open trades to peers: {e}"),
                        log::Level::Error,
                    ),
                }

                match self.network_handler.resend_open_challenges(&self.world) {
                    Ok(stale) => {
                        if let Ok(own_team) = self.world.get_own_team_mut() {
                            for id in stale {
                                own_team.sent_challenges.remove(&id);
                            }
                        }
                    }
                    Err(e) => self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Failed to send open challenges to peers: {e}"),
                        log::Level::Error,
                    ),
                }
            } else {
                // Not connected to anyone: try the seed AND every known peer, so the
                // relayer is not the only path back into the network.
                if let Err(e) = self.network_handler.dial_seed() {
                    self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Failed to dial seed: {e}"),
                        log::Level::Error,
                    );
                }
                if let Err(e) = self
                    .network_handler
                    .dial_known_peers(&self.world.network_store_data.peer_addresses)
                {
                    self.ui.push_log_event(
                        Tick::now(),
                        None,
                        format!("Failed to dial known peers: {e}"),
                        log::Level::Error,
                    );
                }
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
                            self.ui.push_popup(PopupMessage::Message {
                                message,
                                links: vec![],
                                level: log::Level::Info,
                                is_skippable: true,
                                timestamp: Tick::now(),
                            });
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.ui.push_popup(PopupMessage::error(e.to_string()));
                        }
                    }

                    // Don't redraw during space adventure to keep consistent fps.
                    if !self.world.in_space() {
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
                    self.ui.push_popup(PopupMessage::Message {
                        message,
                        links: vec![],
                        level: log::Level::Info,
                        is_skippable: true,
                        timestamp: Tick::now(),
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    self.ui.push_popup(PopupMessage::error(e.to_string()));
                }
            }
            should_draw = true;
        }
        Ok(should_draw)
    }

    fn handle_network_events(&mut self, swarm_event: SwarmEvent<BehaviourEvent>) -> AppResult<()> {
        if let Some(callback) = self.network_handler.handle_network_events(swarm_event) {
            match callback.call(self) {
                Ok(Some(message)) => {
                    self.ui.push_popup(PopupMessage::Message {
                        message,
                        links: vec![],
                        level: log::Level::Info,
                        is_skippable: true,
                        timestamp: Tick::now(),
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

#[cfg(test)]
mod coop_tests {
    use super::{App, AppEvent};
    use crate::{
        core::{resources::Resource, types::TeamLocation},
        space_adventure::{Body, PlayerInput},
        types::{AppResult, SystemTimeTick, TeamId, Tick},
        ui::UiCallback,
    };
    use anyhow::anyhow;
    use libp2p::{multiaddr::Protocol, swarm::SwarmEvent, Multiaddr};
    use std::time::{Duration, Instant};

    struct Peers {
        host: App,
        guest: App,
        ticker: tokio::time::Interval,
        host_address: Option<Multiaddr>,
        connected: bool,
    }

    fn is_loopback_tcp(address: &Multiaddr) -> bool {
        let mut protocols = address.iter();
        matches!(protocols.next(), Some(Protocol::Ip4(ip)) if ip.is_loopback())
            && matches!(protocols.next(), Some(Protocol::Tcp(_)))
    }

    fn fast_tick(app: &mut App) -> AppResult<()> {
        for callback in app.world.handle_fast_tick_events(Tick::now())? {
            callback.call(app)?;
        }
        app.tick_space_session()
    }

    impl Peers {
        async fn connected() -> AppResult<(Self, TeamId)> {
            let mut host = App::test_with_network_handler()?;
            let mut guest = App::test_with_network_handler()?;
            host.world
                .get_own_team_mut()?
                .add_resource(Resource::FUEL, 100)?;
            guest
                .world
                .get_own_team_mut()?
                .add_resource(Resource::FUEL, 100)?;
            let planet_id = host
                .world
                .get_own_team()?
                .is_on_planet()
                .ok_or_else(|| anyhow!("host should be on a planet"))?;
            guest.world.get_own_team_mut()?.current_location = TeamLocation::OnPlanet { planet_id };

            for app in [&mut host, &mut guest] {
                app.network_handler.start_polling_events(
                    app.get_event_sender(),
                    app.get_cancellation_token(),
                    0,
                    true,
                    false,
                );
            }

            UiCallback::StartSpaceAdventure { open: true }.call(&mut host)?;
            host.world
                .space_adventure
                .as_mut()
                .ok_or_else(|| anyhow!("host should be in space"))?
                .force_running();
            host.tick_space_session()?;
            let mut host_team = host.world.get_own_team()?.clone();
            host_team.peer_id = Some(*host.network_handler.own_peer_id());
            let host_team_id = host_team.id;
            guest.world.teams.insert(host_team_id, host_team);

            let mut peers = Self {
                host,
                guest,
                ticker: tokio::time::interval(Duration::from_millis(25)),
                host_address: None,
                connected: false,
            };
            peers
                .run_until(Duration::from_secs(10), |p| p.host_address.is_some())
                .await?;
            let address = peers.host_address.clone().expect("host address");
            peers.guest.network_handler.dial_address(address)?;
            peers
                .run_until(Duration::from_secs(10), |p| p.connected)
                .await?;
            Ok((peers, host_team_id))
        }

        async fn step(&mut self) -> AppResult<()> {
            let next = tokio::select! {
                Some(event) = self.host.event_receiver.recv() => Some((true, event)),
                Some(event) = self.guest.event_receiver.recv() => Some((false, event)),
                _ = self.ticker.tick() => None,
            };
            let host_peer_id = *self.host.network_handler.own_peer_id();
            match next {
                Some((true, AppEvent::SpaceLink(event))) => {
                    self.host.handle_space_link_event(event)?
                }
                Some((false, AppEvent::SpaceLink(event))) => {
                    self.guest.handle_space_link_event(event)?
                }
                Some((true, AppEvent::NetworkEvent(SwarmEvent::NewListenAddr { address, .. }))) => {
                    if self.host_address.is_none() && is_loopback_tcp(&address) {
                        self.host_address = Some(address);
                    }
                }
                Some((
                    false,
                    AppEvent::NetworkEvent(SwarmEvent::ConnectionEstablished { peer_id, .. }),
                )) => {
                    if peer_id == host_peer_id {
                        self.connected = true;
                    }
                }
                Some(_) => {}
                None => {
                    fast_tick(&mut self.host)?;
                    fast_tick(&mut self.guest)?;
                }
            }
            Ok(())
        }

        async fn run_until(
            &mut self,
            timeout: Duration,
            done: impl Fn(&Self) -> bool,
        ) -> AppResult<()> {
            let deadline = Instant::now() + timeout;
            while !done(self) {
                if Instant::now() > deadline {
                    return Err(anyhow!("timed out"));
                }
                self.step().await?;
            }
            Ok(())
        }

        async fn join(&mut self, host_team_id: TeamId) -> AppResult<()> {
            UiCallback::JoinSpaceAdventure { host_team_id }.call(&mut self.guest)?;
            self.run_until(Duration::from_secs(15), |p| {
                p.guest
                    .world
                    .space_mirror
                    .as_ref()
                    .is_some_and(|mirror| mirror.local_view().is_some())
            })
            .await
        }

        fn host_joinable(&self) -> bool {
            self.host.world.get_own_team().is_ok_and(|team| {
                matches!(
                    team.current_location,
                    TeamLocation::OnSpaceAdventure { joinable: true, .. }
                )
            })
        }
    }

    async fn pump_alone(
        app: &mut App,
        ticker: &mut tokio::time::Interval,
        timeout: Duration,
        done: impl Fn(&App) -> bool,
    ) -> AppResult<()> {
        let deadline = Instant::now() + timeout;
        while !done(app) {
            if Instant::now() > deadline {
                return Err(anyhow!("timed out"));
            }
            let next = tokio::select! {
                Some(event) = app.event_receiver.recv() => Some(event),
                _ = ticker.tick() => None,
            };
            match next {
                Some(AppEvent::SpaceLink(event)) => app.handle_space_link_event(event)?,
                Some(_) => {}
                None => fast_tick(app)?,
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_host_drops_a_guest_whose_game_dies() -> AppResult<()> {
        let (mut peers, host_team_id) = Peers::connected().await?;
        peers.join(host_team_id).await?;
        let Peers {
            mut host,
            guest,
            mut ticker,
            ..
        } = peers;
        drop(guest);

        pump_alone(&mut host, &mut ticker, Duration::from_secs(10), |app| {
            app.world
                .space_adventure
                .as_ref()
                .is_some_and(|space| space.guest_id().is_none())
        })
        .await?;
        assert!(matches!(
            host.world.get_own_team()?.current_location,
            TeamLocation::OnSpaceAdventure { joinable: true, .. }
        ));
        Ok(())
    }

    #[tokio::test]
    async fn test_guest_goes_home_when_the_host_game_dies() -> AppResult<()> {
        let (mut peers, host_team_id) = Peers::connected().await?;
        peers.join(host_team_id).await?;
        let Peers {
            host,
            mut guest,
            mut ticker,
            ..
        } = peers;
        drop(host);

        pump_alone(&mut guest, &mut ticker, Duration::from_secs(15), |app| {
            app.world.space_mirror.is_none() && app.space_session.is_none()
        })
        .await?;
        assert!(guest.world.get_own_team()?.is_on_planet().is_some());
        Ok(())
    }

    #[tokio::test]
    async fn test_guest_joins_flies_and_leaves_over_a_real_link() -> AppResult<()> {
        let (mut peers, host_team_id) = Peers::connected().await?;
        assert!(peers.host_joinable());

        peers.join(host_team_id).await?;
        let space = peers
            .host
            .world
            .space_adventure
            .as_ref()
            .expect("host space");
        let guest_ship_id = space.guest_id().expect("guest ship on the host");
        assert!(!peers.host_joinable());
        assert!(matches!(
            peers.guest.world.get_own_team()?.current_location,
            TeamLocation::OnSpaceAdventure {
                joinable: false,
                ..
            }
        ));

        peers.guest.space_player_input(PlayerInput::MoveDown)?;
        peers
            .run_until(Duration::from_secs(5), |p| {
                p.host
                    .world
                    .space_adventure
                    .as_ref()
                    .and_then(|space| space.get_ship(guest_ship_id))
                    .is_some_and(|ship| ship.velocity_f32().y > 0.0)
            })
            .await?;

        peers.guest.leave_space_adventure();
        peers
            .run_until(Duration::from_secs(10), |p| {
                p.guest.world.space_mirror.is_none() && p.host_joinable()
            })
            .await?;
        assert!(peers.guest.space_session.is_none());
        assert!(peers.guest.world.get_own_team()?.is_on_planet().is_some());
        assert!(peers
            .host
            .world
            .space_adventure
            .as_ref()
            .is_some_and(|space| space.guest_id().is_none()));
        Ok(())
    }

    #[tokio::test]
    async fn test_host_going_home_sends_the_guest_home_with_its_hold() -> AppResult<()> {
        let (mut peers, host_team_id) = Peers::connected().await?;
        peers.join(host_team_id).await?;
        let gold_before = peers
            .guest
            .world
            .get_own_team()?
            .resources
            .get(&Resource::GOLD)
            .copied()
            .unwrap_or_default();

        peers.host.leave_space_adventure();
        peers
            .run_until(Duration::from_secs(10), |p| {
                p.guest.world.space_mirror.is_none() && p.host.world.space_adventure.is_none()
            })
            .await?;
        assert!(peers.guest.space_session.is_none());
        assert!(peers.host.space_session.is_none());
        let guest_team = peers.guest.world.get_own_team()?;
        assert!(guest_team.is_on_planet().is_some());
        assert_eq!(
            guest_team
                .resources
                .get(&Resource::GOLD)
                .copied()
                .unwrap_or_default(),
            gold_before
        );
        Ok(())
    }
}
