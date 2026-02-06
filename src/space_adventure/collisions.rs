use super::{
    constants::PROJECTILE_SPACESHIP_DAMAGE_MULTIPLIER, entity::Entity,
    space_callback::SpaceCallback, traits::*, utils::EntityState, visual_effects::VisualEffect,
    Body, Collider, ColliderType, ControllableSpaceship, ResourceFragment,
};
use crate::types::AppResult;
use glam::{I16Vec2, Vec2};
use image::{Pixel, Rgba};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::{
    collections::{
        hash_map::{Iter, Keys, Values},
        HashMap,
    },
    fmt::Debug,
};

const SPACESHIP_COLLISION_DAMAGE: f32 = 5.0;

#[derive(Debug, Clone, PartialEq)]
pub struct HitBox {
    inner: HashMap<I16Vec2, bool>,
    size: I16Vec2,
    top_left: I16Vec2,
    bottom_right: I16Vec2,
}

impl From<HashMap<I16Vec2, bool>> for HitBox {
    fn from(value: HashMap<I16Vec2, bool>) -> Self {
        let min_x = if !value.is_empty() {
            value
                .keys()
                .min_by(|a, b| a.x.cmp(&b.x))
                .expect("There should be a max x")
                .x
        } else {
            0
        };

        let max_x = if !value.is_empty() {
            value
                .keys()
                .max_by(|a, b| a.x.cmp(&b.x))
                .expect("There should be a max x")
                .x
        } else {
            0
        };

        let min_y = if !value.is_empty() {
            value
                .keys()
                .min_by(|a, b| a.y.cmp(&b.y))
                .expect("There should be a max x")
                .y
        } else {
            0
        };

        let max_y = if !value.is_empty() {
            value
                .keys()
                .max_by(|a, b| a.y.cmp(&b.y))
                .expect("There should be a max x")
                .y
        } else {
            0
        };

        Self {
            inner: value,
            size: I16Vec2::new(max_x - min_x + 1, max_y - min_y + 1),
            top_left: I16Vec2::new(min_x, min_y),
            bottom_right: I16Vec2::new(max_x, max_y),
        }
    }
}

impl HitBox {
    pub fn iter(&self) -> Iter<'_, I16Vec2, bool> {
        self.inner.iter()
    }

    pub fn keys(&self) -> Keys<'_, I16Vec2, bool> {
        self.inner.keys()
    }

    pub fn values(&self) -> Values<'_, I16Vec2, bool> {
        self.inner.values()
    }

    pub fn contains_key(&self, k: &I16Vec2) -> bool {
        self.inner.contains_key(k)
    }

    pub fn size(&self) -> I16Vec2 {
        self.size
    }
    pub fn top_left(&self) -> I16Vec2 {
        self.top_left
    }
    pub fn bottom_right(&self) -> I16Vec2 {
        self.bottom_right
    }
}

fn check_physical_collision(one: &Entity, other: &Entity) -> Option<I16Vec2> {
    if one.previous_position() == one.position() {
        return None;
    }

    // Find all integer points in vector connecting self entity current and previous positions
    // and check if they are in other entity hitbox.
    let path = one.previous_position() - one.position();
    if path.x != 0 {
        let slope = path.y as f32 / path.x as f32;
        if path.x > 0 {
            for x in 0..=path.x {
                let y = (slope * x as f32).round() as i16;
                for (&point, &_) in one.hit_box().iter() {
                    let g_point = one.position() + point + I16Vec2::new(x, y) - other.position();
                    if other.hit_box().contains_key(&g_point) {
                        return Some(one.position() + point + I16Vec2::new(x, y));
                    }
                }
            }
        } else {
            for x in path.x..=0 {
                let y = (slope * x as f32).round() as i16;
                for (&point, &_) in one.hit_box().iter() {
                    let g_point = one.position() + point + I16Vec2::new(x, y) - other.position();
                    if other.hit_box().contains_key(&g_point) {
                        return Some(one.position() + point + I16Vec2::new(x, y));
                    }
                }
            }
        }
    } else if path.y > 0 {
        for y in 0..=path.y {
            let x = path.x;
            for (&point, &_) in one.hit_box().iter() {
                let g_point = one.position() + point + I16Vec2::new(x, y) - other.position();
                if other.hit_box().contains_key(&g_point) {
                    return Some(one.position() + point + I16Vec2::new(x, y));
                }
            }
        }
    } else {
        for y in path.y..=0 {
            let x = path.x;
            for (&point, &_) in one.hit_box().iter() {
                let g_point = one.position() + point + I16Vec2::new(x, y) - other.position();
                if other.hit_box().contains_key(&g_point) {
                    return Some(one.position() + point + I16Vec2::new(x, y));
                }
            }
        }
    }

    None
}

