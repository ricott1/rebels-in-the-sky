use super::space_callback::SpaceCallback;
use super::utils::{body_data_from_image, EntityState};
use super::{constants::*, traits::*};
use crate::image::components::ImageComponent;
use crate::image::utils::open_image;
use crate::register_impl;
use crate::space_adventure::visual_effects::VisualEffect;
use crate::space_adventure::Direction;
use crate::types::*;
use crate::world::constants::{FUEL_CONSUMPTION_PER_UNIT_STORAGE, SPEED_PENALTY_PER_UNIT_STORAGE};
use crate::world::resources::Resource;
use crate::{image::types::Gif, types::AppResult, world::spaceship::Spaceship};
use glam::{I16Vec2, Vec2};
use image::imageops::rotate90;
use image::{Rgba, RgbaImage};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShooterState {
    Ready { charge: f32 },
    Shooting { charge: f32, recoil: f32 },
    Recharging { charge: f32 },
}

impl ShooterState {
    const MAX_SHOOTING_RECOIL: f32 = 0.125;
    const SHOOTING_CHARGE_COST: f32 = 1.0;
    const MAX_CHARGE: f32 = 100.0;
    const CHARGE_RECOVERY_SPEED: f32 = 2.0;
    const RECHARGE_RECOVERY_SPEED: f32 = 7.5;
}

#[derive(Debug)]
pub struct SpaceshipEntity {
    id: usize,
    resources: ResourceMap,
    used_storage_capacity: u32,
    storage_capacity: u32,
    previous_position: Vec2,
    position: Vec2,
    velocity: Vec2,
    acceleration: Vec2,
    // We need a single hit box since it does not change with the gif.
    // Maps with hit box points, Point -> is_border in local coordinates.
    hit_boxes: Vec<HitBox>,
    current_durability: f32,
    durability: f32,
    base_thrust: f32,
    base_speed: f32,
    maneuverability: f32,
    fuel: f32,
    fuel_capacity: u32,
    base_fuel_consumption: f32,
    friction_coeff: f32,
    tick: usize,
    gif: Gif,
    engine_exhaust: Vec<I16Vec2>, // Position of exhaust in relative coords
    shooters: Vec<I16Vec2>,       // Position of shooters in relative coords
    auto_shoot: bool,
    shooter_state: ShooterState,
    visual_effects: VisualEffectMap,
}

impl Body for SpaceshipEntity {
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
        self.tick += 1;
        self.previous_position = self.position;

        let mut callbacks = vec![];
        if self.acceleration.length_squared() > 0.0 {
            let rng = &mut ChaCha8Rng::from_entropy();
            for &point in self.engine_exhaust.iter() {
                if self.velocity.length_squared() < self.acceleration.length_squared() / 2.0
                    || rng.gen_bool(0.25)
                {
                    let layer = rng.gen_range(0..2);
                    callbacks.push(SpaceCallback::GenerateParticle {
                        position: self.position + point.as_vec2(),
                        velocity: -3.0 * self.acceleration.normalize()
                            + Vec2::new(rng.gen_range(-0.5..0.5), rng.gen_range(-0.5..0.5)),
                        color: Rgba([
                            205 + rng.gen_range(0..50),
                            55 + rng.gen_range(0..200),
                            rng.gen_range(0..55),
                            255,
                        ]),
                        particle_state: EntityState::Decaying {
                            lifetime: 2.0 + rng.gen_range(0.0..1.5),
                        },
                        layer,
                    });
                }
            }
        }

        self.acceleration = self.acceleration - self.friction_coeff * self.velocity;

        let prev_velocity = self.velocity;
        self.velocity += self.acceleration * deltatime;

        if prev_velocity.x < -self.maneuverability {
            self.velocity.x = self.velocity.x.min(0.0);
        } else if prev_velocity.x > self.maneuverability {
            self.velocity.x = self.velocity.x.max(0.0);
        }

        if prev_velocity.y < -self.maneuverability {
            self.velocity.y = self.velocity.y.min(0.0);
        } else if prev_velocity.y > self.maneuverability {
            self.velocity.y = self.velocity.y.max(0.0);
        }

