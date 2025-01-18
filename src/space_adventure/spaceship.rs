use super::collisions::HitBox;
use super::networking::ImageType;
use super::space_callback::SpaceCallback;
use super::utils::{body_data_from_image, EntityState};
use super::{constants::*, traits::*};
use crate::image::color_map::ColorMap;
use crate::image::components::{ImageComponent, SizedImageComponent};
use crate::image::spaceship::SpaceshipImage;
use crate::image::utils::open_image;
use crate::register_impl;
use crate::space_adventure::visual_effects::VisualEffect;
use crate::space_adventure::Direction;
use crate::types::*;
use crate::world::constants::{FUEL_CONSUMPTION_PER_UNIT_STORAGE, SPEED_PENALTY_PER_UNIT_STORAGE};
use crate::world::resources::Resource;
use crate::world::spaceship::SpaceshipPrefab;
use crate::{image::types::Gif, types::AppResult, world::spaceship::Spaceship};
use glam::{I16Vec2, Vec2};
use image::imageops::{rotate270, rotate90};
use image::{Pixel, Rgba, RgbaImage};
use itertools::Itertools;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use strum::IntoEnumIterator;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShooterState {
    Ready { charge: f32 },
    Shooting { charge: f32, recoil: f32 },
    Recharging { charge: f32 },
}

impl ShooterState {
    const SHOOTING_CHARGE_COST: f32 = 1.0;
    const MAX_CHARGE: f32 = 100.0;
    const CHARGE_RECOVERY_SPEED: f32 = 5.0;
    const RECHARGE_RECOVERY_SPEED: f32 = 3.0;
}

#[derive(Debug)]
struct Shooter {
    pub positions: Vec<I16Vec2>,
    pub damage: f32,
    pub max_recoil: f32,
    pub state: ShooterState,
}

impl Shooter {
    pub fn new(positions: Vec<I16Vec2>, damage: f32, fire_rate: f32) -> Self {
        Self {
            positions,
            damage,
            max_recoil: 1.0 / fire_rate,
            state: ShooterState::Ready {
                charge: ShooterState::MAX_CHARGE,
            },
        }
    }

    pub fn set_state(&mut self, state: ShooterState) {
        self.state = state;
    }

    pub fn shoot(&mut self, charge: f32) {
        self.set_state(ShooterState::Shooting {
            charge,
            recoil: self.max_recoil,
        })
    }
}

#[derive(Debug)]
pub struct SpaceshipEntity {
    id: usize,
    is_player: bool,
    base_spaceship: Spaceship,
    resources: ResourceMap,
    used_storage_capacity: u32, // Not necessary, we keep it to avoid recalculating them every time.
    storage_capacity: u32,
    fuel_capacity: u32,
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
    fuel: f32, // Fuel cannot be stored in the resource map because it's a f32 rather than a u32
    base_fuel_consumption: f32,
    friction_coeff: f32,
    tick: usize,
    gif: Gif,
    engine_exhaust: Vec<I16Vec2>, // Position of exhaust in relative coords
    shooter: Shooter,
    auto_shoot: bool,
    collector_id: usize,
    visual_effects: VisualEffectMap,
    releasing_scraps: bool,
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
        let rng = &mut ChaCha8Rng::from_entropy();

        // Generate fire particle if ship is damaged
        for _ in self.current_durability as usize..(0.5 * self.durability) as usize {
            if rng.gen_bool(0.2) {
                let position = self.center().as_vec2()
                    + Vec2::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0));

                let smoke_color_rng = rng.gen_range(0..=80);
                callbacks.push(SpaceCallback::GenerateParticle {
                    position,
                    velocity: -1.5 * self.acceleration.normalize()
                        + Vec2::new(rng.gen_range(-1.5..1.5), rng.gen_range(-2.5..2.5)),
                    color: Rgba([
                        105 + smoke_color_rng,
                        75 + smoke_color_rng,
                        75 + smoke_color_rng,
                        255,
                    ]),
                    particle_state: EntityState::Decaying {
                        lifetime: 1.5 + rng.gen_range(0.5..1.5),
                    },
                    layer: self.layer() + 1,
                });
            }
        }

        // Generate exhaust particles if accelerating
        if self.acceleration.length_squared() > 0.0 {
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

        // The spaceship must always remain on screen
        let min_position = (MAX_ENTITY_POSITION - SCREEN_SIZE).as_vec2() / 2.0;
        let max_position = min_position + SCREEN_SIZE.as_vec2() - self.size().as_vec2();

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

        callbacks.push(SpaceCallback::SetPosition {
            id: self.collector_id,
            position: self.center(),
        });

        callbacks
    }
}

