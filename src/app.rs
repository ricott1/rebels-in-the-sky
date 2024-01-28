// use core::panic;

use crate::event::TerminalEvent;
use crate::network::handler::NetworkHandler;
use crate::store::{reset, save_world};
use crate::tui::Tui;
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::ui::ui::Ui;
use crate::ui::utils::SwarmPanelEvent;
use crate::world::world::World;
use crossterm::event::{KeyCode, KeyModifiers};
use futures::StreamExt;
use libp2p::{gossipsub, swarm::SwarmEvent};
use ratatui::prelude::CrosstermBackend;
use tokio::select;
use void::Void;

pub struct App {
    pub world: World,
    pub running: bool,
    pub ui: Ui,
    generate_local_world: bool,
    pub network_handler: Option<NetworkHandler>,
}

impl App {
    pub fn new(
        seed: Option<u64>,
        disable_network: bool,
        disable_audio: bool,
        generate_local_world: bool,
        reset_world: bool,
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
        }
    }

    pub async fn run(&mut self, mut ratatui: Tui) -> AppResult<()> {
        ratatui.init()?;
        while self.running {
            if self.network_handler.is_none() && (self.world.has_own_team()) {
                self.initialize_network_handler();
            }
            //FIXME consolidate this into a single select! macro
            if self.network_handler.is_some() {
                select! {
                    //TODO: world_event = app.world_handler
                    swarm_event = self.network_handler.as_mut().unwrap().swarm.select_next_some() =>  self.handle_network_events(swarm_event)?,
                    app_event = ratatui.events.next()? => match app_event{
                        TerminalEvent::Tick {tick} => {
                                self.handle_tick_events(tick)?;
                                ratatui.draw(self)?;
                        }
                        TerminalEvent::Key(key_event) => self.handle_key_events(key_event)?,
                        TerminalEvent::Mouse(mouse_event) => self.handle_mouse_events(mouse_event)?,
                        TerminalEvent::Resize(_, _) => {}
                    }
                }
            } else {
                select! {
                    app_event = ratatui.events.next()? => match app_event{
                        TerminalEvent::Tick {tick} => {
                                self.handle_tick_events(tick)?;
                                ratatui.draw(self)?;
                        }
                        TerminalEvent::Key(key_event) => self.handle_key_events(key_event)?,
                        TerminalEvent::Mouse(mouse_event) => self.handle_mouse_events(mouse_event)?,
                        TerminalEvent::Resize(_, _) => {}
                    }
                }
            }
        }
        ratatui.exit()?;
        Ok(())
    }

    pub fn initialize_network_handler(&mut self) {
        let handler = NetworkHandler::new();
        if handler.is_err() {
            eprintln!("Failed to initialize network handler");
        } else {
            self.network_handler = Some(handler.unwrap());
        }
    }

    pub fn new_world(&mut self) {
        let initialize = self.world.initialize(self.generate_local_world);
        if initialize.is_err() {
            panic!("Failed to initialize world: {}", initialize.err().unwrap());
        }
    }

    pub fn load_world(&mut self) {
        // Try to load an existing world.
        let try_load = World::load();
        if let Ok(loaded_world) = try_load {
            self.world = loaded_world;
        } else {
            panic!("Failed to load world: {}", try_load.err().unwrap());
        }

        let simulation = self.world.simulate_until_now();

        match simulation {
            Ok(messages) => {
                for message in messages.iter() {
                    self.ui.set_popup(crate::ui::ui::PopupMessage::Ok(
                        message.clone(),
                        Tick::now(),
                    ));
                }
            }
            Err(_) => {
                panic!("Failed to simulate world");
            }
        }
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

    pub fn render(&mut self, frame: &mut ratatui::Frame<CrosstermBackend<std::io::Stdout>>) {
        self.ui.render(frame, &mut self.world);
    }

    /// Handles the tick event of the terminal.
    pub fn handle_tick_events(&mut self, current_timestamp: Tick) -> AppResult<()> {
        if self.world.has_own_team() {
            let tick_result = self.world.handle_tick_events(current_timestamp, false);

            match tick_result {
                Ok(messages) => {
                    for message in messages.iter() {
                        self.ui.set_popup(crate::ui::ui::PopupMessage::Ok(
                            message.clone(),
                            Tick::now(),
                        ));
                    }
                }
                Err(e) => {
                    self.ui.set_popup(crate::ui::ui::PopupMessage::Error(
                        format!("Tick error\n{}", e.to_string()),
                        Tick::now(),
                    ));
                }
            }
        }

        match self.ui.update(&self.world) {
            Ok(_) => {}
            Err(e) => {
                self.ui.set_popup(crate::ui::ui::PopupMessage::Error(
                    format!("Ui update error\n{}", e.to_string()),
                    Tick::now(),
                ));
            }
        }
        self.world.dirty_ui = false;

        if self.world.dirty && self.world.has_own_team() {
            self.world.dirty = false;
            let mut own_team = self.world.get_own_team()?.clone();
            own_team.version += 1;
            self.world.teams.insert(own_team.id, own_team);
            self.world.serialized_size =
                save_world(&self.world, false).expect("Failed to save world");

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
                    let send_own_team_result = network_handler.send_own_team(&self.world);
                    if send_own_team_result.is_err() {
                        self.ui.swarm_panel.push_log_event(SwarmPanelEvent {
                            timestamp: Tick::now(),
                            peer_id: None,
                            text: format!(
                                "Failed to send own team to peers: {}",
                                send_own_team_result.err().unwrap()
                            ),
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
                if let Some(callback) = self.ui.handle_key_events(key_event) {
                    match callback.call(self) {
                        Ok(Some(cb)) => {
                            self.ui
                                .set_popup(crate::ui::ui::PopupMessage::Ok(cb, Tick::now()));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.ui.set_popup(crate::ui::ui::PopupMessage::Error(
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
                        .set_popup(crate::ui::ui::PopupMessage::Ok(cb, Tick::now()));
                }
                Ok(None) => {}
                Err(e) => {
                    self.ui.set_popup(crate::ui::ui::PopupMessage::Error(
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
                            .set_popup(crate::ui::ui::PopupMessage::Ok(cb, Tick::now()));
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
