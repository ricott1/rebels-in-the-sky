use super::{
    asteroid::{AsteroidEntity, AsteroidSize},
    collector::CollectorEntity,
    collisions::resolve_collision_between,
    constants::*,
    fragment::FragmentEntity,
    particle::ParticleEntity,
    player::{LocalPlayerView, PlayerOutcome, ShipLoadout},
    projectile::ProjectileEntity,
    space_callback::SpaceCallback,
    spaceship::{SpaceshipEntity, SpaceshipRole},
    traits::*,
    utils::EntityState,
    visual_effects::VisualEffect,
    wire::{NetVec, ParticleSpawn},
    ControllableSpaceship, PlayerInput,
};
use crate::{
    core::{resources::Resource, Shield, SpaceshipPrefab},
    image::{
        color_map::ColorMap,
        utils::{ExtraImageUtils, UNIVERSE_BACKGROUND},
    },
    space_adventure::{
        entity::Entity,
        shield::ShieldEntity,
        utils::{draw_hitbox, EntityMap},
    },
    types::{AppResult, SystemTimeTick, TeamId, Tick},
    ui::{PopupMessage, UiCallback},
};
use anyhow::anyhow;
use glam::{I16Vec2, Vec2};
use image::{imageops::crop_imm, Rgb};
use image::{Rgba, RgbaImage};
use itertools::Itertools;
use rand::{seq::IteratorRandom, RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use strum::{Display, IntoEnumIterator};

#[derive(Debug, Display, Clone, Copy, PartialEq)]
enum SpaceAdventureState {
    Starting { time: Instant },
    Running { time: Instant },
    Ending { time: Instant },
}

const MAX_PARTICLE_OUTBOX: usize = 1024;

#[derive(Debug, Display, Clone, Copy, PartialEq)]
enum AsteroidPlanetState {
    NotSpawned { should_spawn_asteroid: bool },
    Spawned { image_number: usize },
    Landed { image_number: usize },
}

#[derive(Debug, Display, Clone, Copy, PartialEq)]
enum SpaceAdventureObjective {
    Exploration,
    _Raid { target_team_id: TeamId },
}

pub(crate) fn space_background() -> RgbaImage {
    crop_imm(
        &UNIVERSE_BACKGROUND.clone(),
        0,
        0,
        BACKGROUND_IMAGE_SIZE.x,
        BACKGROUND_IMAGE_SIZE.y,
    )
    .to_image()
}

pub(crate) fn draw_entity_at(
    base: &mut RgbaImage,
    entity: &Entity,
    position: I16Vec2,
    debug_view: bool,
) {
    let x = position.x as i32;
    let y = position.y as i32;

    let image = if entity.should_apply_visual_effects() {
        &entity.apply_visual_effects(entity.image())
    } else {
        entity.image()
    };

    let img_w = image.width() as i32;
    let img_h = image.height() as i32;
    let base_w = base.width() as i32;
    let base_h = base.height() as i32;

    // Compute clipping
    let src_x = 0.max(-x);
    let src_y = 0.max(-y);
    let dst_x = 0.max(x);
    let dst_y = 0.max(y);

    let draw_w = (img_w - src_x).min(base_w - dst_x);
    let draw_h = (img_h - src_y).min(base_h - dst_y);

    // Nothing visible
    if draw_w <= 0 || draw_h <= 0 {
        // still draw hitbox if desired
        if debug_view {
            draw_hitbox(base, entity);
        }
        return;
    }

    base.copy_non_transparent_from_clipped(
        image,
        src_x as u32,
        src_y as u32,
        draw_w as u32,
        draw_h as u32,
        dst_x as u32,
        dst_y as u32,
    );

    if debug_view {
        draw_hitbox(base, entity);
    }
}

pub(crate) fn crop_to_screen(base: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    crop_imm(
        base,
        (MAX_ENTITY_POSITION.x - SCREEN_SIZE.x) / 2,
        (MAX_ENTITY_POSITION.y - SCREEN_SIZE.y) / 2,
        width,
        height,
    )
    .to_image()
}

#[derive(Debug)]
pub struct SpaceAdventure {
    id: usize,
    rng: ChaCha8Rng,
    state: SpaceAdventureState,
    _objective: SpaceAdventureObjective,
    tick: usize,
    background: RgbaImage,
    // Layered entities, to allow to draw/interact on separate layers.
    entities: Vec<EntityMap>,
    id_to_layer: HashMap<usize, usize>,
    host_id: Option<usize>,
    guest_id: Option<usize>,
    guest_destroyed: Option<PlayerOutcome>,
    record_particles: bool,
    particle_outbox: Vec<ParticleSpawn>,
    asteroid_planet_state: AsteroidPlanetState,
    enemy_ship_spawned: bool,
    gold_fragment_probability: f64,
}

impl SpaceAdventure {
    fn get_difficulty_level(time: Instant) -> usize {
        5 + time.elapsed().as_secs() as usize
    }

    pub fn duration(&self) -> Duration {
        let time = match self.state {
            SpaceAdventureState::Starting { time }
            | SpaceAdventureState::Running { time }
            | SpaceAdventureState::Ending { time } => time,
        };

        time.elapsed()
    }

    fn draw_entity(base: &mut RgbaImage, entity: &Entity, debug_view: bool) {
        draw_entity_at(base, entity, entity.position(), debug_view);
    }

    fn insert_entity(&mut self, mut entity: Entity) -> usize {
        let id = self.id;
        let layer = entity.layer();
        entity.set_id(id);

        self.entities[layer].insert(entity.id(), entity);
        self.id_to_layer.insert(id, layer);
        self.id += 1;
        id
    }

    pub const fn is_starting(&self) -> bool {
        matches!(self.state, SpaceAdventureState::Starting { .. })
    }

    pub const fn is_ending(&self) -> bool {
        matches!(self.state, SpaceAdventureState::Ending { .. })
    }

    pub fn entity_count(&self) -> usize {
        let guest_entities = self
            .guest_entity_ids()
            .iter()
            .filter(|id| self.get_entity(id).is_some())
            .count();
        (0..MAX_LAYER)
            .map(|l| self.entities[l].len())
            .sum::<usize>()
            + usize::from(self.host_id.is_some())
            - guest_entities
    }

    pub(crate) fn all_entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter().flat_map(|layer| layer.values())
    }

    pub fn outcome(&self, ship_id: usize) -> Option<PlayerOutcome> {
        let ship = self.get_ship(ship_id)?;
        Some(PlayerOutcome {
            resources: ship.resources().clone(),
            durability: ship.current_durability(),
        })
    }

    pub fn local_view(&self, ship_id: usize) -> Option<LocalPlayerView> {
        let ship = self.get_ship(ship_id)?;
        let (shield_durability, shield_max_durability) = ship
            .shield_id()
            .and_then(|id| self.get_entity(&id))
            .and_then(|entity| entity.as_shield().ok())
            .map(|shield| (shield.current_durability(), shield.max_durability()))
            .unwrap_or_default();
        Some(LocalPlayerView {
            durability: ship.current_durability(),
            max_durability: ship.max_durability(),
            shield_durability,
            shield_max_durability,
            charge: ship.current_charge(),
            max_charge: ship.max_charge(),
            is_recharging: ship.is_recharging(),
            fuel: ship.fuel(),
            fuel_capacity: ship.fuel_capacity(),
            resources: ship.resources().clone(),
            storage_capacity: ship.storage_capacity(),
        })
    }

    pub fn guest_outcome(&self) -> Option<PlayerOutcome> {
        self.guest_id.and_then(|id| self.outcome(id))
    }

    pub fn add_guest(&mut self, loadout: &ShipLoadout) -> AppResult<usize> {
        if self.guest_id.is_some() {
            return Err(anyhow!("There is already a guest"));
        }
        let ship_id = self.insert_player_ship(loadout, SpaceshipRole::Guest)?;
        if let Some(ship) = self.get_ship_mut(ship_id) {
            ship.set_invulnerable(STARTING_DURATION.as_secs_f32());
        }
        self.guest_id = Some(ship_id);
        self.guest_destroyed = None;
        self.record_particles = true;
        Ok(ship_id)
    }

    pub fn remove_guest(&mut self) -> Option<PlayerOutcome> {
        let outcome = self.guest_outcome();
        self.clear_guest();
        outcome
    }

    pub fn drain_particle_outbox(&mut self, into: &mut Vec<ParticleSpawn>) {
        into.append(&mut self.particle_outbox);
    }

    pub fn take_guest_destroyed(&mut self) -> Option<PlayerOutcome> {
        self.guest_destroyed.take()
    }

    fn guest_entity_ids(&self) -> Vec<usize> {
        let Some(ship) = self.guest_id.and_then(|id| self.get_ship(id)) else {
            return vec![];
        };
        std::iter::once(ship.id())
            .chain(ship.collector_id())
            .chain(ship.shield_id())
            .collect()
    }

    fn clear_guest(&mut self) {
        for id in self.guest_entity_ids() {
            self.remove_entity(&id);
        }
        self.guest_id = None;
        self.record_particles = false;
        self.particle_outbox.clear();
    }

    fn destroy_guest(&mut self) {
        let Some(guest_id) = self.guest_id else {
            return;
        };
        if let Some(ship) = self.get_ship_mut(guest_id) {
            ship.empty_hold();
        }
        self.guest_destroyed = self.outcome(guest_id);
        SpaceCallback::DestroyEntity { id: guest_id }.call(self);
        self.clear_guest();
    }

    pub const fn host_id(&self) -> Option<usize> {
        self.host_id
    }

    pub const fn guest_id(&self) -> Option<usize> {
        self.guest_id
    }

    pub fn player_ship_ids(&self) -> Vec<usize> {
        self.host_id.into_iter().chain(self.guest_id).collect()
    }

    pub const fn is_running(&self) -> bool {
        matches!(self.state, SpaceAdventureState::Running { .. })
    }

    pub fn get_ship(&self, id: usize) -> Option<&SpaceshipEntity> {
        match self.get_entity(&id) {
            Some(Entity::Spaceship(entity)) => Some(entity),
            _ => None,
        }
    }

    pub fn get_ship_mut(&mut self, id: usize) -> Option<&mut SpaceshipEntity> {
        match self.get_entity_mut(&id) {
            Some(Entity::Spaceship(entity)) => Some(entity),
            _ => None,
        }
    }

    pub fn host_ship(&self) -> Option<&SpaceshipEntity> {
        self.host_id.and_then(|id| self.get_ship(id))
    }

    pub fn host_ship_mut(&mut self) -> Option<&mut SpaceshipEntity> {
        let id = self.host_id?;
        self.get_ship_mut(id)
    }

    #[cfg(test)]
    pub fn force_running(&mut self) {
        self.state = SpaceAdventureState::Running {
            time: Instant::now(),
        };
    }

    #[cfg(test)]
    pub fn test_running() -> AppResult<Self> {
        let mut space = Self::new(false, 0.0)?.with_host(&ShipLoadout::test_default())?;
        space.force_running();
        Ok(space)
    }

    #[cfg(test)]
    pub fn test_kill_guest(&mut self) -> AppResult<()> {
        let guest_id = self.guest_id.ok_or_else(|| anyhow!("No guest"))?;
        if let Some(ship) = self.get_ship_mut(guest_id) {
            ship.set_invulnerable(0.0);
        }
        SpaceCallback::DamageEntity {
            id: guest_id,
            damage: 10_000.0,
        }
        .call(self);
        self.update(0.025)?;
        Ok(())
    }

    pub fn remove_entity(&mut self, id: &usize) {
        if let Some(&layer) = self.id_to_layer.get(id) {
            self.entities[layer].remove(id);
        }
    }

    pub fn get_entity(&self, id: &usize) -> Option<&Entity> {
        if let Some(&layer) = self.id_to_layer.get(id) {
            return self.entities[layer].get(id);
        }

        None
    }

    pub fn get_entity_mut(&mut self, id: &usize) -> Option<&mut Entity> {
        if let Some(&layer) = self.id_to_layer.get(id) {
            return self.entities[layer].get_mut(id);
        }

        None
    }

    pub(crate) fn generate_enemy_spaceship(&mut self) -> AppResult<usize> {
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());

        let mut color_map = ColorMap::random(rng);
        color_map.blue = Rgb([
            color_map.blue.0[0] / 6,
            color_map.blue.0[1] / 6,
            color_map.blue.0[2] / 6,
        ]);
        let spaceship = SpaceshipPrefab::iter()
            .filter(|s| s.spaceship().has_shooters())
            .choose(&mut rand::rng())
            .ok_or_else(|| anyhow!("There should be one spaceship available"))?
            .spaceship()
            .with_name("Baddy")
            .with_color_map(color_map);

        let shield_id = if spaceship.shield == Shield::None {
            None
        } else {
            Some(self.insert_entity(ShieldEntity::new_entity(
                spaceship.shield_max_durability(),
                spaceship.shield_damage_reduction(),
                false,
            )))
        };
        let enemy_id = self.insert_entity(SpaceshipEntity::random_enemy_spaceship_entity(
            &spaceship, shield_id,
        )?);
        self.enemy_ship_spawned = true;
        Ok(enemy_id)
    }

    pub fn generate_asteroid(
        &mut self,
        position: Vec2,
        velocity: Vec2,
        size: AsteroidSize,
    ) -> usize {
        self.insert_entity(AsteroidEntity::new_entity(
            position,
            velocity,
            size,
            self.gold_fragment_probability,
        ))
    }

    pub fn generate_particle(
        &mut self,
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        particle_state: EntityState,
        layer: usize,
    ) -> usize {
        if self.record_particles && self.particle_outbox.len() < MAX_PARTICLE_OUTBOX {
            self.particle_outbox.push(ParticleSpawn {
                pos: NetVec::from_vec2(position),
                vel: NetVec::from_vec2(velocity),
                color: color.0,
                state: particle_state,
                layer: layer as u8,
            });
        }
        self.insert_entity(ParticleEntity::new_entity(
            position,
            velocity,
            color,
            particle_state,
            layer,
        ))
    }

    pub fn generate_fragment(
        &mut self,
        position: Vec2,
        velocity: Vec2,
        resource: Resource,
        amount: u32,
    ) -> usize {
        self.insert_entity(FragmentEntity::new_entity(
            position, velocity, resource, amount,
        ))
    }

    pub fn generate_projectile(
        &mut self,
        shot_by_id: usize,
        shooter_shield_id: Option<usize>,
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        damage: f32,
        by_player: bool,
    ) -> usize {
        self.insert_entity(ProjectileEntity::new_entity(
            shot_by_id,
            shooter_shield_id,
            position,
            velocity,
            color,
            damage,
            by_player,
        ))
    }

    pub fn nearest_player_center(&self, from: I16Vec2) -> Option<I16Vec2> {
        let from = from.as_vec2();
        self.host_id
            .into_iter()
            .chain(self.guest_id)
            .filter_map(|id| self.get_ship(id))
            .map(|ship| ship.center())
            .min_by(|a, b| {
                a.as_vec2()
                    .distance_squared(from)
                    .total_cmp(&b.as_vec2().distance_squared(from))
            })
    }

    pub const fn asteroid_planet_found(&self) -> Option<usize> {
        match self.asteroid_planet_state {
            AsteroidPlanetState::Landed { image_number } => Some(image_number),
            _ => None,
        }
    }

    pub fn new(should_spawn_asteroid: bool, gold_fragment_probability: f64) -> AppResult<Self> {
        let background = space_background();

        let mut entities = vec![];
        for _ in 0..MAX_LAYER {
            entities.push(HashMap::new());
        }

        Ok(Self {
            id: 0,
            rng: ChaCha8Rng::from_rng(&mut rand::rng()),
            state: SpaceAdventureState::Starting {
                time: Instant::now(),
            },
            _objective: SpaceAdventureObjective::Exploration,
            tick: 0,
            background,
            entities,
            id_to_layer: HashMap::new(),
            host_id: None,
            guest_id: None,
            guest_destroyed: None,
            record_particles: false,
            particle_outbox: Vec::new(),
            asteroid_planet_state: AsteroidPlanetState::NotSpawned {
                should_spawn_asteroid,
            },
            enemy_ship_spawned: false,
            gold_fragment_probability,
        })
    }

    fn insert_player_ship(
        &mut self,
        loadout: &ShipLoadout,
        role: SpaceshipRole,
    ) -> AppResult<usize> {
        let collector_id = self.insert_entity(CollectorEntity::new_entity());
        let shield_id = if loadout.spaceship.shield == Shield::None {
            None
        } else {
            Some(self.insert_entity(ShieldEntity::new_entity(
                loadout.spaceship.shield_max_durability(),
                loadout.spaceship.shield_damage_reduction(),
                true,
            )))
        };
        Ok(self.insert_entity(SpaceshipEntity::player_spaceship_entity(
            loadout,
            Some(collector_id),
            shield_id,
            role,
        )?))
    }

    pub fn with_host(mut self, loadout: &ShipLoadout) -> AppResult<Self> {
        let ship_id = self.insert_player_ship(loadout, SpaceshipRole::Host)?;
        self.host_id = Some(ship_id);

        for _ in 0..10 {
            let asteroid = AsteroidEntity::new_at_screen_edge(self.gold_fragment_probability);
            self.insert_entity(asteroid);
        }

        Ok(self)
    }

    pub fn handle_player_input(&mut self, ship_id: usize, input: PlayerInput) -> AppResult<()> {
        if !self.is_running() {
            return Ok(());
        }

        let ship = self
            .get_ship_mut(ship_id)
            .ok_or_else(|| anyhow!("No spaceship {ship_id}"))?;
        if !ship.is_player() {
            return Err(anyhow!("Spaceship {ship_id} is not a player"));
        }
        ship.handle_player_input(input);

        Ok(())
    }

    pub fn stop_space_adventure(&mut self) {
        match self.state {
            SpaceAdventureState::Ending { .. } => {}
            _ => {
                self.state = SpaceAdventureState::Ending {
                    time: Instant::now(),
                }
            }
        }
    }

    pub fn land_on_asteroid(&mut self) {
        match self.asteroid_planet_state {
            AsteroidPlanetState::NotSpawned { .. } => {
                unreachable!("Should not be possible to land on unspawned asteroid planet.")
            }
            AsteroidPlanetState::Spawned { image_number } => {
                self.asteroid_planet_state = AsteroidPlanetState::Landed { image_number }
            }
            AsteroidPlanetState::Landed { .. } => {}
        }

        match self.state {
            SpaceAdventureState::Ending { .. } => {}
            _ => {
                self.state = SpaceAdventureState::Ending {
                    time: Instant::now(),
                }
            }
        }
    }

    pub fn update(&mut self, deltatime: f32) -> AppResult<Vec<UiCallback>> {
        let time = match self.state {
            SpaceAdventureState::Starting { time } => {
                if time.elapsed() >= STARTING_DURATION {
                    self.state = SpaceAdventureState::Running {
                        time: Instant::now(),
                    };
                    return Ok(vec![]);
                }
                time
            }

            SpaceAdventureState::Running { time } => {
                if self
                    .guest_id
                    .and_then(|id| self.get_ship(id))
                    .is_some_and(|ship| ship.current_durability() == 0)
                {
                    self.destroy_guest();
                }

                if let Some(player) = self.host_ship_mut() {
                    if player.current_durability() == 0 {
                        player.empty_hold();
                        self.stop_space_adventure();

                        return Ok(vec![UiCallback::PushUiPopup {
                            popup_message: PopupMessage::Message {
                                message: HULL_BREACH_MESSAGE.to_string(),
                                links: vec![],
                                level: log::Level::Info,
                                is_skippable: true,
                                timestamp: Tick::now(),
                            },
                        }]);
                    }
                }
                time
            }

            SpaceAdventureState::Ending { time } => {
                if time.elapsed() >= ENDING_DURATION {
                    return Ok(vec![UiCallback::ReturnFromSpaceAdventure]);
                }
                time
            }
        };

        self.tick += 1;

        let mut callbacks = vec![];

        // Update from lowest layer
        for layer_entities in self.entities.iter_mut() {
            for (_, entity) in layer_entities.iter_mut() {
                callbacks.append(&mut entity.update(deltatime));
            }
        }

        // Resolve collisions (only if state is running)
        if let SpaceAdventureState::Running { .. } = self.state {
            for layer in 0..MAX_LAYER {
                let layer_entities = self.entities[layer].keys().collect_vec();
                if layer_entities.is_empty() {
                    continue;
                }

                for idx in 0..layer_entities.len() - 1 {
                    let entity = self.entities[layer]
                        .get(layer_entities[idx])
                        .expect("Entity should exist.");
                    for other_id in layer_entities.iter().skip(idx + 1) {
                        let other = self.entities[layer]
                            .get(other_id)
                            .expect("Entity should exist.");
                        callbacks.append(&mut resolve_collision_between(entity, other, deltatime)?);
                    }
                }
            }
        }

        // Execute callbacks
        for cb in callbacks {
            cb.call(self);
        }

        // Generate asteroids
        let difficulty_level = Self::get_difficulty_level(time);
        if self.entity_count() < difficulty_level.min(MAX_ENTITY_COUNT_FOR_GENERATION)
            && self.rng.random_bool(ASTEROID_GENERATION_PROBABILITY)
        {
            let asteroid = AsteroidEntity::new_at_screen_edge(self.gold_fragment_probability);
            self.insert_entity(asteroid);
        }

        let mut ui_callbacks = vec![];

        if difficulty_level >= DIFFICULTY_FOR_ENEMY_SHIP_GENERATION && !self.enemy_ship_spawned {
            self.generate_enemy_spaceship()?;
        }

        if difficulty_level >= DIFFICULTY_FOR_ASTEROID_PLANET_GENERATION {
            if let AsteroidPlanetState::NotSpawned {
                should_spawn_asteroid,
            } = self.asteroid_planet_state
            {
                if should_spawn_asteroid {
                    let asteroid = AsteroidEntity::planet();
                    let id = self.insert_entity(asteroid);
                    self.asteroid_planet_state = AsteroidPlanetState::Spawned {
                        image_number: id % MAX_ASTEROID_PLANET_IMAGE_NUMBER,
                    };
                    ui_callbacks.push(UiCallback::PushUiPopup { popup_message:
                        PopupMessage::Message {
                        message: "You've found an asteroid! Bring the spaceship in touch with it to claim it.".to_string(),
                            links: vec![], level: log::Level::Info,
                            is_skippable:true, timestamp:Tick::now()}
                        });
                }
            }
        }

        // TODO: spawn enemy ship
        Ok(ui_callbacks)
    }

    pub fn image(&self, width: u32, height: u32, debug_view: bool) -> AppResult<RgbaImage> {
        let mut base = self.background.clone();

        // Draw starting from lowest layer
        for layer in 0..MAX_LAYER {
            for (_, entity) in self.entities[layer].iter() {
                Self::draw_entity(&mut base, entity, debug_view);
            }
        }

        match self.state {
            // If adventure is starting, fade in.
            SpaceAdventureState::Starting { time } => {
                VisualEffect::FadeIn
                    .apply_global_effect(&mut base, time.elapsed().as_millis() as f32 / 1000.0);
            }
            // If adventure is ending, fade out.
            SpaceAdventureState::Ending { time } => {
                VisualEffect::FadeOut
                    .apply_global_effect(&mut base, time.elapsed().as_millis() as f32 / 1000.0);
            }
            SpaceAdventureState::Running { .. } => {}
        }

        Ok(crop_to_screen(&base, width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space_adventure::player::ShipLoadout;
    use crate::space_adventure::spaceship::SpaceshipRole;
    use crate::space_adventure::SpaceCallback;
    use crate::types::StorableResourceMap;

    #[test]
    fn test_host_ship_has_host_role() -> AppResult<()> {
        let space = SpaceAdventure::test_running()?;
        let host = space.host_ship().expect("There should be a host ship");
        assert_eq!(host.role(), SpaceshipRole::Host);
        assert_eq!(space.player_ship_ids(), vec![host.id()]);
        Ok(())
    }

    #[test]
    fn test_input_goes_to_the_given_ship() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        let host_id = space.host_id().expect("There should be a host id");
        space.handle_player_input(host_id, PlayerInput::MoveRight)?;
        space.update(0.025)?;
        space.update(0.025)?;
        assert!(space.host_ship().expect("host").velocity_f32().x > 0.0);
        Ok(())
    }

    #[test]
    fn test_input_to_unknown_or_enemy_ship_fails() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        assert!(space
            .handle_player_input(usize::MAX, PlayerInput::MoveRight)
            .is_err());
        let enemy_id = space.generate_enemy_spaceship()?;
        assert!(space
            .handle_player_input(enemy_id, PlayerInput::MoveRight)
            .is_err());
        Ok(())
    }

    fn guest_loadout() -> ShipLoadout {
        let mut loadout = ShipLoadout::test_default();
        loadout.spaceship.shield = Shield::Small;
        loadout.resources.insert(Resource::GOLD, 10);
        loadout.resources.insert(Resource::SATOSHI, 500);
        loadout
    }

    #[test]
    fn test_add_guest_spawns_one_guest_ship() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        let guest_id = space.add_guest(&guest_loadout())?;
        assert_eq!(space.guest_id(), Some(guest_id));
        assert_eq!(
            space.get_ship(guest_id).expect("guest").role(),
            SpaceshipRole::Guest
        );
        assert!(space.add_guest(&guest_loadout()).is_err());
        Ok(())
    }

    #[test]
    fn test_remove_guest_returns_outcome_and_clears_entities() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        let guest_id = space.add_guest(&guest_loadout())?;
        let guest = space.get_ship(guest_id).expect("guest");
        let collector_id = guest.collector_id().expect("collector");
        let shield_id = guest.shield_id().expect("shield");

        let outcome = space.remove_guest().expect("outcome");

        assert_eq!(outcome.resources.value(&Resource::GOLD), 10);
        assert!(space.guest_id().is_none());
        for id in [guest_id, collector_id, shield_id] {
            assert!(space.get_entity(&id).is_none());
        }
        assert!(space.remove_guest().is_none());
        Ok(())
    }

    #[test]
    fn test_guest_ignores_damage_while_invulnerable() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        let guest_id = space.add_guest(&guest_loadout())?;
        let full = space
            .get_ship(guest_id)
            .expect("guest")
            .current_durability();

        SpaceCallback::DamageEntity {
            id: guest_id,
            damage: 5.0,
        }
        .call(&mut space);
        assert_eq!(
            space
                .get_ship(guest_id)
                .expect("guest")
                .current_durability(),
            full
        );

        space.update(STARTING_DURATION.as_secs_f32() + 0.1)?;
        SpaceCallback::DamageEntity {
            id: guest_id,
            damage: 5.0,
        }
        .call(&mut space);
        assert!(
            space
                .get_ship(guest_id)
                .expect("guest")
                .current_durability()
                < full
        );
        Ok(())
    }

    #[test]
    fn test_guest_death_wipes_hold_and_keeps_adventure_running() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        let guest_id = space.add_guest(&guest_loadout())?;
        space.test_kill_guest()?;

        let outcome = space.take_guest_destroyed().expect("guest destroyed");
        assert_eq!(outcome.durability, 0);
        assert_eq!(outcome.resources.value(&Resource::GOLD), 0);
        assert_eq!(outcome.resources.value(&Resource::SATOSHI), 500);
        assert!(space.guest_id().is_none());
        assert!(space.get_entity(&guest_id).is_none());
        assert!(space.is_running());
        assert!(!space
            .all_entities()
            .any(|e| matches!(e, Entity::Fragment(_))));
        assert!(space.take_guest_destroyed().is_none());
        Ok(())
    }

    #[test]
    fn test_guest_entities_do_not_count_for_difficulty() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        let before = space.entity_count();
        space.add_guest(&guest_loadout())?;
        assert_eq!(space.entity_count(), before);
        Ok(())
    }

    #[test]
    fn test_enemies_track_the_nearer_player() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        let guest_id = space.add_guest(&ShipLoadout::test_default())?;
        space
            .get_ship_mut(guest_id)
            .expect("guest")
            .set_position(Vec2::new(150.0, 100.0));
        let guest_center = space.get_ship(guest_id).expect("guest").center();
        let host_center = space.host_ship().expect("host").center();

        assert_eq!(
            space.nearest_player_center(guest_center + I16Vec2::new(5, 0)),
            Some(guest_center)
        );
        assert_eq!(space.nearest_player_center(host_center), Some(host_center));
        Ok(())
    }

    #[test]
    fn test_local_view_reports_the_ship() -> AppResult<()> {
        let space = SpaceAdventure::test_running()?;
        let host = space.host_ship().expect("host");
        let view = space.local_view(host.id()).expect("view");
        assert_eq!(view.fuel, host.fuel());
        assert_eq!(view.durability, host.current_durability());
        assert_eq!(view.storage_capacity, host.storage_capacity());
        assert_eq!(view.outcome(), space.outcome(host.id()).expect("outcome"));
        Ok(())
    }

    #[test]
    fn test_new_guest_does_not_inherit_a_pending_death() -> AppResult<()> {
        let mut space = SpaceAdventure::test_running()?;
        space.add_guest(&guest_loadout())?;
        space.test_kill_guest()?;

        space.add_guest(&guest_loadout())?;
        assert!(space.take_guest_destroyed().is_none());
        Ok(())
    }
}