        self.velocity = self.velocity.clamp_length_max(self.max_speed());

        self.position += self.velocity * deltatime;
        self.acceleration = Vec2::ZERO;

        let min_position = Vec2::ZERO;
        let max_position =
            Vec2::new(SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32) - self.size().as_vec2();

        if self.position.x < min_position.x {
            self.position.x = min_position.x;
            self.velocity.x = 0.0;
        } else if self.position.x > max_position.x {
            self.position.x = max_position.x;
            self.velocity.x = 0.0;
        }

        if self.position.y < min_position.y {
            self.position.y = min_position.y;
            self.velocity.y = 0.0;
        } else if self.position.y > max_position.y {
            self.position.y = max_position.y;
            self.velocity.y = 0.0;
        }

        callbacks
    }
}

impl Sprite for SpaceshipEntity {
    fn layer(&self) -> usize {
        1
    }

    fn image(&self) -> &RgbaImage {
        &self.gif[self.frame()]
    }

    fn hit_box(&self) -> &HitBox {
        &self.hit_boxes[self.frame()]
    }

    fn should_apply_visual_effects<'a>(&self) -> bool {
        self.visual_effects.len() > 0
    }

    fn apply_visual_effects<'a>(&'a self, image: &'a RgbaImage) -> RgbaImage {
        let mut image = image.clone();
        if self.visual_effects.len() > 0 {
            for (effect, time) in self.visual_effects.iter() {
                effect.apply(self, &mut image, *time);
            }
        }
        image
    }

    fn add_visual_effect(&mut self, duration: f32, effect: VisualEffect) {
        self.visual_effects.insert(effect, duration);
    }

    fn remove_visual_effect(&mut self, effect: &VisualEffect) {
        self.visual_effects.remove(&effect);
    }

    fn update_sprite(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        for (_, lifetime) in self.visual_effects.iter_mut() {
            *lifetime -= deltatime;
        }

        self.visual_effects.retain(|_, lifetime| *lifetime > 0.0);

        vec![]
    }
}

impl Entity for SpaceshipEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }
    fn id(&self) -> usize {
        self.id
    }

    fn update(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        let mut callbacks = vec![];
        callbacks.append(&mut self.update_body(deltatime));
        callbacks.append(&mut self.update_sprite(deltatime));

        match self.shooter_state {
            ShooterState::Shooting { charge, recoil } => {
                if recoil == ShooterState::MAX_SHOOTING_RECOIL {
                    for shooter_position in self.shooters.iter() {
                        callbacks.push(SpaceCallback::GenerateProjectile {
                            shot_by_id: self.id(),
                            position: self.position + shooter_position.as_vec2(),
                            velocity: Vec2::X * 100.0,
                            color: Rgba([
                                25,
                                125,
                                55 + (200.0 * charge / ShooterState::MAX_CHARGE) as u8,
                                255,
                            ]),
                            damage: 1.5 * (charge / ShooterState::MAX_CHARGE).powf(0.25),
                        });
                    }
                    let new_charge = charge - ShooterState::SHOOTING_CHARGE_COST;
                    if new_charge > 0.0 {
                        let new_recoil = recoil - deltatime;
                        self.shooter_state = ShooterState::Shooting {
                            charge: new_charge,
                            recoil: new_recoil,
                        };
                    } else {
                        self.shooter_state = ShooterState::Recharging { charge: 0.0 };
                    }
                } else {
                    let new_recoil = recoil - deltatime;
                    if new_recoil > 0.0 {
                        self.shooter_state = ShooterState::Shooting {
                            charge,
                            recoil: new_recoil,
                        };
                    } else {
                        self.shooter_state = ShooterState::Ready { charge };
                    }
                }
            }

            ShooterState::Ready { charge } => {
                if self.auto_shoot {
                    self.shooter_state = ShooterState::Shooting {
                        charge,
                        recoil: ShooterState::MAX_SHOOTING_RECOIL,
                    };
                } else if charge < ShooterState::MAX_CHARGE {
                    let new_charge = (charge + ShooterState::CHARGE_RECOVERY_SPEED * deltatime)
                        .min(ShooterState::MAX_CHARGE);
                    self.shooter_state = ShooterState::Ready { charge: new_charge };
                }
            }

            ShooterState::Recharging { charge } => {
                let new_charge = charge + ShooterState::RECHARGE_RECOVERY_SPEED * deltatime;
                if new_charge < ShooterState::MAX_CHARGE {
                    self.shooter_state = ShooterState::Recharging { charge: new_charge };
                } else {
                    self.shooter_state = ShooterState::Ready {
                        charge: ShooterState::MAX_CHARGE,
                    };
                }
            }
        }

        callbacks
    }

    fn handle_space_callback(&mut self, callback: SpaceCallback) -> Vec<SpaceCallback> {
        match callback {
            SpaceCallback::DamageEntity { damage, .. } => {
                self.add_damage(damage);
                self.add_visual_effect(
                    VisualEffect::COLOR_MASK_LIFETIME,
                    VisualEffect::ColorMask {
                        color: Rgba([255, 0, 0, 0]),
                    },
                );
            }

            SpaceCallback::CollectFragment {
                resource, amount, ..
            } => {
                // FIXME: we need to figure out a way to handle fuel, since it cannot be handled as a normal resource
                // because we want it to be an f32.
                assert!(resource != Resource::FUEL);

                self.resources
                    .saturating_add(resource, amount, self.storage_capacity);
                self.used_storage_capacity = self.resources.used_storage_capacity()
            }

            _ => {}
        }
        vec![]
    }
}

