use super::collisions::HitBox;
use super::core_constants::{FUEL_CONSUMPTION_PER_UNIT_STORAGE, SPEED_PENALTY_PER_UNIT_STORAGE};
use super::entity::Entity;
use super::resources::Resource;
use super::space_callback::SpaceCallback;
use super::utils::{body_data_from_image, EntityState};
use super::visual_effects::VisualEffect;
use super::Direction;
use super::{constants::*, traits::*};
use crate::image::components::{ImageComponent, SizedImageComponent};
use crate::image::spaceship::SpaceshipImage;
use crate::image::utils::{open_image, Gif, LightMaskStyle};
use crate::types::*;
use crate::{core::spaceship::Spaceship, types::AppResult};
use glam::{I16Vec2, Vec2};
use image::imageops::{rotate270, rotate90};
use image::{Pixel, Rgba, RgbaImage};
use rand::seq::IndexedRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShooterState {
    Ready,
    Shooting { recoil: f32 },
}

impl ShooterState {
    const SHOOTING_CHARGE_COST: f32 = 2.05;
}

#[derive(Debug)]
struct ShooterInSpaceAdventure {
    pub positions: Vec<I16Vec2>,
    pub damage: f32,
    pub max_recoil: f32,
    pub autofire: bool,
    pub state: ShooterState,
}

impl ShooterInSpaceAdventure {
    pub fn new(positions: Vec<I16Vec2>, damage: f32, fire_rate: f32) -> Self {
        Self {
            positions,
            damage,
            max_recoil: 1.0 / fire_rate,
            autofire: false,
            state: ShooterState::Ready,
        }
    }

    fn set_state(&mut self, state: ShooterState) {
        self.state = state;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChargeUnitState {
    Ready,
    Recharging,
}

#[derive(Debug)]
struct ChargeUnitInSpaceAdventure {
    pub current_charge: f32,
    pub max_charge: f32,
    pub state: ChargeUnitState,
}

impl ChargeUnitInSpaceAdventure {
    const CHARGE_RECOVERY_PER_SECOND: f32 = 2.5;
    const RECHARGE_RECOVERY_PER_SECOND: f32 = 2.25;

    pub fn new(max_charge: f32) -> Self {
        Self {
            current_charge: max_charge,
            max_charge,
            state: ChargeUnitState::Ready,
        }
    }

    fn set_state(&mut self, state: ChargeUnitState) {
        self.state = state;
    }

    fn sub_charge(&mut self, value: f32) {
        self.current_charge = (self.current_charge - value).clamp(0.0, self.max_charge)
    }

    fn add_charge(&mut self, value: f32) {
        self.current_charge = (self.current_charge + value).clamp(0.0, self.max_charge)
    }

    fn recharge(&mut self, deltatime: f32) {
        match self.state {
            ChargeUnitState::Ready => {
                self.add_charge(ChargeUnitInSpaceAdventure::CHARGE_RECOVERY_PER_SECOND * deltatime);
            }
            ChargeUnitState::Recharging => {
                self.add_charge(
                    ChargeUnitInSpaceAdventure::RECHARGE_RECOVERY_PER_SECOND * deltatime,
                );
            }
        }
    }
}

#[derive(Debug)]
pub struct SpaceshipEntity {
    id: usize,
    is_player: bool,
    spaceship: Spaceship,
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
    max_durability: f32,
    damage_reduction: f32,
    base_thrust: f32,
    base_speed: f32,
    maneuverability: f32,
    fuel: f32, // Fuel cannot be stored in the resource map because it's a f32 rather than a u32
    base_fuel_consumption: f32,
    friction_coeff: f32,
    tick: usize,
    gif: Gif,
    engine_exhaust: Vec<I16Vec2>, // GamePosition of exhaust in relative coords
    shooter: Option<ShooterInSpaceAdventure>, // FIXME: it should be its own entity (as the engine and storage) so they can be damaged separately.
    charge_unit: ChargeUnitInSpaceAdventure,
    collector_id: Option<usize>,
    shield_id: Option<usize>,
    visual_effects: VisualEffectMap,
    releasing_scraps: bool,
    pending_input_callbacks: Vec<SpaceCallback>,
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
        let rng = &mut ChaCha8Rng::from_os_rng();

        // Generate fire particle if ship is damaged
        for _ in self.current_durability as usize..(0.5 * self.max_durability) as usize {
            if rng.random_bool(0.2) {
                let position = self.center().as_vec2()
                    + Vec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0));

                let smoke_color_rng = rng.random_range(0..=80);
                callbacks.push(SpaceCallback::GenerateParticle {
                    position,
                    velocity: -1.5 * self.acceleration.normalize()
                        + Vec2::new(rng.random_range(-1.5..1.5), rng.random_range(-2.5..2.5)),
                    color: Rgba([
                        105 + smoke_color_rng,
                        75 + smoke_color_rng,
                        75 + smoke_color_rng,
                        255,
                    ]),
                    particle_state: EntityState::Decaying {
                        lifetime: 1.5 + rng.random_range(0.5..1.5),
                    },
                    layer: self.layer() + 1,
                });
            }
        }

