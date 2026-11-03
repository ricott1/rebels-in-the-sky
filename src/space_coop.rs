use crate::{
    app::App,
    network::space_link::{SpaceLinkEvent, SpaceLinkHandle},
    space_adventure::{
        session::{GuestEvent, GuestSession, HostEvent, HostSession, SpaceSession},
        wire::{EndReason, LinkSender, RejectReason, SessionMessage},
        PlayerInput,
    },
    types::{AppResult, SystemTimeTick, TeamId, Tick},
    ui::{PopupMessage, UiState},
};
use anyhow::anyhow;
use std::time::Instant;

const CONTACTING_HOST_MESSAGE: &str = "Contacting the host...";

pub(crate) fn contacting_host_popup() -> PopupMessage {
    PopupMessage::Message {
        message: CONTACTING_HOST_MESSAGE.to_string(),
        links: vec![],
        level: log::Level::Info,
        is_skippable: true,
        timestamp: Tick::now(),
    }
}

fn is_contacting_host(popup: &PopupMessage) -> bool {
    matches!(popup, PopupMessage::Message { message, .. } if message == CONTACTING_HOST_MESSAGE)
}

impl App {
    pub(crate) fn launch_space_adventure(&mut self, open: bool) -> AppResult<()> {
        if self.space_session.is_some() {
            return Err(anyhow!("Already joining a space adventure"));
        }
        self.world.start_space_adventure()?;
        if open {
            self.space_session = Some(SpaceSession::Host(HostSession::new()));
        }
        Ok(())
    }

    pub(crate) fn join_space_adventure(&mut self, host_team_id: TeamId) -> AppResult<()> {
        if self.space_session.is_some() || self.world.in_space() {
            return Err(anyhow!("Already on a space adventure"));
        }
        let (peer_id, join) = self.world.prepare_join(host_team_id)?;
        let loadout = join.loadout.clone();
        let link_id = self.network_handler.connect_space_link(
            peer_id,
            SessionMessage::Join(join),
            self.get_event_sender(),
        )?;
        self.space_session = Some(SpaceSession::Guest(GuestSession::new(
            link_id,
            host_team_id,
            loadout,
            Instant::now(),
        )));
        Ok(())
    }

    pub(crate) fn space_player_input(&mut self, input: PlayerInput) -> AppResult<()> {
        if let Some(SpaceSession::Guest(guest)) = self.space_session.as_mut() {
            guest.send_input(input, Instant::now());
            return Ok(());
        }
        if let Some(space) = self.world.space_adventure.as_mut() {
            if let Some(host_id) = space.host_id() {
                space.handle_player_input(host_id, input)?;
            }
        }
        Ok(())
    }

    pub(crate) fn leave_space_adventure(&mut self) {
        if let Some(SpaceSession::Guest(guest)) = self.space_session.as_mut() {
            if let Some(mirror) = self.world.space_mirror.as_mut() {
                if let Some(outcome) = guest.leave(Some(&*mirror)) {
                    mirror.start_ending(outcome);
                }
            }
            return;
        }
        if let Some(space) = self.world.space_adventure.as_mut() {
            space.stop_space_adventure();
        }
    }

    pub(crate) fn shutdown_space_session(&mut self) {
        match self.space_session.as_mut() {
            Some(SpaceSession::Host(host)) => {
                if let Some(space) = self.world.space_adventure.as_ref() {
                    host.shutdown(space);
                }
            }
            Some(SpaceSession::Guest(_)) => self.leave_space_adventure(),
            None => {}
        }
    }

    pub(crate) fn handle_space_link_event(&mut self, event: SpaceLinkEvent) -> AppResult<()> {
        let now = Instant::now();
        let mut host_events = vec![];
        let mut guest_events = vec![];

        match event {
            SpaceLinkEvent::Opened {
                link_id,
                handle,
                inbound: true,
                ..
            } => match self.space_session.as_mut() {
                Some(SpaceSession::Host(host)) => host.link_opened(link_id, Box::new(handle), now),
                _ => reject_closed(&handle),
            },
            SpaceLinkEvent::Opened {
                link_id,
                handle,
                inbound: false,
                ..
            } => {
                if let Some(SpaceSession::Guest(guest)) = self.space_session.as_mut() {
                    if guest.link_id() == link_id {
                        guest.link_opened(Box::new(handle), now);
                    }
                }
            }
            SpaceLinkEvent::Message { link_id, message } => match self.space_session.as_mut() {
                Some(SpaceSession::Host(host)) => {
                    let around = self.world.space_adventure_planet()?;
                    if let Some(space) = self.world.space_adventure.as_mut() {
                        host_events = host.handle_message(link_id, message, space, around, now);
                    }
                }
                Some(SpaceSession::Guest(guest)) if guest.link_id() == link_id => {
                    guest_events =
                        guest.handle_message(message, self.world.space_mirror.as_mut(), now);
                }
                _ => {}
            },
            SpaceLinkEvent::Closed { link_id, reason } => {
                log::info!("Space link {link_id} closed: {reason}");
                match self.space_session.as_mut() {
                    Some(SpaceSession::Host(host)) => {
                        if let Some(space) = self.world.space_adventure.as_mut() {
                            host_events = host.link_closed(link_id, space);
                        }
                    }
                    Some(SpaceSession::Guest(guest)) if guest.link_id() == link_id => {
                        guest_events = guest.link_closed(self.world.space_mirror.as_ref());
                    }
                    _ => {}
                }
            }
        }

        self.apply_host_events(host_events);
        self.apply_guest_events(guest_events)?;
        self.sync_joinable()
    }

