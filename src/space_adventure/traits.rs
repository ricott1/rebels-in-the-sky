use crate::{types::ResourceMap, world::resources::Resource};

use super::{
    collisions::HitBox, networking::ImageType, space_callback::SpaceCallback,
    spaceship::ShooterState, visual_effects::VisualEffect,
};
use glam::I16Vec2;
use image::RgbaImage;
use std::{collections::HashMap, fmt::Debug};

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

pub trait Body: Collider {
    fn previous_rect(&self) -> (I16Vec2, I16Vec2) {
        (
            self.previous_position() + self.hit_box().top_left(),
            self.previous_position() + self.hit_box().bottom_right(),
        )
    }

    fn rect(&self) -> (I16Vec2, I16Vec2) {
        (
            self.position() + self.hit_box().top_left(),
            self.position() + self.hit_box().bottom_right(),
        )
    }

    fn center(&self) -> I16Vec2 {
        self.position() + (self.hit_box().top_left() + self.hit_box().bottom_right()) / 2
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

    fn network_image_type(&self) -> ImageType;

    fn should_apply_visual_effects(&self) -> bool {
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
    Asteroid,
    AsteroidPlanet,
    Collector,
    Fragment,
    Projectile,
    Spaceship,
}
pub trait Collider {
    fn collision_damage(&self) -> f32 {
        0.0
    }

    fn collider_type(&self) -> ColliderType {
        ColliderType::None
    }

    fn hit_box(&self) -> &HitBox;

    fn size(&self) -> I16Vec2 {
        self.hit_box().size()
    }
}

pub trait Entity:
    Sprite
    + Body
    + Collider
    + MaybeImplements<dyn ControllableSpaceship>
    + MaybeImplements<dyn ResourceFragment>
    + Debug
    + Send
    + Sync
{
    fn set_id(&mut self, id: usize);
    fn id(&self) -> usize;
    fn set_parent_id(&mut self, _parent_id: usize) {}
    fn parent_id(&self) -> Option<usize> {
        None
    }
    fn layer(&self) -> usize {
        0
    }
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
    ToggleAutofire,
    Shoot,
    ReleaseScraps,
}

pub trait ControllableSpaceship {
    fn is_player(&self) -> bool;
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
