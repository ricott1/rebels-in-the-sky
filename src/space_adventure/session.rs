use super::{
    mirror::SpaceMirror,
    player::{PlayerOutcome, ShipLoadout},
    snapshot::SnapshotTracker,
    space::SpaceAdventure,
    wire::{EndReason, JoinRequest, LinkSender, RejectReason, SessionMessage, Welcome},
    PlayerInput,
};
use crate::types::{PlanetId, TeamId};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub const LINK_TIMEOUT: Duration = Duration::from_secs(5);
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
pub const JOIN_TIMEOUT: Duration = Duration::from_secs(10);
pub const SNAPSHOT_EVERY_FAST_TICKS: u64 = 2;

#[derive(Debug)]
pub enum SpaceSession {
    Host(HostSession),
    Guest(GuestSession),
}

pub fn validate_join(
    join: &JoinRequest,
    space: &SpaceAdventure,
    has_guest: bool,
    around: PlanetId,
) -> Result<(), RejectReason> {
    if !space.is_running() {
        return Err(RejectReason::NotRunning);
    }
    if has_guest {
        return Err(RejectReason::Full);
    }
    let [major, minor, _] = crate::app_version();
    if join.version[0] != major || join.version[1] != minor {
        return Err(RejectReason::VersionMismatch);
    }
    if join.planet_id != around {
        return Err(RejectReason::NotOnPlanet);
    }
    Ok(())
}

#[derive(Debug)]
pub enum HostEvent {
    GuestJoined { team_name: String },
    GuestLeft { team_name: String },
    GuestDestroyed { team_name: String },
}

#[derive(Debug)]
struct HostGuest {
    link_id: u64,
    link: Box<dyn LinkSender>,
    team_name: String,
    ship_id: usize,
    tracker: SnapshotTracker,
    last_input_seq: u64,
    last_heard: Instant,
}

#[derive(Debug, Default)]
pub struct HostSession {
    pending: HashMap<u64, (Box<dyn LinkSender>, Instant)>,
    guest: Option<HostGuest>,
    fast_ticks: u64,
    ended_sent: bool,
}

