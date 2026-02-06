use super::{
    asteroid::{AsteroidEntity, AsteroidSize},
    collector::CollectorEntity,
    collisions::resolve_collision_between,
    constants::*,
    fragment::FragmentEntity,
    particle::ParticleEntity,
    projectile::ProjectileEntity,
    spaceship::SpaceshipEntity,
    traits::*,
    utils::EntityState,
    visual_effects::VisualEffect,
    ControllableSpaceship, PlayerInput,
};
use crate::{
    core::{resources::Resource, spaceship::Spaceship, Shield, SpaceshipPrefab},
    image::{
        color_map::ColorMap,
        utils::{ExtraImageUtils, UNIVERSE_BACKGROUND},
    },
    space_adventure::{
        entity::Entity,
        shield::ShieldEntity,
        utils::{draw_hitbox, EntityMap},
    },
    types::{AppResult, ResourceMap, SystemTimeTick, Tick},
    ui::{PopupMessage, UiCallback},
};
use anyhow::anyhow;
use glam::Vec2;
use image::{imageops::crop_imm, Rgb};
use image::{Rgba, RgbaImage};
use itertools::Itertools;
use rand::{seq::IteratorRandom, Rng, SeedableRng};
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

impl SpaceAdventureState {
    pub const STARTING_DURATION: Duration = Duration::from_millis(3500);
    pub const ENDING_DURATION: Duration = Duration::from_millis(2500);
}

#[derive(Debug, Display, Clone, Copy, PartialEq)]
enum AsteroidPlanetState {
    NotSpawned { should_spawn_asteroid: bool },
    Spawned { image_number: usize },
    Landed { image_number: usize },
}

#[derive(Debug)]
pub struct SpaceAdventure {
    id: usize,
    rng: ChaCha8Rng,
    state: SpaceAdventureState,
    tick: usize,
    background: RgbaImage,
    // Layered entities, to allow to draw/interact on separate layers.
    entities: Vec<EntityMap>,
    id_to_layer: HashMap<usize, usize>,
    player_id: Option<usize>,
    asteroid_planet_state: AsteroidPlanetState,
    enemy_ship_spawned: bool,
    gold_fragment_probability: f64,
}

impl SpaceAdventure {
    fn get_difficulty_level(time: Instant) -> usize {
        5 + time.elapsed().as_secs() as usize
    }

    fn draw_entity(base: &mut RgbaImage, entity: &Entity, debug_view: bool) {
        let pos = entity.position();
        let x = pos.x as i32;
        let y = pos.y as i32;

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
        (0..MAX_LAYER)
            .map(|l| self.entities[l].len())
            .sum::<usize>()
            + if self.player_id.is_some() { 1 } else { 0 }
    }

