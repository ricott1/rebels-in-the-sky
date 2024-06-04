use crate::event::{EventHandler, TerminalEvent};
use crate::network::handler::NetworkHandler;
use crate::store::{get_world_size, reset, save_world};
use crate::tui::Tui;
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::ui::ui::Ui;
use crate::ui::utils::SwarmPanelEvent;
use crate::world::world::World;
use crossterm::event::{KeyCode, KeyModifiers};
use futures::StreamExt;
use libp2p::{gossipsub, swarm::SwarmEvent};
use ratatui::backend::CrosstermBackend;
use std::io::{self};
use tokio::select;
use void::Void;

pub struct App {
    pub world: World,
    pub running: bool,
    pub ui: Ui,
    generate_local_world: bool,
    pub network_handler: Option<NetworkHandler>,
    pub seed_ip: Option<String>,
}

impl App {
    pub fn new(
        seed: Option<u64>,
        disable_network: bool,
        disable_audio: bool,
        generate_local_world: bool,
        reset_world: bool,
        seed_ip: Option<String>,
    ) -> Self {
        // If the reset_world flag is set, reset the world.
        if reset_world {
            reset().expect("Failed to reset world");
        }
        let ui = Ui::new(disable_network, disable_audio);
        Self {
            world: World::new(seed),
            running: true,
            ui,
            generate_local_world,
            network_handler: None,
            seed_ip,
        }
    }

    pub async fn run(&mut self) -> AppResult<()> {
        // Initialize the terminal user interface.
        let writer = io::stdout();
        let events = EventHandler::crossterm_handler();
        let backend = CrosstermBackend::new(writer);
        let mut tui = Tui::new(backend, events)?;

        while self.running {
            if self.network_handler.is_none() && (self.world.has_own_team()) {
                self.initialize_network_handler(self.seed_ip.clone());
            }
            //FIXME consolidate this into a single select! macro
            if let Some(network_handler) = self.network_handler.as_mut() {
                select! {
                    //TODO: world_event = app.world_handler
                    swarm_event = network_handler.swarm.select_next_some() =>  self.handle_network_events(swarm_event)?,
                    app_event = tui.events.next()? => match app_event{
                        TerminalEvent::Tick {tick} => {
                                self.handle_tick_events(tick)?;
                                tui.draw(self)?;
                        }
                        TerminalEvent::Key(key_event) => self.handle_key_events(key_event)?,
                        TerminalEvent::Mouse(mouse_event) => self.handle_mouse_events(mouse_event)?,
                        TerminalEvent::Resize(_, _) => {}
                    }
                }
            } else {
                select! {
                    app_event = tui.events.next()? => match app_event{
                        TerminalEvent::Tick {tick} => {
                                self.handle_tick_events(tick)?;
                                tui.draw(self)?;
                        }
                        TerminalEvent::Key(key_event) => self.handle_key_events(key_event)?,
                        TerminalEvent::Mouse(mouse_event) => self.handle_mouse_events(mouse_event)?,
                        TerminalEvent::Resize(_, _) => {}
                    }
                }
            }
        }
        tui.exit()?;
        Ok(())
    }

    pub fn initialize_network_handler(&mut self, seed_ip: Option<String>) {
        self.network_handler = NetworkHandler::new(seed_ip).ok();
    }

    pub fn new_world(&mut self) {
        if let Err(e) = self.world.initialize(self.generate_local_world) {
            panic!("Failed to initialize world: {}", e);
        }
    }

    pub fn load_world(&mut self) {
        // Try to load an existing world.
        match World::load() {
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
                                .set_popup(crate::ui::popup_message::PopupMessage::Ok(
                                    text,
                                    Tick::now(),
                                ));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            panic!("Failed to load world: {}", e);
                        }
                    }
                }
            }
            Err(_) => {
                panic!("Failed to simulate world");
            }
        }
        self.world.serialized_size = get_world_size().expect("Failed to get world size");
    }

    /// Set running to false to quit the application.
    pub fn quit(&mut self) -> AppResult<()> {
        self.running = false;
        // save world and backup
        if self.world.has_own_team() {
            save_world(&self.world, true)?;
        }
        Ok(())
    }

    pub fn render(&mut self, frame: &mut ratatui::Frame) {
        self.ui.render(frame, &mut self.world);
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
                                    .set_popup(crate::ui::popup_message::PopupMessage::Ok(
                                        text,
                                        Tick::now(),
                                    ));
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.ui
                                    .set_popup(crate::ui::popup_message::PopupMessage::Error(
                                        e.to_string(),
                                        Tick::now(),
                                    ));
                            }
                        }
                    }
                }
                Err(e) => {
                    self.ui
                        .set_popup(crate::ui::popup_message::PopupMessage::Error(
                            format!("Tick error\n{}", e.to_string()),
                            Tick::now(),
                        ));
                }
            }
        }

        match self.ui.update(&self.world) {
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
            save_world(&self.world, false).expect("Failed to save world");
            self.world.serialized_size = get_world_size().expect("Failed to get world size");

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
                                .set_popup(crate::ui::popup_message::PopupMessage::Ok(
                                    text,
                                    Tick::now(),
                                ));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.ui
                                .set_popup(crate::ui::popup_message::PopupMessage::Error(
                                    e.to_string(),
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
                    self.ui
                        .set_popup(crate::ui::popup_message::PopupMessage::Ok(cb, Tick::now()));
                }
                Ok(None) => {}
                Err(e) => {
                    self.ui
                        .set_popup(crate::ui::popup_message::PopupMessage::Error(
                            e.to_string(),
                            Tick::now(),
                        ));
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
                        self.ui
                            .set_popup(crate::ui::popup_message::PopupMessage::Ok(cb, Tick::now()));
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
}
