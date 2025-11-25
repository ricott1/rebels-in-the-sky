use super::{collisions::HitBox, networking::ImageType, space_callback::SpaceCallback, traits::*};
use crate::{register_impl, space_adventure::constants::*};
use glam::{I16Vec2, Vec2};
use image::{Pixel, Rgba, RgbaImage};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
#[allow(unused)]
pub enum ProjectileState {
    Immortal,
    Decaying { lifetime: f32 },
}

#[derive(Debug)]
#[allow(unused)]
pub struct ProjectileEntity {
    id: usize,
    shot_by_id: usize,
    color: Rgba<u8>,
    previous_position: Vec2,
    position: Vec2,
    velocity: Vec2,
    state: ProjectileState,
    damage: f32,
    image: RgbaImage,
    layer: usize,
    hit_box: HitBox,
}

impl Body for ProjectileEntity {
    fn previous_position(&self) -> I16Vec2 {
        self.previous_position.as_i16vec2()
    }

    fn position(&self) -> I16Vec2 {
        self.position.as_i16vec2()
    }

    fn velocity(&self) -> I16Vec2 {
        self.velocity.as_i16vec2()
    }

    fn update_body(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        self.previous_position = self.position;
        self.position += self.velocity * deltatime;

        if self.position.x < 0.0 || self.position.x > MAX_ENTITY_POSITION.x as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }
        if self.position.y < 0.0 || self.position.y > MAX_ENTITY_POSITION.y as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }

        if let ProjectileState::Decaying { lifetime } = self.state {
            let new_lifetime = lifetime - deltatime;
            if new_lifetime > 0.0 {
                self.state = ProjectileState::Decaying {
                    lifetime: new_lifetime,
                };
            } else {
                return vec![SpaceCallback::DestroyEntity { id: self.id() }];
            }
        }

        vec![]
    }
}

impl Sprite for ProjectileEntity {
    fn image(&self) -> &RgbaImage {
        &self.image
    }

    fn network_image_type(&self) -> ImageType {
        ImageType::Projectile {
            color: self.color.to_rgb().0,
        }
    }
}

impl Collider for ProjectileEntity {
    fn collision_damage(&self) -> f32 {
        self.damage
    }

    fn collider_type(&self) -> ColliderType {
        ColliderType::Projectile
    }

    fn hit_box(&self) -> &HitBox {
        &self.hit_box
    }
}

impl Entity for ProjectileEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }

    fn id(&self) -> usize {
        self.id
    }

    fn parent_id(&self) -> Option<usize> {
        Some(self.shot_by_id)
    }

    fn layer(&self) -> usize {
        self.layer
    }
}

register_impl!(!ControllableSpaceship for ProjectileEntity);
register_impl!(!ResourceFragment for ProjectileEntity);

impl ProjectileEntity {
    pub fn new(
        shot_by_id: usize,
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        damage: f32,
    ) -> Self {
        let image = RgbaImage::from_pixel(1, 1, color);
        let mut hit_box = HashMap::new();
        hit_box.insert(I16Vec2::ZERO, true);
        Self {
            id: 0,
            shot_by_id,
            color,
            previous_position: position,
            position,
            velocity,
            state: ProjectileState::Immortal,
            damage,
            image,
            layer: 1,
            hit_box: hit_box.into(),
        }
    }
}