    pub(crate) fn tick_space_session(&mut self) -> AppResult<()> {
        let now = Instant::now();
        let (host_events, guest_events) = match self.space_session.as_mut() {
            Some(SpaceSession::Host(host)) => match self.world.space_adventure.as_mut() {
                Some(space) => (host.tick(space, now), vec![]),
                None => (vec![], vec![]),
            },
            Some(SpaceSession::Guest(guest)) => {
                (vec![], guest.tick(self.world.space_mirror.as_ref(), now))
            }
            None => return Ok(()),
        };
        self.apply_host_events(host_events);
        self.apply_guest_events(guest_events)?;
        self.sync_joinable()
    }

    fn apply_host_events(&mut self, events: Vec<HostEvent>) {
        for event in events {
            let text = match event {
                HostEvent::GuestJoined { team_name } => {
                    format!("{team_name} joined your space adventure")
                }
                HostEvent::GuestLeft { team_name } => {
                    format!("{team_name} left your space adventure")
                }
                HostEvent::GuestDestroyed { team_name } => {
                    format!("{team_name}'s spaceship was destroyed")
                }
            };
            self.ui
                .push_log_event(Tick::now(), None, text, log::Level::Info);
        }
    }

    fn apply_guest_events(&mut self, events: Vec<GuestEvent>) -> AppResult<()> {
        for event in events {
            if matches!(event, GuestEvent::Welcomed(_) | GuestEvent::Rejected(_)) {
                self.ui.close_popup_where(is_contacting_host);
            }
            match event {
                GuestEvent::Welcomed(welcome) => {
                    let planet_id = self.world.get_own_team()?.is_on_planet();
                    let entered = planet_id
                        .ok_or_else(|| anyhow!("Team left the planet"))
                        .and_then(|planet_id| {
                            self.world.enter_guest_adventure(&welcome, planet_id)
                        });
                    if let Err(err) = entered {
                        if let Some(SpaceSession::Guest(guest)) = self.space_session.as_mut() {
                            guest.abort();
                        }
                        self.space_session = None;
                        self.ui.push_popup(PopupMessage::error(format!(
                            "Could not join the space adventure: {err}"
                        )));
                        continue;
                    }
                    self.ui.set_state(UiState::SpaceAdventure);
                }
                GuestEvent::Rejected(reason) => {
                    self.space_session = None;
                    self.ui.push_popup(PopupMessage::error(format!(
                        "Could not join the space adventure: {}",
                        reason.message()
                    )));
                }
                GuestEvent::Ended { reason, outcome } => {
                    if let Some(mirror) = self.world.space_mirror.as_mut() {
                        mirror.start_ending(outcome);
                    }
                    self.ui.push_popup(PopupMessage::Message {
                        message: reason.message().to_string(),
                        links: vec![],
                        level: if reason == EndReason::Destroyed {
                            log::Level::Warn
                        } else {
                            log::Level::Info
                        },
                        is_skippable: true,
                        timestamp: Tick::now(),
                    });
                }
            }
        }
        Ok(())
    }

    fn sync_joinable(&mut self) -> AppResult<()> {
        let joinable = match (&self.space_session, &self.world.space_adventure) {
            (Some(SpaceSession::Host(host)), Some(space)) => host.is_joinable(space),
            _ => false,
        };
        if self.world.space_adventure.is_some() {
            self.world.set_space_adventure_joinable(joinable)?;
        }
        Ok(())
    }
}

fn reject_closed(handle: &SpaceLinkHandle) {
    handle.send_control(SessionMessage::Reject {
        reason: RejectReason::Closed,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space_adventure::ShipLoadout;

    fn guest_app(popups: Vec<PopupMessage>) -> AppResult<App> {
        let mut app = App::test_default()?;
        for popup in popups {
            app.ui.push_popup(popup);
        }
        app.space_session = Some(SpaceSession::Guest(GuestSession::new(
            1,
            TeamId::new_v4(),
            ShipLoadout::test_default(),
            Instant::now(),
        )));
        Ok(app)
    }

    fn has_message(app: &App, text: &str) -> bool {
        app.ui
            .popup_messages()
            .iter()
            .any(|popup| matches!(popup, PopupMessage::Message { message, .. } if message == text))
    }

    #[test]
    fn test_join_answer_closes_only_the_contacting_popup() -> AppResult<()> {
        let game_result = PopupMessage::error("Game result".to_string());

        let mut app = guest_app(vec![game_result.clone(), contacting_host_popup()])?;
        app.apply_guest_events(vec![GuestEvent::Rejected(RejectReason::Full)])?;
        assert!(has_message(&app, "Game result"));
        assert!(!has_message(&app, CONTACTING_HOST_MESSAGE));

        let mut dismissed = guest_app(vec![game_result])?;
        dismissed.apply_guest_events(vec![GuestEvent::Rejected(RejectReason::Full)])?;
        assert!(has_message(&dismissed, "Game result"));
        Ok(())
    }
}