impl Collider for SpaceshipEntity {
    fn collider_type(&self) -> ColliderType {
        ColliderType::Spaceship
    }
}

register_impl!(PlayerControlled for SpaceshipEntity);
register_impl!(!ResourceFragment for SpaceshipEntity);

impl PlayerControlled for SpaceshipEntity {
    fn fuel(&self) -> u32 {
        self.fuel.round() as u32
    }

    fn fuel_capacity(&self) -> u32 {
        self.fuel_capacity
    }

    fn resources(&self) -> &ResourceMap {
        &self.resources
    }

    fn storage_capacity(&self) -> u32 {
        self.storage_capacity
    }

    fn max_speed(&self) -> u32 {
        self.max_speed().round() as u32
    }

    fn charge(&self) -> u32 {
        match self.shooter_state {
            ShooterState::Ready { charge } => charge,
            ShooterState::Recharging { charge } => charge,
            ShooterState::Shooting { charge, .. } => charge,
        }
        .round() as u32
    }

    fn shooter_state(&self) -> ShooterState {
        self.shooter_state
    }

    fn max_charge(&self) -> u32 {
        ShooterState::MAX_CHARGE.round() as u32
    }

    fn thrust(&self) -> u32 {
        self.thrust().round() as u32
    }

    fn maneuverability(&self) -> u32 {
        self.maneuverability.round() as u32
    }

    fn current_durability(&self) -> u32 {
        self.current_durability.round() as u32
    }

    fn durability(&self) -> u32 {
        self.durability.round() as u32
    }

    fn handle_player_input(&mut self, input: PlayerInput) {
        match input {
            PlayerInput::MoveDown => self.accelerate(Direction::DOWN),
            PlayerInput::MoveUp => self.accelerate(Direction::UP),
            PlayerInput::MoveLeft => self.accelerate(Direction::LEFT),
            PlayerInput::MoveRight => self.accelerate(Direction::RIGHT),
            PlayerInput::MainButton => self.shoot(),
            PlayerInput::SecondButton => self.auto_shoot = !self.auto_shoot,
        }
    }
}

impl SpaceshipEntity {
    fn thrust(&self) -> f32 {
        self.base_thrust
            / (1.0 + SPEED_PENALTY_PER_UNIT_STORAGE * self.used_storage_capacity as f32)
    }

    fn max_speed(&self) -> f32 {
        self.base_speed / (1.0 + SPEED_PENALTY_PER_UNIT_STORAGE * self.used_storage_capacity as f32)
    }