        // Generate exhaust particles if accelerating
        if self.acceleration.length_squared() > 0.0 {
            for &point in self.engine_exhaust.iter() {
                if self.velocity.length_squared() < self.acceleration.length_squared() / 2.0
                    || rng.random_bool(0.25)
                {
                    let layer = rng.random_range(0..2);
                    callbacks.push(SpaceCallback::GenerateParticle {
                        position: self.position + point.as_vec2(),
                        velocity: -3.0 * self.acceleration.normalize()
                            + Vec2::new(rng.random_range(-0.5..0.5), rng.random_range(-0.5..0.5)),
                        color: Rgba([
                            205 + rng.random_range(0..50),
                            55 + rng.random_range(0..200),
                            rng.random_range(0..55),
                            255,
                        ]),
                        particle_state: EntityState::Decaying {
                            lifetime: 2.0 + rng.random_range(0.0..1.5),
                        },
                        layer,
                    });
                }
            }
        }

        self.acceleration -= self.friction_coeff * self.velocity;

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

        if self.is_player() {
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
        }

        if let Some(id) = self.collector_id {
            callbacks.push(SpaceCallback::SetCenterPosition {
                id,
                center: self.center(),
            });
        }

        if let Some(id) = self.shield_id {
            callbacks.push(SpaceCallback::SetCenterPosition {
                id,
                center: self.center(),
            });
        }

        callbacks
    }
}

impl Sprite for SpaceshipEntity {
    fn image(&self) -> &RgbaImage {
        &self.gif[self.frame()]
    }

    fn should_apply_visual_effects<'a>(&self) -> bool {
        !self.visual_effects.is_empty()
    }

    fn apply_visual_effects<'a>(&'a self, image: &'a RgbaImage) -> RgbaImage {
        let mut image = image.clone();
        for (effect, time) in self.visual_effects.iter() {
            effect.apply(self, &mut image, *time);
        }
        image
    }

    fn add_visual_effect(&mut self, duration: f32, effect: VisualEffect) {
        self.visual_effects.insert(effect, duration);
    }

    fn remove_visual_effect(&mut self, effect: &VisualEffect) {
        self.visual_effects.remove(effect);
    }

    fn update_sprite(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        for (_, lifetime) in self.visual_effects.iter_mut() {
            *lifetime -= deltatime;
        }

        self.visual_effects.retain(|_, lifetime| *lifetime > 0.0);

        vec![]
    }
}

impl GameEntity for SpaceshipEntity {
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
        let mut callbacks = self.pending_input_callbacks.to_owned();
        self.pending_input_callbacks.clear();

        // This is only triggered for enemy ships and not for the player ship.
        if !self.is_player() {
            if self.current_durability() == 0 {
                callbacks.push(SpaceCallback::DestroyEntity { id: self.id });

                if let Some(id) = self.collector_id {
                    callbacks.push(SpaceCallback::DestroyEntity { id });
                }

                if let Some(id) = self.shield_id {
                    callbacks.push(SpaceCallback::DestroyEntity { id });
                }

                return callbacks;
            }

            callbacks.push(SpaceCallback::TrackPlayer { id: self.id });
        }

        callbacks.append(&mut self.update_body(deltatime));
        callbacks.append(&mut self.update_sprite(deltatime));