fn check_granular_phase_collision(one: &Entity, other: &Entity) -> Option<I16Vec2> {
    for &point in one.hit_box().keys() {
        let g_point = one.position() + point - other.position();
        if other.hit_box().contains_key(&g_point) {
            // FIXME: check if this is correct
            return Some(one.position() + point);
        }
    }

    None
}

fn check_broad_phase_collision(one: &Entity, other: &Entity) -> bool {
    let (s1_min, s1_max) = one.previous_rect();
    let (o1_min, o1_max) = other.previous_rect();

    let (s2_min, s2_max) = one.rect();
    let (o2_min, o2_max) = other.rect();

    if (s1_min.x > o1_max.x && s2_min.x > o2_max.x)
        || (o1_min.x > s1_max.x && o2_min.x > s2_max.x)
        || (s1_min.y > o1_max.y && s2_min.y > o2_max.y)
        || (o1_min.y > s1_max.y && o2_min.y > s2_max.y)
    {
        return false;
    }

    true
}

fn are_colliding(one: &Entity, other: &Entity) -> Option<I16Vec2> {
    if one.collider_type() == ColliderType::None || other.collider_type() == ColliderType::None {
        return None;
    }

    if one.layer() != other.layer() {
        return None;
    }

    // Broad phase detection, shortcut if rects cannot intersect
    if !check_broad_phase_collision(one, other) {
        return None;
    }

    // Granular phase detection
    if let Some(point) = check_granular_phase_collision(one, other) {
        return Some(point);
    }

    // Physical path phase detection
    // This is not perfect, since we don't check if the entities crossed paths while moving,
    // but only one against the other final position. We also don't check if the entity didn't move
    // but rotated somehow. Good enough for us.
    if let Some(point) = check_physical_collision(one, other) {
        log::debug!(
            "Found physical collision! {}->{} hit {:#?}",
            one.previous_position(),
            one.position(),
            other.rect()
        );
        return Some(point);
    }

    // Do the same swapping entities.
    if let Some(point) = check_physical_collision(other, one) {
        log::debug!(
            "Found physical collision! {}->{} hit {:#?}",
            other.previous_position(),
            other.position(),
            one.rect()
        );
        return Some(point);
    }

    None
}

