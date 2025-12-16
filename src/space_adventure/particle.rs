use super::{collisions::HitBox, space_callback::SpaceCallback, traits::*, utils::EntityState};
use crate::space_adventure::{constants::*, entity::Entity};
use glam::{I16Vec2, Vec2};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

#[derive(Debug)]
pub struct ParticleEntity {
    id: usize,
    _color: Rgba<u8>,
    previous_position: Vec2,
    position: Vec2,
    velocity: Vec2,
    state: EntityState,
    image: RgbaImage,
    layer: usize,
    hit_box: HitBox,
}

impl Body for ParticleEntity {
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

        if self.position.x < 0.0 || self.position.x > SCREEN_SIZE.x as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }
        if self.position.y < 0.0 || self.position.y > SCREEN_SIZE.y as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }

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

        vec![]
    }
}

impl Sprite for ParticleEntity {
    fn image(&self) -> &RgbaImage {
        &self.image
    }
}

impl Collider for ParticleEntity {
    fn hit_box(&self) -> &HitBox {
        &self.hit_box
    }
}

impl GameEntity for ParticleEntity {
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

impl ParticleEntity {
    pub fn new_entity(
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        state: EntityState,
        layer: usize,
    ) -> Entity {
        let image = RgbaImage::from_pixel(1, 1, color);
        let mut hit_box = HashMap::new();
        hit_box.insert(I16Vec2::ZERO, true);
        Entity::Particle(Self {
            id: 0,
            _color: color,
            previous_position: position,
            position,
            velocity,
            state,
            image,
            layer,
            hit_box: hit_box.into(),
        })
    }
}