        if self.current_charge() == 0 {
            self.charge_unit.set_state(ChargeUnitState::Recharging);

            if let Some(id) = self.collector_id {
                callbacks.push(SpaceCallback::DeactivateEntity { id });
            }

            if let Some(id) = self.shield_id {
                callbacks.push(SpaceCallback::DeactivateEntity { id });
            }
        }

        if self.charge_unit.state == ChargeUnitState::Ready {
            // Turn on collector again automatically.
            // FIXME: this is unnecessary most of the time, as it is already active.
            if let Some(id) = self.collector_id {
                callbacks.push(SpaceCallback::ActivateEntity { id });
            }

            if let Some(shooter) = self.shooter.as_mut() {
                match shooter.state {
                    ShooterState::Ready => {
                        if shooter.autofire {
                            shooter.set_state(ShooterState::Shooting {
                                recoil: shooter.max_recoil,
                            });
                        }
                    }

                    ShooterState::Shooting { recoil } => {
                        // If recoil is at max, actually shoot
                        if recoil == shooter.max_recoil {
                            for shooter_position in shooter.positions.iter() {
                                callbacks.push(SpaceCallback::GenerateProjectile {
                                    shot_by_id: self.id,
                                    position: self.position + shooter_position.as_vec2(),
                                    velocity: Vec2::X
                                        * 100.0
                                        * if self.is_player { 1.0 } else { -1.0 },
                                    color: Rgba([
                                        25,
                                        125,
                                        (55.0
                                            + (200.0 * self.charge_unit.current_charge
                                                / self.charge_unit.max_charge))
                                            .min(255.0)
                                            as u8,
                                        255,
                                    ]),
                                    damage: shooter.damage,
                                });
                            }
                            self.charge_unit
                                .sub_charge(ShooterState::SHOOTING_CHARGE_COST);

                            shooter.set_state(ShooterState::Shooting {
                                recoil: shooter.max_recoil - deltatime, // Reduce it a bit so that it does not shoot again immediately.
                            })
                        }
                        // Otherwise reduce recoil
                        else {
                            let new_recoil = recoil - deltatime;
                            if new_recoil > 0.0 {
                                shooter.set_state(ShooterState::Shooting { recoil: new_recoil });
                            } else {
                                shooter.set_state(ShooterState::Ready);
                            }
                        }
                    }
                }
            }
        }

        self.charge_unit.recharge(deltatime);

        if self.releasing_scraps {
            let rng = &mut rand::rng();
            callbacks.push(SpaceCallback::GenerateParticle {
                position: self.center().as_vec2(),
                velocity: Vec2::new(
                    -6.0 + rng.random_range(-0.5..0.5),
                    rng.random_range(-1.5..1.5),
                ),
                color: Resource::SCRAPS.color(),
                particle_state: EntityState::Decaying {
                    lifetime: 3.0 + rng.random_range(0.0..1.5),
                },
                layer: 2,
            });
            self.releasing_scraps = false;
        }

