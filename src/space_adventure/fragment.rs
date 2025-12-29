use super::{collisions::HitBox, space_callback::SpaceCallback, traits::*, utils::EntityState};
use crate::{
    core::resources::Resource,
    space_adventure::{constants::*, entity::Entity},
};
use glam::{I16Vec2, Vec2};
use image::RgbaImage;
use std::collections::HashMap;

const MAGNET_ACCELERATION: f32 = 35.0;

#[derive(Debug)]
pub struct FragmentEntity {
    id: usize,
    previous_position: Vec2,
    position: Vec2,
    velocity: Vec2,
    acceleration: Vec2,
    state: EntityState,
    image: RgbaImage,
    hit_box: HitBox,
    resource: Resource,
    amount: u32,
}

impl Body for FragmentEntity {
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
        if let EntityState::Decaying { lifetime } = self.state {
            let new_lifetime = lifetime - deltatime;
            if new_lifetime > 0.0 {
                self.state = EntityState::Decaying {
                    lifetime: new_lifetime,
                };
            } else {
                return vec![SpaceCallback::DestroyEntity { id: self.id() }];
            }
        }

        self.previous_position = self.position;
        self.velocity += self.acceleration * deltatime;
        self.velocity = self.velocity.clamp_length_max(30.0);

        self.position += self.velocity * deltatime;
        self.acceleration = Vec2::ZERO;

        if self.position.x < 0.0 || self.position.x > SCREEN_SIZE.x as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }
        if self.position.y < 0.0 || self.position.y > SCREEN_SIZE.y as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }

        vec![]
    }
}

impl Sprite for FragmentEntity {
    fn image(&self) -> &RgbaImage {
        &self.image
    }
}

impl Collider for FragmentEntity {
    fn collider_type(&self) -> ColliderType {
        ColliderType::Fragment
    }

    fn hit_box(&self) -> &HitBox {
        &self.hit_box
    }
}

impl ResourceFragment for FragmentEntity {
    fn resource(&self) -> Resource {
        self.resource
    }

    fn amount(&self) -> u32 {
        self.amount
    }
}

impl GameEntity for FragmentEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }

    fn id(&self) -> usize {
        self.id
    }

    fn layer(&self) -> usize {
        1
    }

    fn handle_space_callback(&mut self, callback: SpaceCallback) -> Vec<SpaceCallback> {
        // FIXME: MAGNET_ACCELERATION should come from the collector.
        if let SpaceCallback::SetAcceleration { acceleration, .. } = callback {
            self.acceleration = MAGNET_ACCELERATION * acceleration
        }
        vec![]
    }
}

impl FragmentEntity {
    pub fn new_entity(position: Vec2, velocity: Vec2, resource: Resource, amount: u32) -> Entity {
        let image = RgbaImage::from_pixel(1, 1, resource.color());

        // The fragment hitbox is larger than the sprite on purpose
        // so that when hitting a spaceship it is accelerated towards it.
        let mut hit_box = HashMap::new();
        hit_box.insert(I16Vec2::ZERO, true);

        Entity::Fragment(Self {
            id: 0,
            previous_position: position,
            position,
            velocity,
            acceleration: Vec2::ZERO,
            state: EntityState::Decaying { lifetime: 10.0 },
            image,
            hit_box: hit_box.into(),
            resource,
            amount,
        })
    }
}
