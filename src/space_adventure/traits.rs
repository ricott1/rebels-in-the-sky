use crate::{types::ResourceMap, world::resources::Resource};

use super::{space_callback::SpaceCallback, spaceship::ShooterState, visual_effects::VisualEffect};
use glam::I16Vec2;
use image::{Rgba, RgbaImage};
use itertools::Itertools;
use std::{
    collections::{
        hash_map::{Iter, Keys, Values},
        HashMap,
    },
    fmt::Debug,
};

pub type VisualEffectMap = HashMap<VisualEffect, f32>;

pub trait MaybeImplements<Trait: ?Sized> {
    fn as_trait_ref(&self) -> Option<&Trait>;
    fn as_trait_mut(&mut self) -> Option<&mut Trait>;
}

#[macro_export]
macro_rules! register_impl {
    ($trait_:ident for $ty:ty) => {
        impl MaybeImplements<dyn $trait_> for $ty {
            fn as_trait_ref(&self) -> Option<&(dyn $trait_ + 'static)> {
                Some(self)
            }

            fn as_trait_mut(&mut self) -> Option<&mut (dyn $trait_ + 'static)> {
                Some(self)
            }
        }
    };

    (!$trait_:ident for $ty:ty) => {
        impl MaybeImplements<dyn $trait_> for $ty {
            fn as_trait_ref(&self) -> Option<&(dyn $trait_ + 'static)> {
                None
            }

            fn as_trait_mut(&mut self) -> Option<&mut (dyn $trait_ + 'static)> {
                None
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitBox {
    inner: HashMap<I16Vec2, bool>,
    size: I16Vec2,
    top_left: I16Vec2,
    bottom_right: I16Vec2,
}

impl From<HashMap<I16Vec2, bool>> for HitBox {
    fn from(value: HashMap<I16Vec2, bool>) -> Self {
        let min_x = if value.len() > 0 {
            value
                .keys()
                .min_by(|a, b| a.x.cmp(&b.x))
                .expect("There should be a max x")
                .x
        } else {
            0
        };

        let max_x = if value.len() > 0 {
            value
                .keys()
                .max_by(|a, b| a.x.cmp(&b.x))
                .expect("There should be a max x")
                .x
        } else {
            0
        };

        let min_y = if value.len() > 0 {
            value
                .keys()
                .min_by(|a, b| a.y.cmp(&b.y))
                .expect("There should be a max x")
                .y
        } else {
            0
        };

        let max_y = if value.len() > 0 {
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
            size: I16Vec2::new(max_x - min_x, max_y - min_y),
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
}

pub trait Body: Sprite {
    fn previous_rect(&self) -> (I16Vec2, I16Vec2) {
        (
            self.previous_position() + self.hit_box().top_left,
            self.previous_position() + self.hit_box().bottom_right,
        )
    }

    fn rect(&self) -> (I16Vec2, I16Vec2) {
        (
            self.position() + self.hit_box().top_left,
            self.position() + self.hit_box().bottom_right,
        )
    }

    fn center(&self) -> I16Vec2 {
        self.position() + (self.hit_box().top_left + self.hit_box().bottom_right) / 2
    }

    // Used to calculate collisions.
    fn previous_position(&self) -> I16Vec2;

    fn position(&self) -> I16Vec2;

    fn velocity(&self) -> I16Vec2 {
        I16Vec2::ZERO
    }

    fn update_body(&mut self, _: f32) -> Vec<SpaceCallback> {
        vec![]
    }
}

pub trait Sprite {
    fn image(&self) -> &RgbaImage;

    fn layer(&self) -> usize {
        0
    }

    fn hit_box(&self) -> &HitBox;

    fn hit_box_vec(&self) -> Vec<(I16Vec2, bool)> {
        self.hit_box()
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect_vec()
    }

    fn size(&self) -> I16Vec2 {
        self.hit_box().size
    }

    fn should_apply_visual_effects<'a>(&self) -> bool {
        false
    }

    fn apply_visual_effects<'a>(&'a self, image: &'a RgbaImage) -> RgbaImage {
        image.clone()
    }

    fn add_visual_effect(&mut self, _duration: f32, _effect: VisualEffect) {}

    fn remove_visual_effect(&mut self, _effect: &VisualEffect) {}

    fn update_sprite(&mut self, _deltatime: f32) -> Vec<SpaceCallback> {
        vec![]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColliderType {
    None,
    Spaceship,
    Asteroid,
    Projectile,
    Fragment,
}
pub trait Collider: Body {
    fn collision_damage(&self) -> f32 {
        10.0
    }

    fn collider_type(&self) -> ColliderType {
        ColliderType::None
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
                let g_point = one.position() + I16Vec2::new(x, y) - other.position();
                if other.hit_box().contains_key(&g_point) {
                    return true;
                }
            }
        } else {
            for x in path.x..=0 {
                let y = (slope * x as f32).round() as i16;
                let g_point = one.position() + I16Vec2::new(x, y) - other.position();
                if other.hit_box().contains_key(&g_point) {
                    return true;
                }
            }
        }
    } else {
        if path.y > 0 {
            for y in 0..=path.y {
                let g_point = one.position() + I16Vec2::new(path.x, y) - other.position();
                if other.hit_box().contains_key(&g_point) {
                    return true;
                }
            }
        } else {
            for y in path.y..=0 {
                let g_point = one.position() + I16Vec2::new(path.x, y) - other.position();
                if other.hit_box().contains_key(&g_point) {
                    return true;
                }
            }
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

    return true;
}

fn check_granular_phase_collision(one: &Box<dyn Entity>, other: &Box<dyn Entity>) -> bool {
    for &point in one.hit_box().keys() {
        let g_point = one.position() + point - other.position();
        if other.hit_box().contains_key(&g_point) {
            return true;
        }
    }

    return false;
}

fn are_colliding(one: &Box<dyn Entity>, other: &Box<dyn Entity>) -> bool {
    if one.collider_type() == ColliderType::None || other.collider_type() == ColliderType::None {
        return false;
    }

    if one.layer() != other.layer() {
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
    match (one.collider_type(), other.collider_type()) {
        (ColliderType::Projectile, ColliderType::Asteroid) => {
            if !are_colliding(one, other) {
                return vec![];
            }
            return vec![
                SpaceCallback::DestroyEntity { id: one.id() },
                SpaceCallback::DamageEntity {
                    id: other.id(),
                    damage: one.collision_damage(),
                },
            ];
        }
        (ColliderType::Asteroid, ColliderType::Projectile) => resolve_collision_between(other, one),
        (ColliderType::Spaceship, ColliderType::Asteroid) => {
            if !are_colliding(one, other) {
                return vec![];
            }
            return vec![
                SpaceCallback::DamageEntity {
                    id: one.id(),
                    damage: other.collision_damage(),
                },
                SpaceCallback::DamageEntity {
                    id: other.id(),
                    damage: one.collision_damage(),
                },
            ];
        }
        (ColliderType::Asteroid, ColliderType::Spaceship) => resolve_collision_between(other, one),

        (ColliderType::Spaceship, ColliderType::Fragment) => {
            if !are_colliding(one, other) {
                return vec![];
            }

            // Two cases: if the fragment actually hits the spaceship hitbox, it is collected.
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
                            color: other.image().get_pixel(0, 0).clone(),
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
            // Else, it is accelerated towards the spaceship.
            return vec![
                SpaceCallback::AddVisualEffect {
                    id: other.id(),
                    effect: VisualEffect::ColorMask {
                        color: Rgba([0, 255, 0, 255]),
                    },
                    duration: VisualEffect::COLOR_MASK_LIFETIME,
                },
                SpaceCallback::AccelerateEntity {
                    id: other.id(),
                    acceleration: one.center() - other.center(),
                },
            ];
        }
        (ColliderType::Fragment, ColliderType::Spaceship) => resolve_collision_between(other, one),

        _ => return vec![],
    }
}

pub trait Entity:
    Sprite
    + Collider
    + MaybeImplements<dyn PlayerControlled>
    + MaybeImplements<dyn ResourceFragment>
    + Debug
    + Send
    + Sync
{
    fn set_id(&mut self, id: usize);
    fn id(&self) -> usize;
    fn update(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        let mut callbacks = vec![];
        callbacks.append(&mut self.update_body(deltatime));
        callbacks.append(&mut self.update_sprite(deltatime));

        callbacks
    }

    fn handle_space_callback(&mut self, _callback: SpaceCallback) -> Vec<SpaceCallback> {
        vec![]
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PlayerInput {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MainButton,
    SecondButton,
}

pub trait PlayerControlled {
    fn fuel(&self) -> u32;
    fn fuel_capacity(&self) -> u32;
    fn resources(&self) -> &ResourceMap;
    fn storage_capacity(&self) -> u32;
    fn max_speed(&self) -> u32;
    fn charge(&self) -> u32;
    fn max_charge(&self) -> u32;
    fn shooter_state(&self) -> ShooterState;
    fn thrust(&self) -> u32;
    fn maneuverability(&self) -> u32;
    fn current_durability(&self) -> u32;
    fn durability(&self) -> u32;
    fn handle_player_input(&mut self, input: PlayerInput);
}

pub trait ResourceFragment {
    fn resource(&self) -> Resource;
    fn amount(&self) -> u32;
}

#[cfg(test)]
mod test {

    use crate::{
        space_adventure::{fragment::FragmentEntity, traits::*, SpaceshipEntity},
        types::{AppResult, ResourceMap},
        world::{resources::Resource, spaceship::SpaceshipPrefab},
    };
    use glam::Vec2;

    #[test]
    fn test_spaceship_fragment_collisions() -> AppResult<()> {
        let base_ship = SpaceshipPrefab::Ragnarok.spaceship("name".into());
        let spaceship = SpaceshipEntity::from_spaceship(&base_ship, ResourceMap::default(), 100)?;

        let fragment = FragmentEntity::new(
            Vec2::new(
                spaceship.position().x as f32 + 60.0,
                spaceship.position().y as f32,
            ),
            Vec2::ZERO,
            Resource::SCRAPS,
            1,
        );

        let min_distance = spaceship
            .hit_box()
            .keys()
            .map(|point| point.as_vec2().distance(fragment.position().as_vec2()))
            .reduce(f32::min)
            .unwrap();

        println!(
            "Ship position: {}\nFragment position: {}\nDistance: {}\nHitbox size: {}\n",
            spaceship.center(),
            fragment.center(),
            min_distance,
            fragment.hit_box().size,
        );

        let trait_spaceship: Box<dyn Entity> = Box::new(spaceship) as Box<dyn Entity>;
        let trait_fragment: Box<dyn Entity> = Box::new(fragment) as Box<dyn Entity>;
        assert!(are_colliding(&trait_spaceship, &trait_fragment) == false);

        let spaceship = SpaceshipEntity::from_spaceship(&base_ship, ResourceMap::default(), 100)?;
        let fragment = FragmentEntity::new(
            Vec2::new(
                spaceship.position().x as f32 + 29.0,
                spaceship.position().y as f32,
            ),
            Vec2::ZERO,
            Resource::SCRAPS,
            1,
        );

        let min_distance = spaceship
            .hit_box()
            .keys()
            .map(|point| point.as_vec2().distance(fragment.position().as_vec2()))
            .reduce(f32::min)
            .unwrap();

        println!(
            "Ship position: {}\nFragment position: {}\nDistance: {}\nHitbox size: {}\n",
            spaceship.center(),
            fragment.center(),
            min_distance,
            fragment.hit_box().size,
        );
        let trait_fragment: Box<dyn Entity> = Box::new(fragment) as Box<dyn Entity>;
        assert!(are_colliding(&trait_spaceship, &trait_fragment) == true);

        Ok(())
    }
}
