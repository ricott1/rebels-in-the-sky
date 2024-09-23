use crate::audio;
use crate::audio::music_player::MusicPlayer;
use crate::event::{EventHandler, TerminalEvent};
use crate::network::handler::NetworkHandler;
use crate::store::{get_world_size, load_world, reset, save_world};
use crate::tui::Tui;
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::ui::popup_message::PopupMessage;
use crate::ui::ui::Ui;
use crate::ui::utils::SwarmPanelEvent;
use crate::world::constants::SECONDS;
use crate::world::world::World;
use crossterm::event::{KeyCode, KeyModifiers};
use futures::StreamExt;
use libp2p::{gossipsub, swarm::SwarmEvent};
use log::{error, info, warn};
use ratatui::backend::CrosstermBackend;
use std::io::{self};
use stream_download::storage::temp::TempStorageProvider;
use stream_download::StreamDownload;
use tokio::select;
use void::Void;

const NETWORK_HANDLER_INIT_INTERVAL: u128 = 10 * SECONDS;

pub struct App {
    pub world: World,
    pub running: bool,
    pub ui: Ui,
    pub audio_player: Option<MusicPlayer>,
    generate_local_world: bool,
    pub network_handler: Option<NetworkHandler>,
    seed_ip: Option<String>,
    network_port: Option<u16>,
    store_prefix: String,
}

impl App {
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
    ) -> Option<SwarmEvent<libp2p::gossipsub::Event, Void>> {
        match network_handler.as_mut() {
            Some(handler) => Some(handler.swarm.select_next_some().await),
            None => None,
        }
    }

    pub fn toggle_audio_player(&mut self) -> AppResult<()> {
        if let Some(player) = self.audio_player.as_mut() {
            player.toggle()?;
        } else {
            info!("No audio player, cannot toggle it");
        }
        Ok(())
    }

    pub fn next_sample_audio_player(&mut self) -> AppResult<()> {
        if let Some(player) = self.audio_player.as_mut() {
            player.next_audio_sample()?;
        } else {
            info!("No audio player, cannot select next sample");
        }
        Ok(())
    }

    pub fn initialize_network_handler(&mut self) -> AppResult<()> {
        let handler = NetworkHandler::new(self.seed_ip.clone(), self.network_port)?;
        self.network_handler = Some(handler);
        Ok(())
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
            // let try_audio_player =
            //     futures::executor::block_on(audio::music_player::MusicPlayer::new());
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
            running: true,
            ui,
            audio_player,
            generate_local_world,
            network_handler: None,
            seed_ip,
            network_port,
            store_prefix: store_prefix.to_string(),
        }
    }

    pub async fn run(&mut self) -> AppResult<()> {
        // Initialize the terminal user interface.
        let writer = io::stdout();
        let events = EventHandler::handler();
        let backend = CrosstermBackend::new(writer);
        let mut tui = Tui::new(backend, events)?;

        let mut last_network_handler_init = 0;

        while self.running {
            let now = Tick::now();
            if self.network_handler.is_none()
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
                // music_player = tokio::task::spawn(audio::music_player::MusicPlayer::play()).fuse() => self.audio_player = Some(music_player??),
                Some(streaming_data) = Self::conditional_audio_event(& self.audio_player) =>  self.handle_streaming_data(streaming_data)?,
                Some(swarm_event) = Self::conditional_network_event(&mut self.network_handler) =>  self.handle_network_events(swarm_event)?,
                app_event = tui.events.next()? => match app_event{
                    TerminalEvent::Tick {tick} => {
                            self.handle_tick_events(tick)?;
                            tui.draw(&mut self.ui, &self.world, self.audio_player.as_ref())?;
                    }
                    TerminalEvent::Key(key_event) => self.handle_key_events(key_event)?,
                    TerminalEvent::Mouse(mouse_event) => self.handle_mouse_events(mouse_event)?,
                    TerminalEvent::Resize(_, _) => {}
                }
            }
        }
        info!("Game loop closed");
        tui.exit()?;
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

        let simulation = self.world.simulate_until_now();

        match simulation {
            Ok(callbacks) => {
                for callback in callbacks.iter() {
                    match callback.call(self) {
                        Ok(Some(text)) => {
                            self.ui
                                .push_popup(PopupMessage::Ok(text, true, Tick::now()));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            panic!("Failed to load world: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                panic!("Failed to simulate world: {}", e);
            }
        }
        self.world.serialized_size =
            get_world_size(&self.store_prefix).expect("Failed to get world size");
    }

    /// Set running to false to quit the application.
    pub fn quit(&mut self) -> AppResult<()> {
        self.running = false;

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
    pub fn handle_tick_events(&mut self, current_timestamp: Tick) -> AppResult<()> {
        if self.world.has_own_team() {
            let tick_result = self.world.handle_tick_events(current_timestamp, false);

            match tick_result {
                Ok(callbacks) => {
                    for callback in callbacks.iter() {
                        match callback.call(self) {
                            Ok(Some(text)) => {
                                self.ui
                                    .push_popup(PopupMessage::Ok(text, true, Tick::now()));
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.ui.push_popup(PopupMessage::Error(
                                    e.to_string(),
                                    true,
                                    Tick::now(),
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    self.ui.push_popup(PopupMessage::Error(
                        format!("Tick error\n{}", e.to_string()),
                        true,
                        Tick::now(),
                    ));
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
            let mut own_team = self.world.get_own_team()?.clone();
            own_team.version += 1;
            self.world.teams.insert(own_team.id, own_team);
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
                }
            }
        }

        Ok(())
    }

    pub fn handle_key_events(&mut self, key_event: crossterm::event::KeyEvent) -> AppResult<()> {
        match key_event.code {
            KeyCode::Esc => {
                self.quit()?;
            }
            // Exit application on `Ctrl-C`
            KeyCode::Char('c') | KeyCode::Char('C')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                self.quit()?;
            }
            _ => {
                if let Some(callback) = self.ui.handle_key_events(key_event, &self.world) {
                    match callback.call(self) {
                        Ok(Some(text)) => {
                            self.ui
                                .push_popup(PopupMessage::Ok(text, true, Tick::now()));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.ui.push_popup(PopupMessage::Error(
                                e.to_string(),
                                true,
                                Tick::now(),
                            ));
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
                Ok(Some(cb)) => {
                    self.ui.push_popup(PopupMessage::Ok(cb, true, Tick::now()));
                }
                Ok(None) => {}
                Err(e) => {
                    self.ui
                        .push_popup(PopupMessage::Error(e.to_string(), true, Tick::now()));
                }
            }
        }
        Ok(())
    }

    pub fn handle_network_events(
        &mut self,
        network_event: SwarmEvent<gossipsub::Event, Void>,
    ) -> AppResult<()> {
        if let Some(network_handler) = &mut self.network_handler {
            if let Some(callback) = network_handler.handle_network_events(network_event) {
                match callback.call(self) {
                    Ok(Some(cb)) => {
                        self.ui.push_popup(PopupMessage::Ok(cb, true, Tick::now()));
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
