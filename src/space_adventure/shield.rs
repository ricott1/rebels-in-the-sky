use super::{collisions::HitBox, space_callback::SpaceCallback, traits::*};
use super::{entity::Entity, visual_effects::VisualEffect};
use glam::{I16Vec2, Vec2};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

const HIT_BOX_RADIUS: i16 = 16;
const COLLISION_DAMAGE: f32 = 3.5;
const SHIELD_RECOVERY_PER_SECOND: f32 = 0.5;
const CHARGE_COST_PER_SECOND: f32 = 3.15;

#[derive(Debug)]
pub struct ShieldEntity {
    id: usize,
    previous_position: Vec2,
    position: Vec2,
    velocity: Vec2,
    damage: f32,
    current_durability: f32,
    max_durability: f32,
    is_active: bool,
    is_disabled: bool,
    damage_reduction: f32,
    image: RgbaImage,
    inactive_image: RgbaImage,
    recharging_image: RgbaImage,
    hit_box: HitBox,
}

impl Body for ShieldEntity {
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

impl Sprite for ShieldEntity {
    fn image(&self) -> &RgbaImage {
        if self.is_active() {
            &self.image
        } else if self.current_durability() < self.max_durability() {
            &self.recharging_image
        } else {
            &self.inactive_image
        }
    }
}

impl Collider for ShieldEntity {
    fn collision_damage(&self) -> f32 {
        self.damage
    }

    fn collider_type(&self) -> ColliderType {
        ColliderType::Shield
    }

    fn hit_box(&self) -> &HitBox {
        &self.hit_box
    }
}

impl GameEntity for ShieldEntity {
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

            SpaceCallback::SetCenterPosition { center, .. } => {
                // position is the center of the spaceship. We want the shield center to coincide with position.
                // center is calculated as center = position() + (hit_box().top_left() + hit_box().bottom_right()) / 2
                // hence position = center - (hit_box().top_left() + hit_box().bottom_right()) / 2
                self.previous_position = self.position;
                self.position = self.center_to_top_left(center).as_vec2();
            }
            SpaceCallback::DamageEntity { damage, .. } => {
                self.add_damage(damage);
                self.add_visual_effect(
                    VisualEffect::COLOR_MASK_LIFETIME,
                    VisualEffect::ColorMask {
                        color: [235, 0, 255],
                    },
                );
            }

            SpaceCallback::ToggleShield { .. } => {
                if !self.is_disabled {
                    self.is_active = !self.is_active;
                }
            }

            SpaceCallback::DeactivateEntity { .. } => {
                self.is_active = false;
            }

            _ => {}
        }

        vec![]
    }

    fn update(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        if self.current_durability() == 0 {
            self.is_active = false;
            self.is_disabled = true; // Cannot Toggle back on while recovering.
        }

        if !self.is_active && self.current_durability < self.max_durability {
            self.current_durability = (self.current_durability
                + SHIELD_RECOVERY_PER_SECOND * deltatime)
                .min(self.max_durability);
        }

        if self.is_disabled && self.current_durability() == self.max_durability() {
            self.is_disabled = false;
        }

        let mut callbacks = vec![];
        callbacks.append(&mut self.update_body(deltatime));
        callbacks.append(&mut self.update_sprite(deltatime));

        callbacks
    }
}

impl ShieldEntity {
    pub fn new_entity(max_durability: f32, damage_reduction: f32) -> Entity {
        // The fragment hitbox is larger than the sprite on purpose
        // so that when hitting something it interacts with it before the spaceship.
        let mut hit_box = HashMap::new();
        // Image reflects the hitbox, with a pale blue halo which becomes more transparent in the middle.
        let mut image =
            RgbaImage::new(2 * HIT_BOX_RADIUS as u32 + 1, 2 * HIT_BOX_RADIUS as u32 + 1);
        let mut inactive_image =
            RgbaImage::new(2 * HIT_BOX_RADIUS as u32 + 1, 2 * HIT_BOX_RADIUS as u32 + 1);
        let mut recharging_image =
            RgbaImage::new(2 * HIT_BOX_RADIUS as u32 + 1, 2 * HIT_BOX_RADIUS as u32 + 1);
        const HITBOX_MAX_DISTANCE: i16 = HIT_BOX_RADIUS.pow(2);
        for x in -HIT_BOX_RADIUS..=HIT_BOX_RADIUS {
            for y in -HIT_BOX_RADIUS..=HIT_BOX_RADIUS {
                let mut point = I16Vec2::new(x, y);
                let distance_squared = point.distance_squared(I16Vec2::ZERO);
                if distance_squared > HITBOX_MAX_DISTANCE {
                    continue;
                }

                point += HIT_BOX_RADIUS * I16Vec2::ONE;

                if distance_squared < HITBOX_MAX_DISTANCE {
                    hit_box.insert(point, false);
                } else {
                    //distance_squared == HITBOX_MAX_DISTANCE
                    hit_box.insert(point, true);
                }
                let mut pixel = Rgba([
                    85,
                    165,
                    85,
                    (255.0 * distance_squared as f32 / HITBOX_MAX_DISTANCE as f32) as u8,
                ]);

                image.put_pixel(point.x as u32, point.y as u32, pixel);

                pixel.0[3] =
                    (255.0 * distance_squared as f32 / HITBOX_MAX_DISTANCE as f32).min(25.0) as u8;
                inactive_image.put_pixel(point.x as u32, point.y as u32, pixel);

                pixel.0[0] = 165;
                pixel.0[1] = 85;
                recharging_image.put_pixel(point.x as u32, point.y as u32, pixel);
            }
        }
        hit_box.insert(I16Vec2::ZERO, false);

        Entity::Shield(Self {
            id: 0,
            previous_position: Vec2::ZERO,
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            damage: COLLISION_DAMAGE,
            current_durability: max_durability,
            max_durability,
            is_active: false,
            is_disabled: false,
            damage_reduction,
            image,
            inactive_image,
            recharging_image,
            hit_box: hit_box.into(),
        })
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn charge_cost_per_second(&self) -> f32 {
        if self.is_active() {
            CHARGE_COST_PER_SECOND
        } else {
            0.0
        }
    }

    fn add_damage(&mut self, damage: f32) {
        self.current_durability =
            (self.current_durability - damage * self.damage_reduction).max(0.0);
    }

    pub fn current_durability(&self) -> u32 {
        self.current_durability.round() as u32
    }

    pub fn max_durability(&self) -> u32 {
        self.max_durability.round() as u32
    }
}
