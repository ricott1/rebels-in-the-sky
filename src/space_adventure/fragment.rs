use super::{space_callback::SpaceCallback, traits::*, utils::EntityState};
use crate::{register_impl, space_adventure::constants::*, world::resources::Resource};
use glam::{I16Vec2, Vec2};
use image::RgbaImage;
use std::collections::HashMap;

const HIT_BOX_RADIUS: i16 = 40;
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
        match self.state {
            EntityState::Decaying { lifetime } => {
                let new_lifetime = lifetime - deltatime;
                if new_lifetime > 0.0 {
                    self.state = EntityState::Decaying {
                        lifetime: new_lifetime,
                    };
                } else {
                    return vec![SpaceCallback::DestroyEntity { id: self.id() }];
                }
            }
            _ => {}
        }

        self.previous_position = self.position;
        self.velocity += self.acceleration * deltatime;
        self.velocity = self.velocity.clamp_length_max(30.0);

        self.position += self.velocity * deltatime;
        self.acceleration = Vec2::ZERO;

        if self.position.x < 0.0 || self.position.x > SCREEN_WIDTH as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }
        if self.position.y < 0.0 || self.position.y > SCREEN_HEIGHT as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }

        vec![]
    }
}

impl Sprite for FragmentEntity {
    fn layer(&self) -> usize {
        1
    }
    fn image(&self) -> &RgbaImage {
        &self.image
    }

    fn hit_box(&self) -> &HitBox {
        &self.hit_box
    }
}

impl Collider for FragmentEntity {
    fn collider_type(&self) -> ColliderType {
        ColliderType::Fragment
    }
}

register_impl!(!PlayerControlled for FragmentEntity);
register_impl!(ResourceFragment for FragmentEntity);
impl ResourceFragment for FragmentEntity {
    fn resource(&self) -> Resource {
        self.resource
    }

    fn amount(&self) -> u32 {
        self.amount
    }
}

impl Entity for FragmentEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }

    fn id(&self) -> usize {
        self.id
    }

    fn handle_space_callback(&mut self, callback: SpaceCallback) -> Vec<SpaceCallback> {
        match callback {
            SpaceCallback::AccelerateEntity { acceleration, .. } => {
                self.acceleration = MAGNET_ACCELERATION * acceleration.as_vec2()
            }

            _ => {}
        }
        vec![]
    }
}

impl FragmentEntity {
    pub fn new(position: Vec2, velocity: Vec2, resource: Resource, amount: u32) -> Self {
        let color = resource.color();
        let image = RgbaImage::from_pixel(1, 1, color);

        // The fragment hitbox is larger than the sprite on purpose
        // so that when hitting a spaceship it is accelerated towards it.
        let mut hit_box = HashMap::new();
        for x in -HIT_BOX_RADIUS..=HIT_BOX_RADIUS {
            for y in -HIT_BOX_RADIUS..=HIT_BOX_RADIUS {
                let point = I16Vec2::new(x, y);
                if point.distance_squared(I16Vec2::ZERO) <= HIT_BOX_RADIUS.pow(2) {
                    hit_box.insert(point, false);
                }
            }
        }
        hit_box.insert(I16Vec2::ZERO, true);

        Self {
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
        }
    }
}
