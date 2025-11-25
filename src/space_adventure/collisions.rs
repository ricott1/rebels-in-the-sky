use crate::space_adventure::ResourceFragment;

use super::{
    space_callback::SpaceCallback, visual_effects::VisualEffect, ColliderType,
    ControllableSpaceship, Entity,
};
use glam::I16Vec2;
use image::Pixel;
use std::{
    collections::{
        hash_map::{Iter, Keys, Values},
        HashMap,
    },
    fmt::Debug,
};

#[derive(Debug, Clone, PartialEq, Eq)]
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

fn check_physical_collision(one: &Box<dyn Entity>, other: &Box<dyn Entity>) -> bool {
    if one.previous_position() == one.position() {
        return false;
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
                        return true;
                    }
                }
            }
        } else {
            for x in path.x..=0 {
                let y = (slope * x as f32).round() as i16;
                for (&point, &_) in one.hit_box().iter() {
                    let g_point = one.position() + point + I16Vec2::new(x, y) - other.position();
                    if other.hit_box().contains_key(&g_point) {
                        return true;
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
                    return true;
                }
            }
        }
    } else {
        for y in path.y..=0 {
            let x = path.x;
            for (&point, &_) in one.hit_box().iter() {
                let g_point = one.position() + point + I16Vec2::new(x, y) - other.position();
                if other.hit_box().contains_key(&g_point) {
                    return true;
                }
            }
        }
    }

    false
}

fn check_granular_phase_collision(one: &Box<dyn Entity>, other: &Box<dyn Entity>) -> bool {
    for &point in one.hit_box().keys() {
        let g_point = one.position() + point - other.position();
        if other.hit_box().contains_key(&g_point) {
            return true;
        }
    }

    false
}

fn check_broad_phase_collision(one: &Box<dyn Entity>, other: &Box<dyn Entity>) -> bool {
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

fn are_colliding(one: &Box<dyn Entity>, other: &Box<dyn Entity>) -> bool {
    if one.collider_type() == ColliderType::None || other.collider_type() == ColliderType::None {
        return false;
    }

    if one.layer() != other.layer() {
        return false;
    }

    if one.parent_id() == Some(other.id()) || other.parent_id() == Some(one.id()) {
        return false;
    }

    // Broad phase detection, shortcut if rects cannot intersect
    if !check_broad_phase_collision(one, other) {
        return false;
    }

    // Granular phase detection
    if check_granular_phase_collision(one, other) {
        return true;
    }

    // Physical path phase detection
    // This is not perfect, since we don't check if the entities crossed paths while moving,
    // but only one against the other final position. We also don't check if the entity didn't move
    // but rotated somehow. Good enough for us.
    if check_physical_collision(one, other) {
        log::debug!(
            "Found physical collision! {}->{} hit {:#?}",
            one.previous_position(),
            one.position(),
            other.rect()
        );
        return true;
    }

    // Do the same swapping entities.
    if check_physical_collision(other, one) {
        log::debug!(
            "Found physical collision! {}->{} hit {:#?}",
            other.previous_position(),
            other.position(),
            one.rect()
        );
        return true;
    }

    false
}

pub fn resolve_collision_between(
    one: &Box<dyn Entity>,
    other: &Box<dyn Entity>,
) -> Vec<SpaceCallback> {
    if !are_colliding(one, other) {
        return vec![];
    }

    match (one.collider_type(), other.collider_type()) {
        (ColliderType::AsteroidPlanet, ColliderType::Asteroid) => {
            vec![SpaceCallback::DamageEntity {
                id: other.id(),
                damage: one.collision_damage(),
            }]
        }
        (ColliderType::Asteroid, ColliderType::AsteroidPlanet) => {
            resolve_collision_between(other, one)
        }
        (ColliderType::AsteroidPlanet, ColliderType::Spaceship) => {
            let ship_control: &dyn ControllableSpaceship = other
                .as_trait_ref()
                .expect("Spaceship should implement ControllableSpaceship");

            if ship_control.is_player() {
                return vec![SpaceCallback::LandSpaceshipOnAsteroid];
            }

            vec![]
        }
        (ColliderType::Spaceship, ColliderType::AsteroidPlanet) => {
            resolve_collision_between(other, one)
        }
        (ColliderType::Projectile, ColliderType::Asteroid) => {
            vec![
                SpaceCallback::DestroyEntity { id: one.id() },
                SpaceCallback::DamageEntity {
                    id: other.id(),
                    damage: one.collision_damage(),
                },
            ]
        }
        (ColliderType::Asteroid, ColliderType::Projectile) => resolve_collision_between(other, one),
        (ColliderType::Projectile, ColliderType::Spaceship) => {
            vec![
                SpaceCallback::DestroyEntity { id: one.id() },
                SpaceCallback::DamageEntity {
                    id: other.id(),
                    damage: one.collision_damage(),
                },
            ]
        }
        (ColliderType::Spaceship, ColliderType::Projectile) => {
            resolve_collision_between(other, one)
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
        (ColliderType::Asteroid, ColliderType::Spaceship) => resolve_collision_between(other, one),

        (ColliderType::Spaceship, ColliderType::Fragment) => {
            let g_point = other.position() - one.position();
            if one.hit_box().contains_key(&g_point) {
                let resource_fragment: &dyn ResourceFragment = other
                    .as_trait_ref()
                    .expect("Fragment should implement ResourceFragment.");
                let resource = resource_fragment.resource();
                let amount = resource_fragment.amount();

                return vec![
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
                ];
            }
            vec![]
        }

        (ColliderType::Fragment, ColliderType::Spaceship) => resolve_collision_between(other, one),

        (ColliderType::Collector, ColliderType::Fragment) => {
            // If a fragment touches the collector hit_box, it is accelerated towards it.
            vec![SpaceCallback::SetAcceleration {
                id: other.id(),
                acceleration: one.center() - other.center(),
            }]
        }
        (ColliderType::Fragment, ColliderType::Collector) => resolve_collision_between(other, one),

        _ => vec![],
    }
}

#[cfg(test)]
mod test {

    use crate::{
        space_adventure::{
            collector::CollectorEntity, collisions::are_colliding, fragment::FragmentEntity,
            traits::*,
        },
        types::AppResult,
        world::resources::Resource,
    };
    use glam::Vec2;

    #[test]
    fn test_spaceship_fragment_collisions() -> AppResult<()> {
        let collector = CollectorEntity::new();
        let fragment = FragmentEntity::new(
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

        let trait_collector: Box<dyn Entity> = Box::new(collector) as Box<dyn Entity>;
        let trait_fragment: Box<dyn Entity> = Box::new(fragment) as Box<dyn Entity>;
        assert!(are_colliding(&trait_collector, &trait_fragment) == false);

        let collector = CollectorEntity::new();
        let fragment = FragmentEntity::new(
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
        let trait_collector: Box<dyn Entity> = Box::new(collector) as Box<dyn Entity>;
        let trait_fragment: Box<dyn Entity> = Box::new(fragment) as Box<dyn Entity>;
        assert!(are_colliding(&trait_collector, &trait_fragment) == true);

        Ok(())
    }
}
