use super::{
    asteroid::AsteroidEntity,
    constants::*,
    entity::Entity,
    fragment::FragmentEntity,
    particle::ParticleEntity,
    player::{LocalPlayerView, PlayerOutcome},
    projectile::ProjectileEntity,
    shield::ShieldEntity,
    space::{crop_to_screen, draw_entity_at, space_background},
    space_callback::SpaceCallback,
    spaceship::SpaceshipEntity,
    traits::*,
    utils::EntityState,
    visual_effects::VisualEffect,
    wire::{EntityLook, EntitySpawn, NetEntityState, NetId, Snapshot, SpawnKind, Welcome},
};
use crate::types::AppResult;
use glam::Vec2;
use image::{Rgba, RgbaImage};
#[cfg(test)]
use std::collections::HashSet;
use std::{collections::HashMap, time::Instant};

const MAX_MIRROR_PARTICLES: usize = 2048;

#[derive(Debug)]
struct MirrorEntity {
    entity: Entity,
    position: Vec2,
    velocity: Vec2,
}

#[derive(Debug)]
pub struct SpaceMirror {
    background: RgbaImage,
    entities: HashMap<NetId, MirrorEntity>,
    particles: Vec<Entity>,
    view: Option<LocalPlayerView>,
    started: Instant,
    ending: Option<(Instant, PlayerOutcome)>,
}

fn build_entity(spawn: &EntitySpawn) -> AppResult<Entity> {
    let mut entity = match &spawn.kind {
        SpawnKind::Asteroid { size } => {
            AsteroidEntity::new_entity(Vec2::ZERO, Vec2::ZERO, *size, 0.0)
        }
        SpawnKind::Spaceship { spaceship, role } => {
            SpaceshipEntity::display_entity(spaceship, *role)?
        }
        SpawnKind::Fragment { resource, amount } => {
            FragmentEntity::new_entity(Vec2::ZERO, Vec2::ZERO, *resource, *amount)
        }
        SpawnKind::Projectile { color } => {
            ProjectileEntity::new_entity(0, None, Vec2::ZERO, Vec2::ZERO, Rgba(*color), 0.0, false)
        }
        SpawnKind::Shield => ShieldEntity::new_entity(1.0, 1.0, false),
    };
    entity.set_id(spawn.id as usize);
    Ok(entity)
}

fn apply_look(entity: &mut Entity, look: &EntityLook) {
    match entity {
        Entity::Asteroid(asteroid) => asteroid.set_frame(look.frame),
        Entity::Spaceship(ship) => ship.set_frame(look.frame),
        Entity::Shield(shield) => {
            if let Some(shield_look) = look.shield {
                shield.set_look(shield_look);
            }
        }
        _ => {}
    }
    entity.set_visual_effects(&look.effects);
}

impl SpaceMirror {
    pub fn new(welcome: &Welcome) -> AppResult<Self> {
        let mut mirror = Self {
            background: space_background(),
            entities: HashMap::new(),
            particles: Vec::new(),
            view: None,
            started: Instant::now(),
            ending: None,
        };
        for spawn in welcome.spawns.iter() {
            mirror.insert(spawn)?;
        }
        for state in welcome.states.iter() {
            mirror.apply_state(state);
        }
        Ok(mirror)
    }

    fn insert(&mut self, spawn: &EntitySpawn) -> AppResult<()> {
        let entity = build_entity(spawn)?;
        self.entities.insert(
            spawn.id,
            MirrorEntity {
                entity,
                position: Vec2::ZERO,
                velocity: Vec2::ZERO,
            },
        );
        Ok(())
    }

    fn apply_state(&mut self, state: &NetEntityState) {
        if let Some(mirrored) = self.entities.get_mut(&state.id) {
            mirrored.position = state.pos.to_vec2();
            mirrored.velocity = state.vel.to_vec2();
            apply_look(&mut mirrored.entity, &state.look);
        }
    }

    pub fn apply(&mut self, snapshot: &Snapshot) -> AppResult<()> {
        for spawn in snapshot.spawned.iter() {
            self.insert(spawn)?;
        }
        for id in snapshot.removed.iter() {
            self.entities.remove(id);
        }
        for state in snapshot.states.iter() {
            self.apply_state(state);
        }
        for particle in snapshot.particles.iter() {
            if self.particles.len() >= MAX_MIRROR_PARTICLES {
                break;
            }
            self.particles.push(ParticleEntity::new_entity(
                particle.pos.to_vec2(),
                particle.vel.to_vec2(),
                Rgba(particle.color),
                particle
                    .lifetime
                    .map_or(EntityState::Immortal, |lifetime| EntityState::Decaying {
                        lifetime,
                    }),
                particle.layer as usize,
            ));
        }
        self.view = Some(snapshot.you.clone());
        Ok(())
    }

    pub fn advance(&mut self, deltatime: f32) {
        for mirrored in self.entities.values_mut() {
            mirrored.position += mirrored.velocity * deltatime;
        }
        self.particles.retain_mut(|particle| {
            !particle
                .update(deltatime)
                .iter()
                .any(|callback| matches!(callback, SpaceCallback::DestroyEntity { .. }))
        });
    }

    pub fn image(&self, width: u32, height: u32) -> AppResult<RgbaImage> {
        let mut base = self.background.clone();
        for layer in 0..MAX_LAYER {
            for mirrored in self.entities.values().filter(|m| m.entity.layer() == layer) {
                draw_entity_at(
                    &mut base,
                    &mirrored.entity,
                    mirrored.position.as_i16vec2(),
                    false,
                );
            }
            for particle in self.particles.iter().filter(|p| p.layer() == layer) {
                draw_entity_at(&mut base, particle, particle.position(), false);
            }
        }

        if let Some((time, _)) = &self.ending {
            VisualEffect::FadeOut.apply_global_effect(&mut base, time.elapsed().as_secs_f32());
        } else if self.is_starting() {
            VisualEffect::FadeIn
                .apply_global_effect(&mut base, self.started.elapsed().as_secs_f32());
        }

        Ok(crop_to_screen(&base, width, height))
    }