fn get_collision_callbacks(
    one: &Entity,
    other: &Entity,
    collision_point: I16Vec2,
    deltatime: f32,
) -> AppResult<Vec<SpaceCallback>> {
    let callbacks = match (one.collider_type(), other.collider_type()) {
        (ColliderType::AsteroidPlanet, ColliderType::Asteroid) => {
            vec![SpaceCallback::DamageEntity {
                id: other.id(),
                damage: one.collision_damage(),
            }]
        }
        (ColliderType::Asteroid, ColliderType::AsteroidPlanet) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }
        (ColliderType::AsteroidPlanet, ColliderType::Spaceship) => {
            let spaceship_entity = other.as_spaceship()?;
            if spaceship_entity.is_player() {
                vec![SpaceCallback::LandSpaceshipOnAsteroid]
            } else {
                vec![]
            }
        }
        (ColliderType::Spaceship, ColliderType::AsteroidPlanet) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }
        (ColliderType::Projectile { .. }, ColliderType::Asteroid) => {
            let rng = &mut ChaCha8Rng::from_os_rng();
            let particle_velocity = one.velocity().as_vec2() * rng.random_range(0.1..=0.15)
                + Vec2::Y * rng.random_range(-1.0..=1.0) * 12.0;
            vec![
                SpaceCallback::DestroyEntity { id: one.id() },
                SpaceCallback::GenerateParticle {
                    position: collision_point.as_vec2(),
                    velocity: particle_velocity,
                    color: Rgba([
                        55 + rng.random_range(0..25),
                        55 + rng.random_range(0..25),
                        55 + rng.random_range(0..25),
                        255,
                    ]),
                    particle_state: EntityState::Decaying {
                        lifetime: 1.0 + rng.random_range(0.0..1.5),
                    },
                    layer: 2,
                },
                SpaceCallback::DamageEntity {
                    id: other.id(),
                    damage: one.collision_damage(),
                },
            ]
        }
        (ColliderType::Asteroid, ColliderType::Projectile { .. }) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }
        (ColliderType::Projectile { shot_by, .. }, ColliderType::Spaceship) => {
            if shot_by != other.id() {
                let rng = &mut ChaCha8Rng::from_os_rng();
                vec![
                    SpaceCallback::DestroyEntity { id: one.id() },
                    SpaceCallback::GenerateParticle {
                        position: collision_point.as_vec2(),
                        velocity: one.velocity().as_vec2() * rng.random_range(-0.1..=0.01)
                            + Vec2::Y * rng.random_range(-1.0..=1.0) * 8.0,
                        color: Rgba([210 + rng.random_range(0..=45), 55, 75, 205]),
                        particle_state: EntityState::Decaying {
                            lifetime: 1.0 + rng.random_range(0.0..1.5),
                        },
                        layer: 2,
                    },
                    SpaceCallback::DamageEntity {
                        id: other.id(),
                        damage: one.collision_damage() * PROJECTILE_SPACESHIP_DAMAGE_MULTIPLIER,
                    },
                ]
            } else {
                vec![]
            }
        }
        (ColliderType::Spaceship, ColliderType::Projectile { .. }) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }

        (
            ColliderType::Projectile {
                filter_shield_id, ..
            },
            ColliderType::Shield,
        ) => {
            let shield = other.as_shield()?;
            if matches!(filter_shield_id, Some(id) if id == other.id()) || !shield.is_active() {
                vec![]
            } else {
                let rng = &mut ChaCha8Rng::from_os_rng();
                vec![
                    SpaceCallback::DestroyEntity { id: one.id() },
                    SpaceCallback::GenerateParticle {
                        position: collision_point.as_vec2(),
                        velocity: one.velocity().as_vec2() * rng.random_range(-0.15..=-0.05)
                            + Vec2::Y * rng.random_range(-1.0..=1.0) * 4.0,
                        color: Rgba([210 + rng.random_range(0..=45), 125, 25, 205]),
                        particle_state: EntityState::Decaying {
                            lifetime: 1.0 + rng.random_range(0.0..1.5),
                        },
                        layer: 2,
                    },
                    SpaceCallback::DamageEntity {
                        id: other.id(),
                        damage: one.collision_damage(),
                    },
                ]
            }
        }
        (ColliderType::Shield, ColliderType::Projectile { .. }) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }
        (ColliderType::Asteroid, ColliderType::Shield) => {
            let shield = other.as_shield()?;
            if shield.is_active() {
                vec![
                    SpaceCallback::DamageEntity {
                        id: one.id(),
                        damage: other.collision_damage(),
                    },
                    SpaceCallback::DamageEntity {
                        id: other.id(),
                        damage: one.collision_damage(),
                    },
                ]
            } else {
                vec![]
            }
        }
        (ColliderType::Shield, ColliderType::Asteroid) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }

        (ColliderType::Spaceship, ColliderType::Asteroid) => {
            vec![
                SpaceCallback::DamageEntity {
                    id: one.id(),
                    damage: other.collision_damage(),
                },
                SpaceCallback::DestroyEntity { id: other.id() },
            ]
        }
        (ColliderType::Asteroid, ColliderType::Spaceship) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }
        (ColliderType::Spaceship, ColliderType::Fragment) => {
            let g_point = other.position() - one.position();
            if one.hit_box().contains_key(&g_point) {
                let resource_fragment = other.as_fragment()?;
                let resource = resource_fragment.resource();
                let amount = resource_fragment.amount();

                vec![
                    SpaceCallback::AddVisualEffect {
                        id: one.id(),
                        effect: VisualEffect::ColorMask {
                            color: other.image().get_pixel(0, 0).to_rgb().0,
                        },
                        duration: VisualEffect::COLOR_MASK_LIFETIME,
                    },
                    SpaceCallback::CollectFragment {
                        id: one.id(),
                        resource,
                        amount,
                    },
                    SpaceCallback::DestroyEntity { id: other.id() },
                ]
            } else {
                vec![]
            }
        }

        (ColliderType::Spaceship, ColliderType::Shield) => {
            let spaceship = one.as_spaceship()?;
            let shield = other.as_shield()?;

            if matches!(spaceship.shield_id(), Some(id) if id == shield.id()) && shield.is_active()
            {
                vec![SpaceCallback::UseCharge {
                    id: spaceship.id(),
                    amount: shield.charge_cost_per_second() * deltatime,
                }]
            } else {
                vec![]
            }
        }
        (ColliderType::Shield, ColliderType::Spaceship) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }
        (ColliderType::Fragment, ColliderType::Spaceship) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }
        (ColliderType::Collector, ColliderType::Fragment) => {
            let collector = one.as_collector()?;

            if collector.is_active() {
                // If a fragment touches the collector hit_box, it is accelerated towards it.
                vec![SpaceCallback::SetAcceleration {
                    id: other.id(),
                    acceleration: (one.center() - other.center()).as_vec2(),
                }]
            } else {
                vec![]
            }
        }
        (ColliderType::Fragment, ColliderType::Collector) => {
            get_collision_callbacks(other, one, collision_point, deltatime)?
        }

        (ColliderType::Spaceship, ColliderType::Spaceship) => {
            vec![
                SpaceCallback::DamageEntity {
                    id: one.id(),
                    damage: SPACESHIP_COLLISION_DAMAGE,
                },
                SpaceCallback::SetAcceleration {
                    id: one.id(),
                    acceleration: -one.velocity().as_vec2(),
                },
                SpaceCallback::DamageEntity {
                    id: other.id(),
                    damage: SPACESHIP_COLLISION_DAMAGE,
                },
                SpaceCallback::SetAcceleration {
                    id: other.id(),
                    acceleration: -one.velocity().as_vec2(),
                },
            ]
        }

        // FIXME: Missing shield on shield --> bounce back.
        //        This is tricky because the acceleration needs to be set on the parent spaceship, not on the shield.
        _ => vec![],
    };

    Ok(callbacks)
}

