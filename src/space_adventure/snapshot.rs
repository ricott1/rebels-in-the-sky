use super::{
    entity::Entity,
    space::SpaceAdventure,
    traits::*,
    wire::{
        EntityLook, EntitySpawn, NetEntityState, NetId, NetVec, ParticleSpawn, Snapshot, SpawnKind,
        Welcome,
    },
};
use glam::Vec2;
use std::collections::{HashMap, HashSet};

const MAX_PENDING_PARTICLES: usize = 2048;

pub fn is_synced(entity: &Entity) -> bool {
    !matches!(entity, Entity::Collector(_) | Entity::Particle(_))
}

pub fn spawn_of(entity: &Entity) -> Option<EntitySpawn> {
    let kind = match entity {
        Entity::Asteroid(asteroid) => SpawnKind::Asteroid {
            size: asteroid.asteroid_size(),
        },
        Entity::Spaceship(ship) => SpawnKind::Spaceship {
            spaceship: ship.spaceship().clone(),
            role: ship.role(),
        },
        Entity::Fragment(fragment) => SpawnKind::Fragment {
            resource: fragment.resource(),
            amount: fragment.amount(),
        },
        Entity::Projectile(projectile) => SpawnKind::Projectile {
            color: projectile.color().0,
        },
        Entity::Shield(_) => SpawnKind::Shield,
        Entity::Collector(_) | Entity::Particle(_) => return None,
    };
    Some(EntitySpawn {
        id: entity.id() as NetId,
        kind,
    })
}

pub fn state_of(entity: &Entity) -> Option<NetEntityState> {
    let look = match entity {
        Entity::Asteroid(asteroid) => EntityLook {
            frame: asteroid.frame() as u16,
            shield: None,
            effects: asteroid.visual_effects(),
        },
        Entity::Spaceship(ship) => EntityLook {
            frame: ship.frame() as u16,
            shield: None,
            effects: ship.visual_effects(),
        },
        Entity::Shield(shield) => EntityLook {
            shield: Some(shield.look()),
            ..Default::default()
        },
        Entity::Fragment(_) | Entity::Projectile(_) => EntityLook::default(),
        Entity::Collector(_) | Entity::Particle(_) => return None,
    };
    Some(NetEntityState {
        id: entity.id() as NetId,
        pos: NetVec::from_vec2(entity.position_f32()),
        vel: NetVec::from_vec2(entity.velocity_f32()),
        look,
    })
}

