use crate::world::resources::Resource;

use super::{
    asteroid::AsteroidSize, space::SpaceAdventure, utils::EntityState, visual_effects::VisualEffect,
};
use glam::{I16Vec2, Vec2};
use image::Rgba;

#[derive(Debug, Clone, Copy)]
pub enum SpaceCallback {
    AddVisualEffect {
        id: usize,
        effect: VisualEffect,
        duration: f32,
    },

    CollectFragment {
        id: usize,
        resource: Resource,
        amount: u32,
    },

    DamageEntity {
        id: usize,
        damage: f32,
    },

    DestroyEntity {
        id: usize,
    },

    GenerateAsteroid {
        position: Vec2,
        velocity: Vec2,
        size: AsteroidSize,
    },

    GenerateFragment {
        position: Vec2,
        velocity: Vec2,
        resource: Resource,
        amount: u32,
    },

    GenerateParticle {
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        particle_state: EntityState,
        layer: usize,
    },

    GenerateProjectile {
        shot_by_id: usize,
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        damage: f32,
    },

    LandSpaceshipOnAsteroid,

    SetAcceleration {
        id: usize,
        acceleration: I16Vec2,
    },

    SetPosition {
        id: usize,
        position: I16Vec2,
    },
}

impl SpaceCallback {
    pub fn call(&self, space: &mut SpaceAdventure) {
        let mut callbacks = vec![];
        match *self {
            Self::AddVisualEffect {
                id,
                effect,
                duration,
            } => {
                if let Some(entity) = space.get_entity_mut(&id) {
                    entity.add_visual_effect(duration, effect);
                }
            }

            Self::CollectFragment { id, .. } => {
                if let Some(entity) = space.get_entity_mut(&id) {
                    callbacks.append(&mut entity.handle_space_callback(*self));
                }
            }

            Self::DamageEntity { id, .. } => {
                if let Some(entity) = space.get_entity_mut(&id) {
                    callbacks.append(&mut entity.handle_space_callback(*self));
                }
            }

            Self::DestroyEntity { id } => {
                if let Some(entity) = space.get_entity_mut(&id) {
                    callbacks.append(&mut entity.handle_space_callback(*self));
                    space.remove_entity(&id);
                }
            }

            Self::GenerateAsteroid {
                position,
                velocity,
                size,
            } => {
                space.generate_asteroid(position, velocity, size);
            }

            Self::GenerateFragment {
                position,
                velocity,
                resource,
                amount,
            } => {
                space.generate_fragment(position, velocity, resource, amount);
            }

            Self::GenerateParticle {
                position,
                velocity,
                color,
                particle_state,
                layer,
            } => {
                space.generate_particle(position, velocity, color, particle_state, layer);
            }

            Self::GenerateProjectile {
                shot_by_id,
                position,
                velocity,
                color,
                damage,
            } => {
                space.generate_projectile(shot_by_id, position, velocity, color, damage);
            }

            Self::LandSpaceshipOnAsteroid => {
                space.land_on_asteroid();
            }

            Self::SetAcceleration { id, .. } => {
                if let Some(entity) = space.get_entity_mut(&id) {
                    callbacks.append(&mut entity.handle_space_callback(*self));
                }
            }

            Self::SetPosition { id, .. } => {
                if let Some(entity) = space.get_entity_mut(&id) {
                    callbacks.append(&mut entity.handle_space_callback(*self));
                }
            }
        }
        for callback in callbacks.iter() {
            callback.call(space);
        }
    }
}