    pub fn get_player(&self) -> Option<&SpaceshipEntity> {
        if let Some(player_id) = self.player_id {
            match self.get_entity(&player_id) {
                Some(Entity::Spaceship(entity)) => Some(entity),
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn get_player_mut(&mut self) -> Option<&mut SpaceshipEntity> {
        if let Some(player_id) = self.player_id {
            match self.get_entity_mut(&player_id) {
                Some(Entity::Spaceship(entity)) => Some(entity),
                _ => None,
            }
        } else {
            None
        }
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

    fn generate_enemy_spaceship(&mut self) -> AppResult<usize> {
        let rng = &mut ChaCha8Rng::from_os_rng();

        let mut color_map = ColorMap::random(rng);
        color_map.blue = Rgb([
            color_map.blue.0[0] / 6,
            color_map.blue.0[1] / 6,
            color_map.blue.0[2] / 6,
        ]);
        let spaceship = SpaceshipPrefab::iter()
            .filter(|s| s.spaceship().shooting_points() > 0)
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
    ) -> usize {
        self.insert_entity(ProjectileEntity::new_entity(
            shot_by_id,
            shooter_shield_id,
            position,
            velocity,
            color,
            damage,
        ))
    }

    pub const fn asteroid_planet_found(&self) -> Option<usize> {
        match self.asteroid_planet_state {
            AsteroidPlanetState::Landed { image_number } => Some(image_number),
            _ => None,
        }
    }

    pub fn new(should_spawn_asteroid: bool, gold_fragment_probability: f64) -> AppResult<Self> {
        // Crop background
        let background = crop_imm(
            &UNIVERSE_BACKGROUND.clone(),
            0,
            0,
            BACKGROUND_IMAGE_SIZE.x,
            BACKGROUND_IMAGE_SIZE.y,
        )
        .to_image();

        let mut entities = vec![];
        for _ in 0..MAX_LAYER {
            entities.push(HashMap::new());
        }

        Ok(Self {
            id: 0,
            rng: ChaCha8Rng::from_os_rng(),
            state: SpaceAdventureState::Starting {
                time: Instant::now(),
            },
            tick: 0,
            background,
            entities,
            id_to_layer: HashMap::new(),
            player_id: None,
            asteroid_planet_state: AsteroidPlanetState::NotSpawned {
                should_spawn_asteroid,
            },
            enemy_ship_spawned: false,
            gold_fragment_probability,
        })
    }

    pub fn with_player(
        mut self,
        spaceship: &Spaceship,
        resources: ResourceMap,
        speed_bonus: f32,
        weapons_bonus: f32,
        fuel: u32,
    ) -> AppResult<Self> {
        let collector_id = Some(self.insert_entity(CollectorEntity::new_entity()));
        let shield_id = if spaceship.shield == Shield::None {
            None
        } else {
            Some(self.insert_entity(ShieldEntity::new_entity(
                spaceship.shield_max_durability(),
                spaceship.shield_damage_reduction(),
            )))
        };
        let id = self.insert_entity(SpaceshipEntity::player_spaceship_entity(
            spaceship,
            resources,
            speed_bonus,
            weapons_bonus,
            fuel,
            collector_id,
            shield_id,
        )?);
        self.player_id = Some(id);

        for _ in 0..10 {
            let asteroid = AsteroidEntity::new_at_screen_edge(self.gold_fragment_probability);
            self.insert_entity(asteroid);
        }

        Ok(self)
    }

    pub fn handle_player_input(&mut self, input: PlayerInput) -> AppResult<()> {
        match self.state {
            SpaceAdventureState::Running { .. } => {}
            _ => return Ok(()),
        }

        let player = self
            .get_player_mut()
            .ok_or_else(|| anyhow!("No player set"))?;
        player.handle_player_input(input);

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
                if time.elapsed() >= SpaceAdventureState::STARTING_DURATION {
                    self.state = SpaceAdventureState::Running {
                        time: Instant::now(),
                    };
                    return Ok(vec![]);
                }
                time
            }

            SpaceAdventureState::Running { time } => {
                if let Some(player) = self.get_player_mut() {
                    if player.current_durability() == 0 {
                        player.resources_mut().insert(Resource::GOLD, 0);
                        player.resources_mut().insert(Resource::RUM, 0);
                        player.resources_mut().insert(Resource::SCRAPS, 0);
                        self.stop_space_adventure();

                        return Ok(vec![
                            UiCallback::PushUiPopup { popup_message:
                                PopupMessage::Ok {
                                    message: "Danger! There's a breach in the hull.\nAll the resources in the stiva have been lost,\nyou need to go back to the base...".to_string(),
                                    is_skippable:true, tick:Tick::now()}
                                }
                        ]);
                    }
                }
                time
            }

            SpaceAdventureState::Ending { time } => {
                if time.elapsed() >= SpaceAdventureState::ENDING_DURATION {
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
                        PopupMessage::Ok {
                        message: "You've found an asteroid! Bring the spaceship in touch with it to claim it.".to_string(),
                            is_skippable:true, tick:Tick::now()}
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

        // Crop centered subimage of size SCREEN_SIZE
        let image = crop_imm(
            &base,
            (MAX_ENTITY_POSITION.x - SCREEN_SIZE.x) / 2,
            (MAX_ENTITY_POSITION.y - SCREEN_SIZE.y) / 2,
            width,
            height,
        )
        .to_image();

        Ok(image)
    }
}
