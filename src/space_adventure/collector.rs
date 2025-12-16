use super::{collisions::HitBox, space_callback::SpaceCallback, traits::*};
use crate::space_adventure::entity::Entity;
use glam::{I16Vec2, Vec2};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

const HIT_BOX_RADIUS: i16 = 40;
// const MAGNET_ACCELERATION: f32 = 35.0;

#[derive(Debug)]
pub struct CollectorEntity {
    id: usize,
    is_active: bool,
    previous_position: Vec2,
    position: Vec2,
    velocity: Vec2,
    image: RgbaImage,
    hit_box: HitBox,
}

impl CollectorEntity {
    pub fn is_active(&self) -> bool {
        self.is_active
    }
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
}

impl Collider for CollectorEntity {
    fn collider_type(&self) -> ColliderType {
        ColliderType::Collector
    }

    fn hit_box(&self) -> &HitBox {
        &self.hit_box
    }
}

impl GameEntity for CollectorEntity {
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
        match callback {
            SpaceCallback::ActivateEntity { .. } => {
                self.is_active = true;
            }

            SpaceCallback::DeactivateEntity { .. } => {
                self.is_active = false;
            }

            SpaceCallback::SetCenterPosition { center, .. } => {
                self.previous_position = self.position;
                self.position = self.center_to_top_left(center).as_vec2();
            }

            _ => {}
        }

        vec![]
    }
}

impl CollectorEntity {
    pub fn new_entity() -> Entity {
        let image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0]));

        // The fragment hitbox is larger than the sprite on purpose
        // so that when hitting a spaceship it is accelerated towards it.
        let mut hit_box = HashMap::new();
        const HITBOX_MAX_DISTANCE: i16 = HIT_BOX_RADIUS.pow(2);
        for x in -HIT_BOX_RADIUS..=HIT_BOX_RADIUS {
            for y in -HIT_BOX_RADIUS..=HIT_BOX_RADIUS {
                let point = I16Vec2::new(x, y);
                let distance_squared = point.distance_squared(I16Vec2::ZERO);
                if distance_squared < HITBOX_MAX_DISTANCE {
                    hit_box.insert(point, false);
                } else if distance_squared == HITBOX_MAX_DISTANCE {
                    hit_box.insert(point, true);
                }
            }
        }
        hit_box.insert(I16Vec2::ZERO, false);

        Entity::Collector(Self {
            id: 0,
            is_active: true,
            previous_position: Vec2::ZERO,
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            image,
            hit_box: hit_box.into(),
        })
    }
}
