use crate::{
    core::resources::Resource,
    space_adventure::{
        constants::SCREEN_SIZE, Body, ControllableSpaceship, Direction, GameEntity, Sprite,
    },
};

use super::{
    asteroid::AsteroidSize, space::SpaceAdventure, utils::EntityState, visual_effects::VisualEffect,
};
use glam::{I16Vec2, Vec2};
use image::Rgba;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Clone, Copy)]
pub enum SpaceCallback {
    ActivateEntity {
        id: usize,
    },

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

    DeactivateEntity {
        id: usize,
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

    ReleaseScraps {
        id: usize,
    },

    SetAcceleration {
        id: usize,
        acceleration: Vec2,
    },

    SetPosition {
        id: usize,
        position: I16Vec2,
    },

    // Same as SetPosition but passes the entity center. Useful if we want to align entities on the center.
    SetCenterPosition {
        id: usize,
        center: I16Vec2,
    },

    Shoot {
        id: usize,
    },

    ToggleAutofire {
        id: usize,
    },

    ToggleShield {
        id: usize,
    },

    TrackPlayer {
        id: usize,
    },

    UseCharge {
        id: usize,
        amount: f32,
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
                let shooter_shield_id = if let Some(entity) = space.get_entity(&shot_by_id) {
                    if let Ok(spaceship) = entity.as_spaceship() {
                        spaceship.shield_id()
                    } else {
                        None
                    }
                } else {
                    None
                };

                space.generate_projectile(
                    shot_by_id,
                    shooter_shield_id,
                    position,
                    velocity,
                    color,
                    damage,
                );
            }

            Self::LandSpaceshipOnAsteroid => {
                space.land_on_asteroid();
            }

            Self::ActivateEntity { id }
            | Self::CollectFragment { id, .. }
            | Self::DamageEntity { id, .. }
            | Self::DeactivateEntity { id }
            | Self::SetAcceleration { id, .. }
            | Self::SetPosition { id, .. }
            | Self::SetCenterPosition { id, .. }
            | Self::ReleaseScraps { id }
            | Self::Shoot { id }
            | Self::ToggleAutofire { id }
            | Self::ToggleShield { id }
            | Self::UseCharge { id, .. } => {
                if let Some(entity) = space.get_entity_mut(&id) {
                    callbacks.append(&mut entity.handle_space_callback(*self));
                }
            }

            Self::TrackPlayer { id } => {
                let target_position = if let Some(player) = space.get_player() {
                    player.center()
                } else {
                    SCREEN_SIZE.as_i16vec2()
                };
                if let Some(entity) = space.get_entity_mut(&id) {
                    let entity_position = entity.center();

                    if entity_position.x > SCREEN_SIZE.x as i16 - 12 {
                        callbacks.append(&mut entity.handle_space_callback(
                            SpaceCallback::SetAcceleration {
                                id,
                                acceleration: Direction::Left.as_vec2(),
                            },
                        ))
                    } else if entity_position.x <= 70 || entity_position.x <= target_position.x {
                        callbacks.append(&mut entity.handle_space_callback(
                            SpaceCallback::SetAcceleration {
                                id,
                                acceleration: Direction::Right.as_vec2(),
                            },
                        ))
                    }

                    let y_distance = (target_position.y - entity_position.y).abs();
                    let rng = &mut ChaCha8Rng::from_os_rng();

                    if let Ok(spaceship) = entity.as_spaceship_mut() {
                        if y_distance > 4 {
                            if entity_position.y > target_position.y {
                                spaceship.thrust_towards(Direction::Up.as_vec2());
                            } else if entity_position.y < target_position.y {
                                spaceship.thrust_towards(Direction::Down.as_vec2());
                            }
                        }

                        if y_distance > 14
                            || spaceship.current_charge() < spaceship.max_charge() / 5
                        {
                            spaceship.set_autofire(false);
                        } else if spaceship.current_charge() as f32
                            > rng.random_range(0.25..=0.5) * spaceship.max_charge() as f32
                        {
                            spaceship.set_autofire(true);
                        }

                        if entity_position.x >= SCREEN_SIZE.x as i16 - 8 {
                            spaceship.thrust_towards(Direction::Left.as_vec2());
                        }
                    }
                }
            }
        }
        for callback in callbacks.iter() {
            callback.call(space);
        }
    }
}