impl HostSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_guest(&self) -> bool {
        self.guest.is_some()
    }

    pub fn is_joinable(&self, space: &SpaceAdventure) -> bool {
        self.guest.is_none() && space.is_running()
    }

    pub fn link_opened(&mut self, link_id: u64, link: Box<dyn LinkSender>, now: Instant) {
        self.pending.insert(link_id, (link, now));
    }

    fn drop_guest(&mut self, space: &mut SpaceAdventure) -> Option<String> {
        let guest = self.guest.take()?;
        space.remove_guest();
        space.set_record_particles(false);
        Some(guest.team_name)
    }

    pub fn handle_message(
        &mut self,
        link_id: u64,
        message: SessionMessage,
        space: &mut SpaceAdventure,
        around: PlanetId,
        now: Instant,
    ) -> Vec<HostEvent> {
        if let Some((link, _)) = self.pending.remove(&link_id) {
            let SessionMessage::Join(join) = message else {
                return vec![];
            };
            if let Err(reason) = validate_join(&join, space, self.guest.is_some(), around) {
                link.send_control(SessionMessage::Reject { reason });
                return vec![];
            }
            let Ok(ship_id) = space.add_guest(&join.loadout) else {
                link.send_control(SessionMessage::Reject { reason: RejectReason::Full });
                return vec![];
            };
            space.set_record_particles(true);
            let mut tracker = SnapshotTracker::new();
            link.send_control(SessionMessage::Welcome(tracker.welcome(space, ship_id)));
            let team_name = join.team_name.clone();
            self.guest = Some(HostGuest {
                link_id,
                link,
                team_name: join.team_name,
                ship_id,
                tracker,
                last_input_seq: 0,
                last_heard: now,
            });
            return vec![HostEvent::GuestJoined { team_name }];
        }

        let Some(guest) = self.guest.as_mut().filter(|guest| guest.link_id == link_id) else {
            return vec![];
        };
        guest.last_heard = now;
        match message {
            SessionMessage::Input { seq, input } => {
                guest.last_input_seq = seq;
                let _ = space.handle_player_input(guest.ship_id, input);
                vec![]
            }
            SessionMessage::Heartbeat => vec![],
            _ => self
                .drop_guest(space)
                .map(|team_name| vec![HostEvent::GuestLeft { team_name }])
                .unwrap_or_default(),
        }
    }

    pub fn link_closed(&mut self, link_id: u64, space: &mut SpaceAdventure) -> Vec<HostEvent> {
        self.pending.remove(&link_id);
        if self.guest.as_ref().is_some_and(|guest| guest.link_id == link_id) {
            if let Some(team_name) = self.drop_guest(space) {
                return vec![HostEvent::GuestLeft { team_name }];
            }
        }
        vec![]
    }

    pub fn tick(&mut self, space: &mut SpaceAdventure, now: Instant) -> Vec<HostEvent> {
        self.pending
            .retain(|_, (_, opened)| now.saturating_duration_since(*opened) <= JOIN_TIMEOUT);

        let Some(guest) = self.guest.as_mut() else {
            return vec![];
        };

        if let Some(outcome) = space.take_guest_destroyed() {
            guest.link.send_control(SessionMessage::Ended {
                reason: EndReason::Destroyed,
                outcome,
            });
            let team_name = guest.team_name.clone();
            self.guest = None;
            space.set_record_particles(false);
            return vec![HostEvent::GuestDestroyed { team_name }];
        }

        if space.is_ending() {
            if !self.ended_sent {
                if let Some(outcome) = space.guest_outcome() {
                    guest.link.send_control(SessionMessage::Ended {
                        reason: EndReason::HostEnded,
                        outcome,
                    });
                }
                self.ended_sent = true;
            }
            return vec![];
        }

        if now.saturating_duration_since(guest.last_heard) > LINK_TIMEOUT {
            return self
                .drop_guest(space)
                .map(|team_name| vec![HostEvent::GuestLeft { team_name }])
                .unwrap_or_default();
        }

        self.fast_ticks += 1;
        guest.tracker.collect_particles(space);
        if self.fast_ticks.is_multiple_of(SNAPSHOT_EVERY_FAST_TICKS) {
            if let Some(snapshot) = guest.tracker.prepare(space, guest.ship_id, guest.last_input_seq) {
                if guest
                    .link
                    .try_send_snapshot(SessionMessage::Snapshot(snapshot.clone()))
                {
                    guest.tracker.commit(&snapshot);
                }
            }
        }
        vec![]
    }

    pub fn shutdown(&mut self, space: &SpaceAdventure) {
        if self.ended_sent {
            return;
        }
        if let (Some(guest), Some(outcome)) = (self.guest.as_ref(), space.guest_outcome()) {
            guest.link.send_control(SessionMessage::Ended {
                reason: EndReason::HostEnded,
                outcome,
            });
        }
        self.ended_sent = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuestState {
    Connecting,
    Joining,
    Flying,
    Ending,
}

#[derive(Debug)]
pub enum GuestEvent {
    Welcomed(Welcome),
    Rejected(RejectReason),
    Ended {
        reason: EndReason,
        outcome: PlayerOutcome,
    },
}

#[derive(Debug)]
pub struct GuestSession {
    link_id: u64,
    link: Option<Box<dyn LinkSender>>,
    host_team_id: TeamId,
    loadout: ShipLoadout,
    state: GuestState,
    next_seq: u64,
    started: Instant,
    last_heard: Instant,
    last_sent: Instant,
}

impl GuestSession {
    pub fn new(link_id: u64, host_team_id: TeamId, loadout: ShipLoadout, now: Instant) -> Self {
        Self {
            link_id,
            link: None,
            host_team_id,
            loadout,
            state: GuestState::Connecting,
            next_seq: 0,
            started: now,
            last_heard: now,
            last_sent: now,
        }
    }

    pub fn link_id(&self) -> u64 {
        self.link_id
    }

    pub fn host_team_id(&self) -> TeamId {
        self.host_team_id
    }

    pub fn is_flying(&self) -> bool {
        self.state == GuestState::Flying
    }

    fn send(&mut self, message: SessionMessage, now: Instant) {
        if let Some(link) = self.link.as_ref() {
            link.send_control(message);
            self.last_sent = now;
        }
    }

    fn last_outcome(&self, mirror: Option<&SpaceMirror>) -> PlayerOutcome {
        mirror
            .and_then(|mirror| mirror.last_outcome())
            .map(|outcome| outcome.capped(&self.loadout))
            .unwrap_or_else(|| self.loadout.outcome())
    }

    fn lost(&mut self, mirror: Option<&SpaceMirror>) -> Vec<GuestEvent> {
        match self.state {
            GuestState::Connecting | GuestState::Joining => {
                self.state = GuestState::Ending;
                vec![GuestEvent::Rejected(RejectReason::CantReachHost)]
            }
            GuestState::Flying => {
                self.state = GuestState::Ending;
                vec![GuestEvent::Ended {
                    reason: EndReason::LinkLost,
                    outcome: self.last_outcome(mirror),
                }]
            }
            GuestState::Ending => vec![],
        }
    }

    pub fn link_opened(&mut self, link: Box<dyn LinkSender>, now: Instant) {
        if self.state == GuestState::Connecting {
            self.link = Some(link);
            self.state = GuestState::Joining;
            self.started = now;
            self.last_sent = now;
        }
    }

    pub fn handle_message(
        &mut self,
        message: SessionMessage,
        mirror: Option<&mut SpaceMirror>,
        now: Instant,
    ) -> Vec<GuestEvent> {
        match (self.state, message) {
            (GuestState::Joining, SessionMessage::Welcome(welcome)) => {
                self.state = GuestState::Flying;
                self.last_heard = now;
                vec![GuestEvent::Welcomed(welcome)]
            }
            (GuestState::Connecting | GuestState::Joining, SessionMessage::Reject { reason }) => {
                self.state = GuestState::Ending;
                vec![GuestEvent::Rejected(reason)]
            }
            (GuestState::Flying, SessionMessage::Snapshot(snapshot)) => {
                self.last_heard = now;
                if let Some(mirror) = mirror {
                    if let Err(err) = mirror.apply(&snapshot) {
                        log::error!("Cannot apply space snapshot: {err}");
                    }
                }
                vec![]
            }
            (GuestState::Flying, SessionMessage::Ended { reason, outcome }) => {
                self.state = GuestState::Ending;
                vec![GuestEvent::Ended {
                    reason,
                    outcome: outcome.capped(&self.loadout),
                }]
            }
            (
                _,
                SessionMessage::Join(_)
                | SessionMessage::Input { .. }
                | SessionMessage::Heartbeat
                | SessionMessage::Leave,
            ) => {
                self.link = None;
                self.lost(mirror.as_deref())
            }
            _ => vec![],
        }
    }

    pub fn send_input(&mut self, input: PlayerInput, now: Instant) {
        if self.state != GuestState::Flying {
            return;
        }
        self.next_seq += 1;
        let seq = self.next_seq;
        self.send(SessionMessage::Input { seq, input }, now);
    }

    pub fn leave(&mut self, mirror: Option<&SpaceMirror>) -> Option<PlayerOutcome> {
        if self.state != GuestState::Flying {
            return None;
        }
        self.send(SessionMessage::Leave, Instant::now());
        self.state = GuestState::Ending;
        Some(self.last_outcome(mirror))
    }

    pub fn abort(&mut self) {
        self.send(SessionMessage::Leave, Instant::now());
        self.state = GuestState::Ending;
    }

    pub fn link_closed(&mut self, mirror: Option<&SpaceMirror>) -> Vec<GuestEvent> {
        self.link = None;
        self.lost(mirror)
    }

    pub fn tick(&mut self, mirror: Option<&SpaceMirror>, now: Instant) -> Vec<GuestEvent> {
        match self.state {
            GuestState::Connecting => vec![],
            GuestState::Joining => {
                if now.saturating_duration_since(self.started) > JOIN_TIMEOUT {
                    return self.lost(mirror);
                }
                vec![]
            }
            GuestState::Flying => {
                if now.saturating_duration_since(self.last_heard) > LINK_TIMEOUT {
                    return self.lost(mirror);
                }
                if now.saturating_duration_since(self.last_sent) >= HEARTBEAT_INTERVAL {
                    self.send(SessionMessage::Heartbeat, now);
                }
                vec![]
            }
            GuestState::Ending => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::resources::Resource;
    use crate::space_adventure::{wire::NetId, Body, PlayerInput, ShipLoadout};
    use crate::types::{AppResult, ResourceMap, StorableResourceMap};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };

    #[derive(Debug, Clone, Default)]
    struct TestLink {
        sent: Arc<Mutex<Vec<SessionMessage>>>,
        snapshots_full: Arc<AtomicBool>,
    }

    impl LinkSender for TestLink {
        fn send_control(&self, message: SessionMessage) -> bool {
            self.sent.lock().unwrap().push(message);
            true
        }

        fn try_send_snapshot(&self, message: SessionMessage) -> bool {
            if self.snapshots_full.load(Ordering::Relaxed) {
                return false;
            }
            self.sent.lock().unwrap().push(message);
            true
        }
    }

    impl TestLink {
        fn take(&self) -> Vec<SessionMessage> {
            std::mem::take(&mut *self.sent.lock().unwrap())
        }
    }

    fn planet() -> PlanetId {
        PlanetId::from_u128(42)
    }

    fn running_host() -> AppResult<SpaceAdventure> {
        let mut space = SpaceAdventure::new(false, 0.0)?.with_host(&ShipLoadout::test_default())?;
        space.force_running();
        Ok(space)
    }

    fn join(planet_id: PlanetId) -> SessionMessage {
        let mut loadout = ShipLoadout::test_default();
        loadout.resources.insert(Resource::GOLD, 10);
        loadout.resources.insert(Resource::SATOSHI, 500);
        SessionMessage::Join(JoinRequest {
            version: crate::app_version(),
            team_id: TeamId::from_u128(7),
            team_name: "Amarezza".to_string(),
            planet_id,
            loadout,
        })
    }

    fn joined_host(now: Instant) -> AppResult<(HostSession, SpaceAdventure, TestLink)> {
        let mut space = running_host()?;
        let mut host = HostSession::new();
        let link = TestLink::default();
        host.link_opened(1, Box::new(link.clone()), now);
        host.handle_message(1, join(planet()), &mut space, planet(), now);
        Ok((host, space, link))
    }

    #[test]
    fn test_host_accepts_a_valid_join() -> AppResult<()> {
        let now = Instant::now();
        let (host, space, link) = joined_host(now)?;
        assert!(host.has_guest());
        assert!(!host.is_joinable(&space));
        assert!(space.guest_id().is_some());
        assert!(matches!(link.take().as_slice(), [SessionMessage::Welcome(_)]));
        Ok(())
    }

    #[test]
    fn test_host_rejects_invalid_joins() -> AppResult<()> {
        let now = Instant::now();
        let reject = |space: &mut SpaceAdventure, host: &mut HostSession, message: SessionMessage, link_id: u64| {
            let link = TestLink::default();
            host.link_opened(link_id, Box::new(link.clone()), now);
            host.handle_message(link_id, message, space, planet(), now);
            link.take()
        };

        let mut starting = SpaceAdventure::new(false, 0.0)?.with_host(&ShipLoadout::test_default())?;
        let mut host = HostSession::new();
        assert_eq!(reject(&mut starting, &mut host, join(planet()), 1), vec![SessionMessage::Reject { reason: RejectReason::NotRunning }]);

        let mut space = running_host()?;
        let mut host = HostSession::new();
        assert_eq!(reject(&mut space, &mut host, join(PlanetId::from_u128(1)), 2), vec![SessionMessage::Reject { reason: RejectReason::NotOnPlanet }]);

        let SessionMessage::Join(mut old) = join(planet()) else { unreachable!() };
        old.version[1] += 1;
        assert_eq!(reject(&mut space, &mut host, SessionMessage::Join(old), 3), vec![SessionMessage::Reject { reason: RejectReason::VersionMismatch }]);

        assert!(matches!(reject(&mut space, &mut host, join(planet()), 4).as_slice(), [SessionMessage::Welcome(_)]));
        assert_eq!(reject(&mut space, &mut host, join(planet()), 5), vec![SessionMessage::Reject { reason: RejectReason::Full }]);
        Ok(())
    }

    #[test]
    fn test_host_applies_guest_input_and_reports_it() -> AppResult<()> {
        let now = Instant::now();
        let (mut host, mut space, link) = joined_host(now)?;
        link.take();
        host.handle_message(1, SessionMessage::Input { seq: 5, input: PlayerInput::MoveRight }, &mut space, planet(), now);
        space.update(0.025)?;
        space.update(0.025)?;
        let guest_id = space.guest_id().expect("guest");
        assert!(space.get_ship(guest_id).expect("guest").velocity_f32().x > 0.0);

        host.tick(&mut space, now);
        host.tick(&mut space, now);
        let snapshots: Vec<_> = link.take().into_iter().filter_map(|m| match m { SessionMessage::Snapshot(s) => Some(s), _ => None }).collect();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].last_input_seq, 5);
        Ok(())
    }

    #[test]
    fn test_host_snapshots_every_other_tick_and_resends_dropped_spawns() -> AppResult<()> {
        let now = Instant::now();
        let (mut host, mut space, link) = joined_host(now)?;
        link.take();
        link.snapshots_full.store(true, Ordering::Relaxed);
        let new_id = space.generate_asteroid(glam::Vec2::new(50.0, 50.0), glam::Vec2::ZERO, crate::space_adventure::asteroid::AsteroidSize::Small) as NetId;
        for _ in 0..4 {
            host.tick(&mut space, now);
        }
        assert!(link.take().is_empty());

        link.snapshots_full.store(false, Ordering::Relaxed);
        for _ in 0..4 {
            host.tick(&mut space, now);
        }
        let snapshots: Vec<_> = link.take().into_iter().filter_map(|m| match m { SessionMessage::Snapshot(s) => Some(s), _ => None }).collect();
        assert_eq!(snapshots.len(), 2);
        assert!(snapshots[0].spawned.iter().any(|s| s.id == new_id));
        assert!(!snapshots[1].spawned.iter().any(|s| s.id == new_id));
        Ok(())
    }

    #[test]
    fn test_host_handles_leave_close_and_timeout() -> AppResult<()> {
        let now = Instant::now();

        let (mut host, mut space, _) = joined_host(now)?;
        let events = host.handle_message(1, SessionMessage::Leave, &mut space, planet(), now);
        assert!(matches!(events.as_slice(), [HostEvent::GuestLeft { .. }]));
        assert!(space.guest_id().is_none());
        assert!(host.is_joinable(&space));

        let (mut host, mut space, _) = joined_host(now)?;
        assert!(matches!(host.link_closed(1, &mut space).as_slice(), [HostEvent::GuestLeft { .. }]));
        assert!(space.guest_id().is_none());

        let (mut host, mut space, _) = joined_host(now)?;
        host.handle_message(1, SessionMessage::Heartbeat, &mut space, planet(), now + Duration::from_secs(3));
        assert!(host.tick(&mut space, now + Duration::from_secs(7)).is_empty());
        assert!(matches!(host.tick(&mut space, now + Duration::from_secs(9)).as_slice(), [HostEvent::GuestLeft { .. }]));
        Ok(())
    }

    #[test]
    fn test_host_drops_a_guest_that_breaks_the_protocol() -> AppResult<()> {
        let now = Instant::now();
        let (mut host, mut space, _) = joined_host(now)?;
        let events = host.handle_message(1, SessionMessage::Reject { reason: RejectReason::Full }, &mut space, planet(), now);
        assert!(matches!(events.as_slice(), [HostEvent::GuestLeft { .. }]));
        assert!(space.guest_id().is_none());
        Ok(())
    }

    #[test]
    fn test_host_forgets_links_that_never_join() -> AppResult<()> {
        let now = Instant::now();
        let mut space = running_host()?;
        let mut host = HostSession::new();
        let link = TestLink::default();
        host.link_opened(1, Box::new(link.clone()), now);
        let later = now + JOIN_TIMEOUT + Duration::from_secs(1);
        host.tick(&mut space, later);
        host.handle_message(1, join(planet()), &mut space, planet(), later);
        assert!(!host.has_guest());
        assert!(link.take().is_empty());
        Ok(())
    }

    #[test]
    fn test_host_reports_guest_death() -> AppResult<()> {
        let now = Instant::now();
        let (mut host, mut space, link) = joined_host(now)?;
        link.take();
        let guest_id = space.guest_id().expect("guest");
        space.get_ship_mut(guest_id).expect("guest").set_invulnerable(0.0);
        crate::space_adventure::SpaceCallback::DamageEntity { id: guest_id, damage: 10_000.0 }.call(&mut space);
        space.update(0.025)?;

        let events = host.tick(&mut space, now);
        assert!(matches!(events.as_slice(), [HostEvent::GuestDestroyed { .. }]));
        let sent = link.take();
        let [SessionMessage::Ended { reason: EndReason::Destroyed, outcome }] = sent.as_slice() else {
            panic!("expected one Ended, got {sent:?}");
        };
        assert_eq!(outcome.durability, 0);
        assert_eq!(outcome.resources.value(&Resource::GOLD), 0);
        Ok(())
    }

    #[test]
    fn test_guest_death_and_host_ending_on_the_same_tick_send_one_ended() -> AppResult<()> {
        let now = Instant::now();
        let (mut host, mut space, link) = joined_host(now)?;
        link.take();
        let guest_id = space.guest_id().expect("guest");
        space.get_ship_mut(guest_id).expect("guest").set_invulnerable(0.0);
        crate::space_adventure::SpaceCallback::DamageEntity { id: guest_id, damage: 10_000.0 }.call(&mut space);
        space.update(0.025)?;
        space.stop_space_adventure();

        host.tick(&mut space, now);
        host.tick(&mut space, now);
        let ended: Vec<_> = link.take().into_iter().filter(|m| matches!(m, SessionMessage::Ended { .. })).collect();
        assert!(matches!(ended.as_slice(), [SessionMessage::Ended { reason: EndReason::Destroyed, .. }]));
        Ok(())
    }

    #[test]
    fn test_host_ending_sends_the_guest_hold_once() -> AppResult<()> {
        let now = Instant::now();
        let (mut host, mut space, link) = joined_host(now)?;
        link.take();
        space.stop_space_adventure();
        host.tick(&mut space, now);
        host.tick(&mut space, now);
        let sent = link.take();
        let [SessionMessage::Ended { reason: EndReason::HostEnded, outcome }] = sent.as_slice() else {
            panic!("expected one Ended, got {sent:?}");
        };
        assert_eq!(outcome.resources.value(&Resource::GOLD), 10);
        Ok(())
    }

    #[test]
    fn test_host_shutdown_tells_the_guest() -> AppResult<()> {
        let now = Instant::now();
        let (mut host, space, link) = joined_host(now)?;
        link.take();
        host.shutdown(&space);
        host.shutdown(&space);
        assert!(matches!(link.take().as_slice(), [SessionMessage::Ended { reason: EndReason::HostEnded, .. }]));
        Ok(())
    }

    fn welcome_and_mirror() -> AppResult<(Welcome, SpaceMirror)> {
        let mut space = running_host()?;
        let guest_id = space.add_guest(&ShipLoadout::test_default())?;
        let welcome = crate::space_adventure::snapshot::SnapshotTracker::new().welcome(&space, guest_id);
        let mirror = SpaceMirror::new(&welcome)?;
        Ok((welcome, mirror))
    }

    fn guest_loadout() -> ShipLoadout {
        let mut loadout = ShipLoadout::test_default();
        loadout.resources.insert(Resource::SATOSHI, 500);
        loadout.resources.insert(Resource::FUEL, 100);
        loadout
    }

    fn flying_guest(now: Instant) -> AppResult<(GuestSession, SpaceMirror, TestLink)> {
        let (welcome, mut mirror) = welcome_and_mirror()?;
        let mut guest = GuestSession::new(3, TeamId::from_u128(1), guest_loadout(), now);
        let link = TestLink::default();
        guest.link_opened(Box::new(link.clone()), now);
        let events = guest.handle_message(SessionMessage::Welcome(welcome), Some(&mut mirror), now);
        assert!(matches!(events.as_slice(), [GuestEvent::Welcomed(_)]));
        assert!(guest.is_flying());
        Ok((guest, mirror, link))
    }

    #[test]
    fn test_guest_connect_failures_are_rejections() {
        let now = Instant::now();
        let mut guest = GuestSession::new(3, TeamId::from_u128(1), guest_loadout(), now);
        assert!(matches!(guest.link_closed(None).as_slice(), [GuestEvent::Rejected(RejectReason::CantReachHost)]));

        let mut guest = GuestSession::new(3, TeamId::from_u128(1), guest_loadout(), now);
        guest.link_opened(Box::new(TestLink::default()), now);
        let events = guest.handle_message(SessionMessage::Reject { reason: RejectReason::Full }, None, now);
        assert!(matches!(events.as_slice(), [GuestEvent::Rejected(RejectReason::Full)]));

        let mut guest = GuestSession::new(3, TeamId::from_u128(1), guest_loadout(), now);
        guest.link_opened(Box::new(TestLink::default()), now);
        assert!(matches!(guest.tick(None, now + JOIN_TIMEOUT + Duration::from_secs(1)).as_slice(), [GuestEvent::Rejected(RejectReason::CantReachHost)]));
    }

    #[test]
    fn test_guest_input_heartbeat_and_timeout() -> AppResult<()> {
        let now = Instant::now();
        let (mut guest, mirror, link) = flying_guest(now)?;
        guest.send_input(PlayerInput::Shoot, now);
        guest.send_input(PlayerInput::Shoot, now);
        assert_eq!(
            link.take(),
            vec![
                SessionMessage::Input { seq: 1, input: PlayerInput::Shoot },
                SessionMessage::Input { seq: 2, input: PlayerInput::Shoot },
            ]
        );
        assert!(guest.tick(Some(&mirror), now + Duration::from_millis(500)).is_empty());
        assert!(link.take().is_empty());
        guest.tick(Some(&mirror), now + Duration::from_millis(1100));
        assert_eq!(link.take(), vec![SessionMessage::Heartbeat]);

        let events = guest.tick(Some(&mirror), now + Duration::from_secs(6));
        assert!(matches!(events.as_slice(), [GuestEvent::Ended { reason: EndReason::LinkLost, .. }]));
        Ok(())
    }

    #[test]
    fn test_guest_leaving_before_any_snapshot_keeps_the_join_hold() -> AppResult<()> {
        let now = Instant::now();
        let (mut guest, mirror, link) = flying_guest(now)?;
        let outcome = guest.leave(Some(&mirror)).expect("outcome");
        assert_eq!(outcome, guest_loadout().outcome());
        assert_eq!(link.take(), vec![SessionMessage::Leave]);
        assert!(guest.leave(Some(&mirror)).is_none());
        Ok(())
    }

    #[test]
    fn test_guest_caps_what_the_host_reports() -> AppResult<()> {
        let now = Instant::now();
        let (mut guest, mut mirror, _) = flying_guest(now)?;
        let loadout = guest_loadout();
        let mut resources = ResourceMap::new();
        resources.insert(Resource::SATOSHI, 1_000_000);
        resources.insert(Resource::FUEL, 1_000);
        resources.insert(Resource::SCRAPS, loadout.spaceship.storage_capacity() * 10);
        let reported = PlayerOutcome { resources, durability: 10_000 };

        let events = guest.handle_message(
            SessionMessage::Ended { reason: EndReason::HostEnded, outcome: reported },
            Some(&mut mirror),
            now,
        );
        let [GuestEvent::Ended { outcome, .. }] = events.as_slice() else {
            panic!("expected Ended, got {events:?}");
        };
        assert_eq!(outcome.resources.value(&Resource::SATOSHI), 500);
        assert_eq!(outcome.resources.value(&Resource::FUEL), 100);
        assert_eq!(outcome.resources.value(&Resource::SCRAPS), 0);
        assert_eq!(outcome.durability, loadout.spaceship.current_durability());
        Ok(())
    }

    #[test]
    fn test_guest_closes_on_unexpected_messages() -> AppResult<()> {
        let now = Instant::now();
        let (mut guest, mut mirror, _) = flying_guest(now)?;
        let events = guest.handle_message(SessionMessage::Heartbeat, Some(&mut mirror), now);
        assert!(matches!(events.as_slice(), [GuestEvent::Ended { reason: EndReason::LinkLost, .. }]));
        Ok(())
    }

    #[test]
    fn test_guest_link_lost_settles_from_the_last_view() -> AppResult<()> {
        let now = Instant::now();
        let (mut guest, mirror, _) = flying_guest(now)?;
        let events = guest.link_closed(Some(&mirror));
        assert!(matches!(events.as_slice(), [GuestEvent::Ended { reason: EndReason::LinkLost, .. }]));
        assert!(guest.link_closed(Some(&mirror)).is_empty());
        Ok(())
    }
}