impl Sprite for SpaceshipEntity {
    fn image(&self) -> &RgbaImage {
        &self.gif[self.frame()]
    }

    fn network_image_type(&self) -> ImageType {
        ImageType::Spaceship {
            hull: self.base_spaceship.hull,
            engine: self.base_spaceship.engine,
            storage: self.base_spaceship.storage,
            color_map: self.base_spaceship.image.color_map,
        }
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

    fn layer(&self) -> usize {
        1
    }

    fn update(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        // This is only triggered for enemy ships and not for the player ship.
        if !self.is_player && self.current_durability() == 0 {
            return vec![SpaceCallback::DestroyEntity { id: self.id }];
        }

        if !self.is_player {
            let rng = &mut rand::thread_rng();
            match self.shooter.state {
                ShooterState::Ready { charge } => {
                    if !self.auto_shoot
                        && charge > ShooterState::MAX_CHARGE * rng.gen_range(0.25..1.0)
                    {
                        self.auto_shoot = true;
                    }
                    if charge <= 0.25 {
                        self.auto_shoot = false;
                    }
                }
                _ => {}
            }
        }

        let mut callbacks = vec![];
        callbacks.append(&mut self.update_body(deltatime));
        callbacks.append(&mut self.update_sprite(deltatime));

        match self.shooter.state {
            ShooterState::Shooting { charge, recoil } => {
                if recoil == self.shooter.max_recoil {
                    for shooter_position in self.shooter.positions.iter() {
                        callbacks.push(SpaceCallback::GenerateProjectile {
                            shot_by_id: self.id(),
                            position: self.position + shooter_position.as_vec2(),
                            velocity: Vec2::X * 100.0 * self.orientation() as f32,
                            color: Rgba([
                                25,
                                125,
                                55 + (200.0 * charge / ShooterState::MAX_CHARGE) as u8,
                                255,
                            ]),
                            damage: self.shooter.damage,
                        });
                    }
                    let new_charge = charge - ShooterState::SHOOTING_CHARGE_COST;
                    if new_charge > 0.0 {
                        let new_recoil = recoil - deltatime;
                        self.shooter.set_state(ShooterState::Shooting {
                            charge: new_charge,
                            recoil: new_recoil,
                        });
                    } else {
                        self.shooter
                            .set_state(ShooterState::Recharging { charge: 0.0 });
                    }
                } else {
                    let new_recoil = recoil - deltatime;
                    if new_recoil > 0.0 {
                        self.shooter.set_state(ShooterState::Shooting {
                            charge,
                            recoil: new_recoil,
                        });
                    } else {
                        self.shooter.set_state(ShooterState::Ready { charge });
                    }
                }
            }

            ShooterState::Ready { charge } => {
                if self.auto_shoot {
                    self.shooter.set_state(ShooterState::Shooting {
                        charge,
                        recoil: self.shooter.max_recoil,
                    });
                } else if charge < ShooterState::MAX_CHARGE {
                    let new_charge = (charge + ShooterState::CHARGE_RECOVERY_SPEED * deltatime)
                        .min(ShooterState::MAX_CHARGE);
                    self.shooter
                        .set_state(ShooterState::Ready { charge: new_charge });
                }
            }

            ShooterState::Recharging { charge } => {
                let new_charge = charge + ShooterState::RECHARGE_RECOVERY_SPEED * deltatime;
                if new_charge < ShooterState::MAX_CHARGE {
                    self.shooter
                        .set_state(ShooterState::Recharging { charge: new_charge });
                } else {
                    self.shooter.set_state(ShooterState::Ready {
                        charge: ShooterState::MAX_CHARGE,
                    });
                }
            }
        }

        if self.releasing_scraps {
            let rng = &mut rand::thread_rng();
            callbacks.push(SpaceCallback::GenerateParticle {
                position: self.center().as_vec2(),
                velocity: Vec2::new(-6.0 + rng.gen_range(-0.5..0.5), rng.gen_range(-1.5..1.5)),
                color: Resource::SCRAPS.color(),
                particle_state: EntityState::Decaying {
                    lifetime: 3.0 + rng.gen_range(0.0..1.5),
                },
                layer: 2,
            });
            self.releasing_scraps = false;
        }
        callbacks
    }

    fn handle_space_callback(&mut self, callback: SpaceCallback) -> Vec<SpaceCallback> {
        match callback {
            SpaceCallback::DestroyEntity { .. } => {
                let rng = &mut rand::thread_rng();
                let mut callbacks = vec![SpaceCallback::DestroyEntity {
                    id: self.collector_id,
                }];

                let color_map = self.base_spaceship.image.color_map;
                let colors = [color_map.red, color_map.green, color_map.blue];
                for _ in 0..24 {
                    let color = colors.choose(rng).expect("There should be one color");
                    callbacks.push(SpaceCallback::GenerateParticle {
                        position: self.center().as_vec2(),
                        velocity: self.velocity
                            + Vec2::new(rng.gen_range(-10.0..10.0), rng.gen_range(-10.0..10.0)),
                        color: color.to_rgba(),
                        particle_state: EntityState::Decaying {
                            lifetime: 5.0 + rng.gen_range(-1.5..1.5),
                        },
                        layer: rng.gen_range(0..=2),
                    });
                }
                return callbacks;
            }

            SpaceCallback::DamageEntity { damage, .. } => {
                self.add_damage(damage);
                self.add_visual_effect(
                    VisualEffect::COLOR_MASK_LIFETIME,
                    VisualEffect::ColorMask { color: [255, 0, 0] },
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

    fn hit_box(&self) -> &HitBox {
        &self.hit_boxes[self.frame()]
    }
}

register_impl!(ControllableSpaceship for SpaceshipEntity);
register_impl!(!ResourceFragment for SpaceshipEntity);

impl ControllableSpaceship for SpaceshipEntity {
    fn is_player(&self) -> bool {
        self.is_player
    }

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
        match self.shooter.state {
            ShooterState::Ready { charge } => charge,
            ShooterState::Recharging { charge } => charge,
            ShooterState::Shooting { charge, .. } => charge,
        }
        .round() as u32
    }

    fn shooter_state(&self) -> ShooterState {
        self.shooter.state
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
            PlayerInput::Shoot => self.shoot(),
            PlayerInput::ToggleAutofire => self.auto_shoot = !self.auto_shoot,
            PlayerInput::ReleaseScraps => self.release_scraps(),
        }
    }
}

impl SpaceshipEntity {
    fn orientation(&self) -> i8 {
        if self.is_player {
            return 1;
        }
        return -1;
    }

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
        // Keep fuel in resources updated
        self.resources.insert(Resource::FUEL, self.fuel());
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
        match self.shooter.state {
            ShooterState::Ready { charge } => self.shooter.shoot(charge),
            _ => {}
        }
    }

    fn release_scraps(&mut self) {
        if self.resources.sub(Resource::SCRAPS, 1).is_ok() {
            self.releasing_scraps = true;
        }
    }

    pub fn from_spaceship(
        spaceship: &Spaceship,
        resources: ResourceMap,
        speed_bonus: f32,
        weapons_bonus: f32,
        fuel: u32,
        collector_id: usize,
    ) -> AppResult<Self> {
        let mut gif = vec![];
        let mut hit_boxes = vec![];
        let base_gif = spaceship.compose_image()?;
        for idx in 0..base_gif.len() {
            let base_image = rotate90(&base_gif[idx]);
            let (image, hit_box) = body_data_from_image(&base_image, true);

            gif.push(image);
            hit_boxes.push(hit_box);
        }

        let position = Vec2::new(
            (MAX_ENTITY_POSITION.x - SCREEN_SIZE.x) as f32 / 2.0,
            0.5 * SCREEN_SIZE.y as f32,
        );

        let mut engine_img = open_image(&spaceship.engine.select_mask_file())?;
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

        let size = SpaceshipImage::size(&spaceship.hull);
        let mut shooter_img = spaceship.shooter.image(size)?;
        shooter_img = rotate90(&shooter_img);

        let y_offset = shooter_img.height().saturating_sub(gif[0].height()) / 2;
        let mut shooter_positions = vec![];
        for x in 0..shooter_img.width() {
            for y in 0..shooter_img.height() {
                if let Some(pixel) = shooter_img.get_pixel_checked(x, y) {
                    // If pixel is blue, it is at the shooter position.
                    if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 255 && pixel[3] > 0 {
                        shooter_positions.push(I16Vec2::new(x as i16, y as i16 - y_offset as i16));
                    }
                }
            }
        }

        let used_storage_capacity = resources.used_storage_capacity();

        let shooter = Shooter::new(
            shooter_positions,
            spaceship.damage() * weapons_bonus,
            spaceship.fire_rate(),
        );

        Ok(Self {
            id: 0,
            is_player: true,
            base_spaceship: spaceship.clone(),
            resources,
            used_storage_capacity,
            storage_capacity: spaceship.storage_capacity(),
            previous_position: position,
            position,
            gif,
            hit_boxes,
            current_durability: spaceship.current_durability() as f32,
            durability: spaceship.durability() as f32,
            base_thrust: spaceship.speed(0) * THRUST_MOD * speed_bonus.powf(0.35), // Assigning the full speed bonus would make the ship too fast
            base_speed: spaceship.speed(0) * MAX_SPACESHIP_SPEED_MOD,
            maneuverability: 0.0,
            fuel: fuel as f32,
            fuel_capacity: spaceship.fuel_capacity(),
            base_fuel_consumption: spaceship.fuel_consumption_per_tick(0) * FUEL_CONSUMPTION_MOD,
            friction_coeff: FRICTION_COEFF,
            engine_exhaust,
            shooter,
            auto_shoot: false,
            velocity: Vec2::default(),
            acceleration: Vec2::default(),
            tick: 0,
            collector_id,
            visual_effects: HashMap::new(),
            releasing_scraps: true,
        })
    }

    pub fn random_enemy(collector_id: usize) -> AppResult<Self> {
        let rng = &mut ChaCha8Rng::from_entropy();
        let mut gif = vec![];
        let mut hit_boxes = vec![];
        let spaceship = SpaceshipPrefab::iter()
            .collect_vec()
            .choose(&mut rand::thread_rng())
            .expect("There shiuld be one spaceship available")
            .spaceship("Baddy".to_string())
            .with_color_map(ColorMap::random(rng));

        let base_gif = spaceship.compose_image()?;
        for idx in 0..base_gif.len() {
            let base_image = rotate270(&base_gif[idx]);
            let (image, hit_box) = body_data_from_image(&base_image, true);

            gif.push(image);
            hit_boxes.push(hit_box);
        }

        let position = Vec2::new(SCREEN_SIZE.x as f32, SCREEN_SIZE.y as f32 / 2.0);

        let mut engine_img = open_image(&spaceship.engine.select_mask_file())?;
        engine_img = rotate270(&engine_img);

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

        let size = SpaceshipImage::size(&spaceship.hull);
        let mut shooter_img = spaceship.shooter.image(size)?;
        shooter_img = rotate270(&shooter_img);

        let y_offset = (shooter_img.height() - gif[0].height()) / 2;
        let mut shooter_positions = vec![];
        for x in 0..shooter_img.width() {
            for y in 0..shooter_img.height() {
                if let Some(pixel) = shooter_img.get_pixel_checked(x, y) {
                    // If pixel is blue, it is at the shooter position.
                    if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 255 && pixel[3] > 0 {
                        shooter_positions.push(I16Vec2::new(x as i16, y as i16 - y_offset as i16));
                    }
                }
            }
        }

        let resources = ResourceMap::new();
        let used_storage_capacity = 0;
        let fuel = spaceship.fuel_capacity() as f32;
        let storage_capacity = spaceship.storage_capacity();
        let current_durability = spaceship.current_durability() as f32;
        let durability = spaceship.durability() as f32;
        let base_thrust = spaceship.speed(0) * THRUST_MOD;
        let base_speed = spaceship.speed(0) * MAX_SPACESHIP_SPEED_MOD;
        let fuel_capacity = spaceship.fuel_capacity();
        let base_fuel_consumption = spaceship.fuel_consumption_per_tick(0) * FUEL_CONSUMPTION_MOD;

        let shooter = Shooter::new(shooter_positions, spaceship.damage(), spaceship.fire_rate());

        Ok(Self {
            id: 0,
            is_player: false,
            base_spaceship: spaceship,
            resources,
            used_storage_capacity,
            storage_capacity,
            previous_position: position,
            position,
            gif,
            hit_boxes,
            current_durability,
            durability,
            base_thrust,
            base_speed,
            maneuverability: 0.0,
            fuel,
            fuel_capacity,
            base_fuel_consumption,
            friction_coeff: FRICTION_COEFF,
            engine_exhaust,
            shooter,
            auto_shoot: false,
            velocity: Vec2::default(),
            acceleration: Vec2::default(),
            tick: 0,
            collector_id,
            visual_effects: HashMap::new(),
            releasing_scraps: true,
        })
    }
}
