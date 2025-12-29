use super::{collisions::HitBox, space_callback::SpaceCallback, traits::*};
use super::{constants::*, entity::Entity};
use glam::{I16Vec2, Vec2};
use image::{Rgba, RgbaImage};
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
    own_shield_id: Option<usize>,
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
}

impl Collider for ProjectileEntity {
    fn collision_damage(&self) -> f32 {
        self.damage
    }

    fn collider_type(&self) -> ColliderType {
        ColliderType::Projectile {
            shot_by: self.shot_by_id,
            filter_shield_id: self.own_shield_id,
        }
    }

    fn hit_box(&self) -> &HitBox {
        &self.hit_box
    }
}

impl GameEntity for ProjectileEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }

    fn id(&self) -> usize {
        self.id
    }

    fn layer(&self) -> usize {
        self.layer
    }
}

impl ProjectileEntity {
    pub fn new_entity(
        shot_by_id: usize,
        own_shield_id: Option<usize>,
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        damage: f32,
    ) -> Entity {
        let image = RgbaImage::from_pixel(1, 1, color);
        let mut hit_box = HashMap::new();
        hit_box.insert(I16Vec2::ZERO, true);
        Entity::Projectile(Self {
            id: 0,
            shot_by_id,
            own_shield_id,
            color,
            previous_position: position,
            position,
            velocity,
            state: ProjectileState::Immortal,
            damage,
            image,
            layer: 1,
            hit_box: hit_box.into(),
        })
    }
}
