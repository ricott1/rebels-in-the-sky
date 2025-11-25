use super::{collisions::HitBox, networking::ImageType, space_callback::SpaceCallback, traits::*};
use crate::register_impl;
use glam::{I16Vec2, Vec2};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

const HIT_BOX_RADIUS: i16 = 40;
// const MAGNET_ACCELERATION: f32 = 35.0;

#[derive(Debug)]
pub struct CollectorEntity {
    id: usize,
    previous_position: Vec2,
    position: Vec2,
    velocity: Vec2,
    image: RgbaImage,
    hit_box: HitBox,
}

impl Body for CollectorEntity {
    fn previous_position(&self) -> I16Vec2 {
        self.previous_position.as_i16vec2()
    }

    fn position(&self) -> I16Vec2 {
        self.position.as_i16vec2()
    }

    fn velocity(&self) -> I16Vec2 {
        self.velocity.as_i16vec2()
    }

    fn update_body(&mut self, _deltatime: f32) -> Vec<SpaceCallback> {
        vec![]
    }
}

impl Sprite for CollectorEntity {
    fn image(&self) -> &RgbaImage {
        &self.image
    }

    fn network_image_type(&self) -> ImageType {
        ImageType::None
    }
}

impl Collider for CollectorEntity {
    fn collider_type(&self) -> ColliderType {
        ColliderType::Collector
    }

    fn hit_box(&self) -> &HitBox {
        &self.hit_box
    }
}

register_impl!(!ControllableSpaceship for CollectorEntity);
register_impl!(!ResourceFragment for CollectorEntity);

impl Entity for CollectorEntity {
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
        if let SpaceCallback::SetPosition { position, .. } = callback {
            self.previous_position = self.position;
            self.position = position.as_vec2();
        }
        vec![]
    }
}

impl CollectorEntity {
    pub fn new() -> Self {
        let image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0]));

        // The fragment hitbox is larger than the sprite on purpose
        // so that when hitting a spaceship it is accelerated towards it.
        let mut hit_box = HashMap::new();
        for x in -HIT_BOX_RADIUS..=HIT_BOX_RADIUS {
            for y in -HIT_BOX_RADIUS..=HIT_BOX_RADIUS {
                let point = I16Vec2::new(x, y);
                if point.distance_squared(I16Vec2::ZERO) < HIT_BOX_RADIUS.pow(2) {
                    hit_box.insert(point, false);
                } else if point.distance_squared(I16Vec2::ZERO) == HIT_BOX_RADIUS.pow(2) {
                    hit_box.insert(point, true);
                }
            }
        }
        hit_box.insert(I16Vec2::ZERO, false);

        Self {
            id: 0,
            previous_position: Vec2::ZERO,
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            image,
            hit_box: hit_box.into(),
        }
    }
}
