use super::{collisions::HitBox, space_callback::SpaceCallback, visual_effects::VisualEffect};
use crate::{core::resources::Resource, types::ResourceMap};
use glam::I16Vec2;
use image::{Rgba, RgbaImage};
use std::{collections::HashMap, fmt::Debug};

pub type VisualEffectMap = HashMap<VisualEffect, f32>;

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

    // Converts the coordinate of the center point to those of the top left corner
    fn center_to_top_left(&self, center: I16Vec2) -> I16Vec2 {
        center - (self.hit_box().top_left() + self.hit_box().bottom_right()) / 2
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

    fn update_body(&mut self, _deltatime: f32) -> Vec<SpaceCallback> {
        vec![]
    }
}

pub trait Sprite {
    fn image(&self) -> &RgbaImage;

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
    Projectile {
        shot_by: usize,
        filter_shield_id: Option<usize>,
    },
    Shield,
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

pub trait GameEntity: Sprite + Body + Collider {
    fn set_id(&mut self, id: usize);
    fn id(&self) -> usize;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerInput {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    ToggleAutofire,
    Shoot,
    ReleaseScraps,
    ToggleShield,
}

pub trait ControllableSpaceship {
    fn is_player(&self) -> bool;
    fn fuel(&self) -> u32;
    fn fuel_capacity(&self) -> u32;
    fn resources(&self) -> &ResourceMap;
    fn resources_mut(&mut self) -> &mut ResourceMap;
    fn storage_capacity(&self) -> u32;
    fn max_speed(&self) -> u32;
    fn current_charge(&self) -> u32;
    fn max_charge(&self) -> u32;
    fn is_recharging(&self) -> bool;
    fn thrust(&self) -> u32;
    fn maneuverability(&self) -> u32;
    fn current_durability(&self) -> u32;
    fn max_durability(&self) -> u32;
    fn handle_player_input(&mut self, input: PlayerInput);
}

pub trait ResourceFragment {
    fn resource(&self) -> Resource;
    fn amount(&self) -> u32;
}

pub trait ColoredResource {
    fn color(&self) -> Rgba<u8>;
}

impl ColoredResource for Resource {
    fn color(&self) -> Rgba<u8> {
        match self {
            Self::GOLD => [240, 230, 140, 255],
            Self::SCRAPS => [192, 192, 192, 255],
            Self::RUM => [114, 47, 55, 255],
            Self::FUEL => [64, 224, 208, 255],
            Self::SATOSHI => [255, 255, 255, 255],
        }
        .into()
    }
}