    pub fn local_view(&self) -> Option<&LocalPlayerView> {
        self.view.as_ref()
    }

    pub fn last_outcome(&self) -> Option<PlayerOutcome> {
        self.view.as_ref().map(|view| view.outcome())
    }

    pub fn start_ending(&mut self, outcome: PlayerOutcome) {
        if self.ending.is_none() {
            self.ending = Some((Instant::now(), outcome));
        }
    }

    pub fn ending_outcome(&self) -> Option<&PlayerOutcome> {
        self.ending.as_ref().map(|(_, outcome)| outcome)
    }

    pub fn is_starting(&self) -> bool {
        self.ending.is_none() && self.started.elapsed() < STARTING_DURATION
    }

    pub fn is_ending(&self) -> bool {
        self.ending.is_some()
    }

    pub fn should_return(&self) -> bool {
        self.ending
            .as_ref()
            .is_some_and(|(time, _)| time.elapsed() >= ENDING_DURATION)
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len() + self.particles.len()
    }

    #[cfg(test)]
    pub fn synced_position(&self, id: NetId) -> Option<Vec2> {
        self.entities.get(&id).map(|mirrored| mirrored.position)
    }

    #[cfg(test)]
    pub fn synced_ids(&self) -> HashSet<NetId> {
        self.entities.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space_adventure::{
        snapshot::{is_synced, SnapshotTracker},
        wire::NetVec,
        ShipLoadout, SpaceAdventure,
    };
    use crate::types::ResourceMap;

    fn host_with_guest() -> AppResult<(SpaceAdventure, usize)> {
        let mut space = SpaceAdventure::new(false, 0.0)?.with_host(&ShipLoadout::test_default())?;
        space.force_running();
        let guest_id = space.add_guest(&ShipLoadout::test_default())?;
        space.set_record_particles(true);
        Ok((space, guest_id))
    }

    fn host_ids(space: &SpaceAdventure) -> HashSet<NetId> {
        space
            .all_entities()
            .filter(|e| is_synced(e))
            .map(|e| e.id() as NetId)
            .collect()
    }

    #[test]
    fn test_mirror_follows_the_host() -> AppResult<()> {
        let (mut space, guest_id) = host_with_guest()?;
        let mut tracker = SnapshotTracker::new();
        let mut mirror = SpaceMirror::new(&tracker.welcome(&space, guest_id))?;

        for tick in 0..40 {
            space.update(0.025)?;
            tracker.collect_particles(&mut space);
            if tick % 2 == 1 {
                let snapshot = tracker.prepare(&space, guest_id, 0).expect("snapshot");
                if tick % 6 != 1 {
                    mirror.apply(&snapshot)?;
                    tracker.commit(&snapshot);
                }
            }
        }
        let snapshot = tracker.prepare(&space, guest_id, 0).expect("snapshot");
        mirror.apply(&snapshot)?;
        tracker.commit(&snapshot);

        assert_eq!(mirror.synced_ids(), host_ids(&space));
        for entity in space.all_entities().filter(|e| is_synced(e)) {
            let mirrored = mirror
                .synced_position(entity.id() as NetId)
                .expect("mirrored");
            assert!(mirrored.distance(entity.position_f32()) < 0.1);
        }
        assert_eq!(mirror.local_view(), space.local_view(guest_id).as_ref());
        Ok(())
    }

    #[test]
    fn test_mirror_moves_entities_between_snapshots() -> AppResult<()> {
        let (space, guest_id) = host_with_guest()?;
        let mut tracker = SnapshotTracker::new();
        let mut welcome = tracker.welcome(&space, guest_id);
        let state = welcome
            .states
            .iter_mut()
            .find(|s| s.id != guest_id as NetId)
            .expect("an entity");
        state.vel = NetVec::from_vec2(Vec2::new(16.0, 0.0));
        let id = state.id;
        let start = state.pos.to_vec2();

        let mut mirror = SpaceMirror::new(&welcome)?;
        mirror.advance(0.5);
        assert!(
            mirror
                .synced_position(id)
                .expect("entity")
                .distance(start + Vec2::new(8.0, 0.0))
                < 0.01
        );
        Ok(())
    }

    #[test]
    fn test_mirror_ending() -> AppResult<()> {
        let (space, guest_id) = host_with_guest()?;
        let mut mirror = SpaceMirror::new(&SnapshotTracker::new().welcome(&space, guest_id))?;
        assert!(mirror.ending_outcome().is_none());
        let outcome = PlayerOutcome {
            resources: ResourceMap::new(),
            durability: 1,
        };
        mirror.start_ending(outcome.clone());
        mirror.start_ending(PlayerOutcome {
            resources: ResourceMap::new(),
            durability: 9,
        });
        assert_eq!(mirror.ending_outcome(), Some(&outcome));
        assert!(mirror.is_ending());
        assert!(!mirror.should_return());
        Ok(())
    }

    #[test]
    fn test_mirror_draws() -> AppResult<()> {
        let (space, guest_id) = host_with_guest()?;
        let mirror = SpaceMirror::new(&SnapshotTracker::new().welcome(&space, guest_id))?;
        let image = mirror.image(SCREEN_SIZE.x, SCREEN_SIZE.y)?;
        assert_eq!(
            (image.width(), image.height()),
            (SCREEN_SIZE.x, SCREEN_SIZE.y)
        );
        Ok(())
    }
}