        callbacks
    }

    fn handle_space_callback(&mut self, callback: SpaceCallback) -> Vec<SpaceCallback> {
        match callback {
            SpaceCallback::SetAcceleration { acceleration, .. } => {
                self.thrust_towards(acceleration);
            }
            SpaceCallback::DestroyEntity { .. } => {
                let rng = &mut rand::rng();
                let mut callbacks = vec![];

                if let Some(id) = self.collector_id {
                    callbacks.push(SpaceCallback::DestroyEntity { id });
                }

                let color_map = self.spaceship.image.color_map;
                let colors = [color_map.red, color_map.green, color_map.blue];

                let position = self.center().as_vec2();
                for _ in 0..32 {
                    let color = colors.choose(rng).expect("There should be one color");
                    callbacks.push(SpaceCallback::GenerateParticle {
                        position,
                        velocity: self.velocity
                            + Vec2::new(
                                rng.random_range(-10.0..10.0),
                                rng.random_range(-10.0..10.0),
                            ) * 3.0,
                        color: color.to_rgba(),
                        particle_state: EntityState::Decaying {
                            lifetime: 5.0 + rng.random_range(-1.5..1.5),
                        },
                        layer: rng.random_range(0..=2),
                    });
                }

                for _ in 4..8 {
                    callbacks.push(SpaceCallback::GenerateFragment {
                        position,
                        velocity: Vec2::new(
                            rng.random_range(-3.5..3.5),
                            rng.random_range(-3.5..3.5),
                        ),
                        resource: Resource::GOLD,
                        amount: 1,
                    });
                }
                for _ in 10..16 {
                    callbacks.push(SpaceCallback::GenerateFragment {
                        position,
                        velocity: Vec2::new(
                            rng.random_range(-3.5..3.5),
                            rng.random_range(-3.5..3.5),
                        ) * 2.0,
                        resource: Resource::SCRAPS,
                        amount: 2,
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

            SpaceCallback::ReleaseScraps { .. } => self.release_scraps(),
            SpaceCallback::Shoot { .. } => self.shoot(),
            SpaceCallback::ToggleAutofire { .. } => self.toggle_autofire(),
            SpaceCallback::UseCharge { amount, .. } => {
                self.charge_unit.sub_charge(amount);
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

    fn resources_mut(&mut self) -> &mut ResourceMap {
        &mut self.resources
    }

    fn storage_capacity(&self) -> u32 {
        self.storage_capacity
    }

    fn max_speed(&self) -> u32 {
        self.max_speed().round() as u32
    }

    fn is_recharging(&self) -> bool {
        self.charge_unit.state == ChargeUnitState::Recharging
    }

    fn current_charge(&self) -> u32 {
        self.charge_unit.current_charge as u32
    }

    fn max_charge(&self) -> u32 {
        self.charge_unit.max_charge as u32
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

    fn max_durability(&self) -> u32 {
        self.max_durability.round() as u32
    }

    fn handle_player_input(&mut self, input: PlayerInput) {
        if let Some(cb) = match input {
            PlayerInput::MoveDown => Some(SpaceCallback::SetAcceleration {
                id: self.id(),
                acceleration: Direction::Down.as_vec2(),
            }),
            PlayerInput::MoveUp => Some(SpaceCallback::SetAcceleration {
                id: self.id(),
                acceleration: Direction::Up.as_vec2(),
            }),
            PlayerInput::MoveLeft => Some(SpaceCallback::SetAcceleration {
                id: self.id(),
                acceleration: Direction::Left.as_vec2(),
            }),
            PlayerInput::MoveRight => Some(SpaceCallback::SetAcceleration {
                id: self.id(),
                acceleration: Direction::Right.as_vec2(),
            }),
            PlayerInput::Shoot => Some(SpaceCallback::Shoot { id: self.id() }),
            PlayerInput::ReleaseScraps => Some(SpaceCallback::ReleaseScraps { id: self.id() }),
            PlayerInput::ToggleAutofire => Some(SpaceCallback::ToggleAutofire { id: self.id() }),
            PlayerInput::ToggleShield => {
                self.shield_id.map(|id| SpaceCallback::ToggleShield { id })
            }
        } {
            self.pending_input_callbacks.push(cb);
        }
    }
}

impl SpaceshipEntity {
    pub fn spaceship(&self) -> &Spaceship {
        &self.spaceship
    }

    pub fn shield_id(&self) -> Option<usize> {
        self.shield_id
    }

    pub fn toggle_autofire(&mut self) {
        if let Some(shooter) = self.shooter.as_mut() {
            shooter.autofire = !shooter.autofire
        }
    }

    pub fn set_autofire(&mut self, value: bool) {
        if let Some(shooter) = self.shooter.as_mut() {
            shooter.autofire = value;
        }
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

    pub fn thrust_towards(&mut self, direction: Vec2) {
        if self.fuel == 0.0 {
            return;
        }

        self.acceleration = direction * self.thrust();
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

    fn add_damage(&mut self, damage: f32) {
        self.current_durability =
            (self.current_durability - damage * self.damage_reduction).max(0.0);
    }

    fn shoot(&mut self) {
        if let Some(shooter) = self.shooter.as_mut() {
            if shooter.state == ShooterState::Ready {
                shooter.set_state(ShooterState::Shooting {
                    recoil: shooter.max_recoil,
                });
            }
        }
    }

    fn release_scraps(&mut self) {
        if self.resources.sub(Resource::SCRAPS, 1).is_ok() {
            self.releasing_scraps = true;
        }
    }

    fn from_spaceship(
        spaceship: &Spaceship,
        resources: ResourceMap,
        speed_bonus: f32,
        weapons_bonus: f32,
        fuel: u32,
        collector_id: Option<usize>,
        shield_id: Option<usize>,
        is_player: bool,
    ) -> AppResult<Entity> {
        let mut gif = vec![];
        let mut hit_boxes = vec![];
        let base_gif = spaceship.compose_image(Some(LightMaskStyle::radial()))?;
        for gif_frame in base_gif.iter() {
            let base_image = if is_player {
                rotate90(gif_frame)
            } else {
                rotate270(gif_frame)
            };
            let (image, hit_box) = body_data_from_image(&base_image, true);

            gif.push(image);
            hit_boxes.push(hit_box);
        }

        let position = if is_player {
            Vec2::new(
                (MAX_ENTITY_POSITION.x as f32 - SCREEN_SIZE.x as f32) / 2.0,
                0.5 * SCREEN_SIZE.y as f32,
            )
        } else {
            Vec2::new(MAX_ENTITY_POSITION.x as f32, SCREEN_SIZE.y as f32 / 2.0)
        };

        let mut engine_img = open_image(&spaceship.engine.select_mask_file())?;
        engine_img = if is_player {
            rotate90(&engine_img)
        } else {
            rotate270(&engine_img)
        };

        let y_offset = (engine_img.height().saturating_sub(gif[0].height())) / 2;
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
        shooter_img = if is_player {
            rotate90(&shooter_img)
        } else {
            rotate270(&shooter_img)
        };

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

        let shooter = if spaceship.fire_rate() == 0.0 {
            None
        } else {
            Some(ShooterInSpaceAdventure::new(
                shooter_positions,
                spaceship.damage() * weapons_bonus,
                spaceship.fire_rate(),
            ))
        };

        let charge_unit = ChargeUnitInSpaceAdventure::new(spaceship.charge_unit.max_charge());

        Ok(Entity::Spaceship(Self {
            id: 0,
            is_player,
            spaceship: spaceship.clone(),
            resources,
            used_storage_capacity,
            storage_capacity: spaceship.storage_capacity(),
            previous_position: position,
            position,
            gif,
            hit_boxes,
            current_durability: spaceship.current_durability() as f32,
            max_durability: spaceship.max_durability() as f32,
            damage_reduction: 1.0,
            base_thrust: spaceship.speed(0) * THRUST_MOD * speed_bonus.powf(0.35), // Assigning the full speed bonus would make the ship too fast
            base_speed: spaceship.speed(0) * MAX_SPACESHIP_SPEED_MOD,
            maneuverability: 0.0,
            fuel: fuel as f32,
            fuel_capacity: spaceship.fuel_capacity(),
            base_fuel_consumption: spaceship.fuel_consumption_per_tick(0) * FUEL_CONSUMPTION_MOD,
            friction_coeff: FRICTION_COEFF,
            engine_exhaust,
            shooter,
            charge_unit,
            velocity: Vec2::default(),
            acceleration: Vec2::default(),
            tick: 0,
            collector_id,
            shield_id,
            visual_effects: HashMap::new(),
            releasing_scraps: false,
            pending_input_callbacks: Vec::new(),
        }))
    }

    pub fn player_spaceship_entity(
        spaceship: &Spaceship,
        resources: ResourceMap,
        speed_bonus: f32,
        weapons_bonus: f32,
        fuel: u32,
        collector_id: Option<usize>,
        shield_id: Option<usize>,
    ) -> AppResult<Entity> {
        Self::from_spaceship(
            spaceship,
            resources,
            speed_bonus,
            weapons_bonus,
            fuel,
            collector_id,
            shield_id,
            true,
        )
    }

    pub fn random_enemy_spaceship_entity(
        spaceship: &Spaceship,
        shield_id: Option<usize>,
    ) -> AppResult<Entity> {
        let resources = ResourceMap::new();

        Self::from_spaceship(
            spaceship,
            resources,
            1.0,
            1.0,
            spaceship.fuel_capacity(),
            None,
            shield_id,
            false,
        )
    }
}