pub fn net_states(space: &SpaceAdventure) -> Vec<NetEntityState> {
    let shield_velocity: HashMap<usize, Vec2> = space
        .all_entities()
        .filter_map(|entity| entity.as_spaceship().ok())
        .filter_map(|ship| ship.shield_id().map(|id| (id, ship.velocity_f32())))
        .collect();

    space
        .all_entities()
        .filter_map(|entity| {
            let mut state = state_of(entity)?;
            if let Some(velocity) = shield_velocity.get(&entity.id()) {
                state.vel = NetVec::from_vec2(*velocity);
            }
            Some(state)
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct SnapshotTracker {
    sent_ids: HashSet<NetId>,
    pending_particles: Vec<ParticleSpawn>,
    tick: u64,
}

impl SnapshotTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn collect_particles(&mut self, space: &mut SpaceAdventure) {
        self.pending_particles.extend(space.take_particle_outbox());
        let overflow = self
            .pending_particles
            .len()
            .saturating_sub(MAX_PENDING_PARTICLES);
        self.pending_particles.drain(..overflow);
    }

    pub fn welcome(&mut self, space: &SpaceAdventure, your_ship_id: usize) -> Welcome {
        let spawns: Vec<EntitySpawn> = space.all_entities().filter_map(spawn_of).collect();
        self.sent_ids = spawns.iter().map(|spawn| spawn.id).collect();
        Welcome {
            your_ship_id: your_ship_id as NetId,
            spawns,
            states: net_states(space),
        }
    }

    pub fn prepare(
        &mut self,
        space: &SpaceAdventure,
        guest_ship_id: usize,
        last_input_seq: u64,
    ) -> Option<Snapshot> {
        let you = space.local_view(guest_ship_id)?;
        let current: HashMap<NetId, &Entity> = space
            .all_entities()
            .filter(|entity| is_synced(entity))
            .map(|entity| (entity.id() as NetId, entity))
            .collect();
        let spawned = current
            .iter()
            .filter(|(id, _)| !self.sent_ids.contains(*id))
            .filter_map(|(_, entity)| spawn_of(entity))
            .collect();
        let removed = self
            .sent_ids
            .iter()
            .filter(|id| !current.contains_key(*id))
            .copied()
            .collect();
        self.tick += 1;
        Some(Snapshot {
            tick: self.tick,
            last_input_seq,
            spawned,
            removed,
            states: net_states(space),
            particles: self.pending_particles.clone(),
            you,
        })
    }

    pub fn commit(&mut self, snapshot: &Snapshot) {
        for spawn in snapshot.spawned.iter() {
            self.sent_ids.insert(spawn.id);
        }
        for id in snapshot.removed.iter() {
            self.sent_ids.remove(id);
        }
        let sent = snapshot.particles.len().min(self.pending_particles.len());
        self.pending_particles.drain(..sent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space_adventure::{utils::EntityState, ShipLoadout, SpaceAdventure};
    use crate::types::AppResult;
    use std::collections::HashSet;

    fn space_with_guest() -> AppResult<(SpaceAdventure, usize)> {
        let mut space =
            SpaceAdventure::new(false, 0.0)?.with_host(&ShipLoadout::test_default())?;
        space.force_running();
        let mut loadout = ShipLoadout::test_default();
        loadout.spaceship.shield = crate::core::Shield::Small;
        let guest_id = space.add_guest(&loadout)?;
        Ok((space, guest_id))
    }

    #[test]
    fn test_welcome_spawns_every_synced_entity() -> AppResult<()> {
        let (space, guest_id) = space_with_guest()?;
        let mut tracker = SnapshotTracker::new();
        let welcome = tracker.welcome(&space, guest_id);

        let synced: HashSet<NetId> = space
            .all_entities()
            .filter(|e| is_synced(e))
            .map(|e| e.id() as NetId)
            .collect();
        let spawned: HashSet<NetId> = welcome.spawns.iter().map(|s| s.id).collect();
        assert_eq!(spawned, synced);
        assert_eq!(welcome.states.len(), synced.len());
        assert!(!space
            .all_entities()
            .any(|e| matches!(e, Entity::Collector(_)) && synced.contains(&(e.id() as NetId))));
        Ok(())
    }

    #[test]
    fn test_shield_moves_with_its_ship() -> AppResult<()> {
        let (mut space, guest_id) = space_with_guest()?;
        space.handle_player_input(guest_id, crate::space_adventure::PlayerInput::MoveDown)?;
        space.update(0.025)?;
        space.update(0.025)?;
        let ship = space.get_ship(guest_id).expect("guest");
        let shield_id = ship.shield_id().expect("shield") as NetId;
        let ship_vel = NetVec::from_vec2(ship.velocity_f32());
        let states = net_states(&space);
        let shield_state = states
            .iter()
            .find(|s| s.id == shield_id)
            .expect("shield state");
        assert_eq!(shield_state.vel, ship_vel);
        assert!(shield_state.look.shield.is_some());
        Ok(())
    }

    #[test]
    fn test_unsent_snapshot_is_folded_into_the_next() -> AppResult<()> {
        let (mut space, guest_id) = space_with_guest()?;
        let mut tracker = SnapshotTracker::new();
        tracker.welcome(&space, guest_id);

        let new_id = space.generate_asteroid(
            Vec2::new(50.0, 50.0),
            Vec2::ZERO,
            crate::space_adventure::asteroid::AsteroidSize::Small,
        ) as NetId;
        let dropped = tracker.prepare(&space, guest_id, 0).expect("snapshot");
        assert!(dropped.spawned.iter().any(|s| s.id == new_id));

        let resent = tracker.prepare(&space, guest_id, 0).expect("snapshot");
        assert!(resent.spawned.iter().any(|s| s.id == new_id));
        tracker.commit(&resent);

        let next = tracker.prepare(&space, guest_id, 0).expect("snapshot");
        assert!(next.spawned.is_empty());

        space.remove_entity(&(new_id as usize));
        let removed = tracker.prepare(&space, guest_id, 0).expect("snapshot");
        assert_eq!(removed.removed, vec![new_id]);
        Ok(())
    }

    #[test]
    fn test_particles_are_kept_until_committed() -> AppResult<()> {
        let (mut space, guest_id) = space_with_guest()?;
        space.set_record_particles(true);
        let mut tracker = SnapshotTracker::new();
        tracker.welcome(&space, guest_id);
        space.generate_particle(
            Vec2::new(10.0, 10.0),
            Vec2::ZERO,
            image::Rgba([1, 2, 3, 255]),
            EntityState::Immortal,
            1,
        );
        tracker.collect_particles(&mut space);

        let first = tracker.prepare(&space, guest_id, 0).expect("snapshot");
        assert_eq!(first.particles.len(), 1);
        let again = tracker.prepare(&space, guest_id, 0).expect("snapshot");
        assert_eq!(again.particles.len(), 1);
        tracker.commit(&again);
        assert!(tracker
            .prepare(&space, guest_id, 0)
            .expect("snapshot")
            .particles
            .is_empty());
        Ok(())
    }

    #[test]
    fn test_no_snapshot_without_the_guest_ship() -> AppResult<()> {
        let (mut space, guest_id) = space_with_guest()?;
        let mut tracker = SnapshotTracker::new();
        space.remove_guest();
        assert!(tracker.prepare(&space, guest_id, 0).is_none());
        Ok(())
    }
}