pub fn resolve_collision_between(
    one: &Entity,
    other: &Entity,
    deltatime: f32,
) -> AppResult<Vec<SpaceCallback>> {
    if let Some(collision_point) = are_colliding(one, other) {
        return get_collision_callbacks(one, other, collision_point, deltatime);
    }
    Ok(vec![])
}

#[cfg(test)]
mod test {
    use crate::space_adventure::resources::Resource;
    use crate::space_adventure::{
        collector::CollectorEntity, collisions::are_colliding, fragment::FragmentEntity, traits::*,
    };
    use crate::types::AppResult;
    use glam::Vec2;

    #[test]
    fn test_spaceship_fragment_collisions() -> AppResult<()> {
        let collector = CollectorEntity::new_entity();
        let fragment = FragmentEntity::new_entity(
            Vec2::new(
                collector.position().x as f32 + 60.0,
                collector.position().y as f32,
            ),
            Vec2::ZERO,
            Resource::SCRAPS,
            1,
        );

        let min_distance = collector
            .hit_box()
            .keys()
            .map(|point| point.as_vec2().distance(fragment.position().as_vec2()))
            .reduce(f32::min)
            .unwrap();

        println!(
            "Ship position: {}\nFragment position: {}\nDistance: {}\n",
            collector.center(),
            fragment.center(),
            min_distance,
        );

        assert!(are_colliding(&collector, &fragment).is_none());

        let collector = CollectorEntity::new_entity();
        let fragment = FragmentEntity::new_entity(
            Vec2::new(
                collector.position().x as f32 + 29.0,
                collector.position().y as f32,
            ),
            Vec2::ZERO,
            Resource::SCRAPS,
            1,
        );

        let min_distance = collector
            .hit_box()
            .keys()
            .map(|point| point.as_vec2().distance(fragment.position().as_vec2()))
            .reduce(f32::min)
            .unwrap();

        println!(
            "Ship position: {}\nFragment position: {}\nDistance: {}\n",
            collector.center(),
            fragment.center(),
            min_distance,
        );
        assert!(are_colliding(&collector, &fragment).is_some());

        Ok(())
    }
}