    fn fuel_consumption(&self) -> f32 {
        self.base_fuel_consumption
            / (1.0 + FUEL_CONSUMPTION_PER_UNIT_STORAGE * self.used_storage_capacity as f32)
    }

    fn accelerate(&mut self, acceleration: Vec2) {
        if self.fuel == 0.0 {
            return;
        }

        self.acceleration = acceleration * self.thrust();
        self.fuel = (self.fuel - self.fuel_consumption()).max(0.0);
    }

    fn frame(&self) -> usize {
        self.tick % self.gif.len()
    }

    pub fn acceleration(&self) -> I16Vec2 {
        self.acceleration.as_i16vec2()
    }

    pub fn add_damage(&mut self, damage: f32) {
        self.current_durability = (self.current_durability - damage).max(0.0);
    }

    fn shoot(&mut self) {
        match self.shooter_state {
            ShooterState::Ready { charge } => {
                self.shooter_state = ShooterState::Shooting {
                    charge,
                    recoil: ShooterState::MAX_SHOOTING_RECOIL,
                }
            }
            _ => {}
        }
    }

    pub fn from_spaceship(
        spaceship: &Spaceship,
        resources: ResourceMap,
        fuel: u32,
    ) -> AppResult<Self> {
        let mut gif = vec![];
        let mut hit_boxes = vec![];
        let base_gif = spaceship.compose_image()?;
        for idx in 0..base_gif.len() {
            let base_image = rotate90(&base_gif[idx]);
            let (image, hit_box) = body_data_from_image(&base_image);

            gif.push(image);
            hit_boxes.push(hit_box);
        }

        let position = Vec2::ZERO.with_y((SCREEN_HEIGHT / 2) as f32);

        let mut engine_img = open_image(&spaceship.engine.select_mask_file(0))?;
        engine_img = rotate90(&engine_img);

        let y_offset = (engine_img.height() - gif[0].height()) / 2;
        let mut engine_exhaust = vec![];
        for x in 0..engine_img.width() {
            for y in 0..engine_img.height() {
                if let Some(pixel) = engine_img.get_pixel_checked(x, y) {
                    // If pixel is blue, it is at the exhaust position.
                    if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 255 && pixel[3] > 0 {
                        engine_exhaust.push(I16Vec2::new(x as i16, y as i16 - y_offset as i16));
                    }
                }
            }
        }

        let mut hull_img = open_image(&spaceship.hull.select_mask_file(0))?;
        hull_img = rotate90(&hull_img);

        let y_offset = (hull_img.height() - gif[0].height()) / 2;
        let mut shooters = vec![];
        for x in 0..hull_img.width() {
            for y in 0..hull_img.height() {
                if let Some(pixel) = hull_img.get_pixel_checked(x, y) {
                    // If pixel is blue, it is at the exhaust position.
                    if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 255 && pixel[3] > 0 {
                        shooters.push(I16Vec2::new(x as i16, y as i16 - y_offset as i16));
                    }
                }
            }
        }

        let used_storage_capacity = resources.used_storage_capacity();

        Ok(Self {
            id: 0,
            resources,
            used_storage_capacity,
            storage_capacity: spaceship.storage_capacity(),
            previous_position: position,
            position,
            gif,
            hit_boxes,
            current_durability: spaceship.current_durability() as f32,
            durability: spaceship.durability() as f32,
            base_thrust: spaceship.speed(0) * THRUST_MOD,
            base_speed: spaceship.speed(0) * MAX_SPACESHIP_SPEED_MOD,
            maneuverability: 0.0,
            fuel: fuel as f32,
            fuel_capacity: spaceship.fuel_capacity(),
            base_fuel_consumption: spaceship.fuel_consumption(0) * FUEL_CONSUMPTION_MOD,
            friction_coeff: FRICTION_COEFF,
            engine_exhaust,
            shooters,
            auto_shoot: false,
            velocity: Vec2::default(),
            acceleration: Vec2::default(),
            tick: 0,
            shooter_state: ShooterState::Ready {
                charge: ShooterState::MAX_CHARGE,
            },
            visual_effects: HashMap::new(),
        })
    }
}
