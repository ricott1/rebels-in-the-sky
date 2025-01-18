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
    image::utils::{ExtraImageUtils, TRAVELLING_BACKGROUND},
    types::{AppResult, ResourceMap, SystemTimeTick, Tick},
    ui::{popup_message::PopupMessage, ui_callback::UiCallback},
    world::{resources::Resource, spaceship::Spaceship},
};
use anyhow::anyhow;
use glam::Vec2;
use image::imageops::crop_imm;
use image::{Rgba, RgbaImage};
use itertools::Itertools;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use strum::Display;

#[derive(Debug, Display, Clone, Copy, PartialEq)]
enum SpaceAdventureState {
    Starting { time: Instant },
    Running { time: Instant },
    Ending { time: Instant },
}

impl SpaceAdventureState {
    pub const STARTING_DURATION: Duration = Duration::from_millis(2500);
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
    entities: Vec<HashMap<usize, Box<dyn Entity>>>,
    id_to_layer: HashMap<usize, usize>,
    player_id: Option<usize>,
    asteroid_planet_state: AsteroidPlanetState,
    enemy_ship_spawned: bool,
    gold_fragment_probability: f64,
}

impl SpaceAdventure {
    fn draw_entity(
        base: &mut RgbaImage,
        entity: &Box<dyn Entity>,
        debug_view: bool,
    ) -> AppResult<()> {
        let [x, y] = entity.position().to_array();
        let image = if entity.should_apply_visual_effects() {
            &entity.apply_visual_effects(&entity.image())
        } else {
            entity.image()
        };

        let cropped_image = if x as u32 + image.width() > base.width()
            && y as u32 + image.height() > base.height()
        {
            &crop_imm(
                image,
                0,
                0,
                base.width().saturating_sub(x as u32),
                base.height().saturating_sub(y as u32),
            )
            .to_image()
        } else if x as u32 + image.width() > base.width() {
            &crop_imm(
                image,
                0,
                0,
                base.width().saturating_sub(x as u32),
                image.height(),
            )
            .to_image()
        } else if y as u32 + image.height() > base.height() {
            &crop_imm(
                image,
                0,
                0,
                image.width(),
                base.height().saturating_sub(y as u32),
            )
            .to_image()
        } else {
            image
        };
        base.copy_non_trasparent_from(cropped_image, x as u32, y as u32)?;

        if debug_view {
            let gray = Rgba([105, 105, 105, 255]);

            for (point, &is_border) in entity.hit_box().iter() {
                let g_point = entity.position() + point;
                if is_border {
                    base.put_pixel(
                        g_point.x.max(0).min(MAX_ENTITY_POSITION.x as i16) as u32,
                        g_point.y.max(0).min(MAX_ENTITY_POSITION.y as i16) as u32,
                        gray,
                    );
                }
            }
        }

        Ok(())
    }

    fn insert_entity(&mut self, mut entity: Box<dyn Entity>) -> usize {
        let id = self.id.clone();
        let layer = entity.layer().clone();
        entity.set_id(id);

        self.entities[layer].insert(entity.id(), entity);
        self.id_to_layer.insert(id, layer);
        self.id += 1;
        id
    }

    pub fn is_starting(&self) -> bool {
        match self.state {
            SpaceAdventureState::Starting { .. } => true,
            _ => false,
        }
    }

    pub fn is_ending(&self) -> bool {
        match self.state {
            SpaceAdventureState::Ending { .. } => true,
            _ => false,
        }
    }

    pub fn entity_count(&self) -> usize {
        (0..MAX_LAYER)
            .map(|l| self.entities[l].len())
            .sum::<usize>()
            + if self.player_id.is_some() { 1 } else { 0 }
    }

    pub fn get_player(&self) -> Option<&Box<dyn Entity>> {
        if let Some(player_id) = self.player_id {
            self.get_entity(&player_id)
        } else {
            None
        }
    }

    pub fn get_player_mut(&mut self) -> Option<&mut Box<dyn Entity>> {
        if let Some(player_id) = self.player_id {
            self.get_entity_mut(&player_id)
        } else {
            None
        }
    }

    pub fn remove_entity(&mut self, id: &usize) {
        if let Some(&layer) = self.id_to_layer.get(id) {
            self.entities[layer].remove(id);
        }
    }

    pub fn get_entity(&self, id: &usize) -> Option<&Box<dyn Entity>> {
        if let Some(&layer) = self.id_to_layer.get(id) {
            return self.entities[layer].get(id);
        }

        None
    }

    pub fn get_entity_mut(&mut self, id: &usize) -> Option<&mut Box<dyn Entity>> {
        if let Some(&layer) = self.id_to_layer.get(id) {
            return self.entities[layer].get_mut(id);
        }

        None
    }

    pub fn generate_enemy_spaceship(&mut self) -> AppResult<usize> {
        let collector_id = self.insert_entity(Box::new(CollectorEntity::new()));
        let enemy_id = self.insert_entity(Box::new(SpaceshipEntity::random_enemy(collector_id)?));
        self.enemy_ship_spawned = true;
        Ok(enemy_id)
    }

    pub fn generate_asteroid(
        &mut self,
        position: Vec2,
        velocity: Vec2,
        size: AsteroidSize,
    ) -> usize {
        self.insert_entity(Box::new(AsteroidEntity::new(
            position,
            velocity,
            size,
            self.gold_fragment_probability,
        )))
    }

    pub fn generate_particle(
        &mut self,
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        particle_state: EntityState,
        layer: usize,
    ) -> usize {
        self.insert_entity(Box::new(ParticleEntity::new(
            position,
            velocity,
            color,
            particle_state,
            layer,
        )))
    }

    pub fn generate_fragment(
        &mut self,
        position: Vec2,
        velocity: Vec2,
        resource: Resource,
        amount: u32,
    ) -> usize {
        self.insert_entity(Box::new(FragmentEntity::new(
            position, velocity, resource, amount,
        )))
    }

    pub fn generate_projectile(
        &mut self,
        shot_by_id: usize,
        position: Vec2,
        velocity: Vec2,
        color: Rgba<u8>,
        damage: f32,
    ) -> usize {
        self.insert_entity(Box::new(ProjectileEntity::new(
            shot_by_id, position, velocity, color, damage,
        )))
    }

    pub fn asteroid_planet_found(&self) -> Option<usize> {
        match self.asteroid_planet_state {
            AsteroidPlanetState::Landed { image_number } => Some(image_number),
            _ => None,
        }
    }

    pub fn new(should_spawn_asteroid: bool, gold_fragment_probability: f64) -> AppResult<Self> {
        let bg = TRAVELLING_BACKGROUND.clone();
        let mut background = RgbaImage::new(bg.width() * 2, bg.height() * 3);
        background.copy_non_trasparent_from(&bg, 0, 0)?;
        background.copy_non_trasparent_from(&bg, bg.width(), 0)?;
        background.copy_non_trasparent_from(&bg, 0, bg.height())?;
        background.copy_non_trasparent_from(&bg, bg.width(), bg.height())?;
        background.copy_non_trasparent_from(&bg, 0, 2 * bg.height())?;
        background.copy_non_trasparent_from(&bg, bg.width(), 2 * bg.height())?;
        // Crop background
        let background = crop_imm(
            &background,
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
            rng: ChaCha8Rng::from_entropy(),
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
        let collector_id = self.insert_entity(Box::new(CollectorEntity::new()));
        let id = self.insert_entity(Box::new(SpaceshipEntity::from_spaceship(
            spaceship,
            resources,
            speed_bonus,
            weapons_bonus,
            fuel,
            collector_id,
        )?));
        self.player_id = Some(id);

        for _ in 0..10 {
            let asteroid = AsteroidEntity::new_at_screen_edge(self.gold_fragment_probability);
            self.insert_entity(Box::new(asteroid));
        }

        Ok(self)
    }

    pub fn handle_player_input(&mut self, input: PlayerInput) -> AppResult<()> {
        match self.state {
            SpaceAdventureState::Running { .. } => {}
            _ => return Ok(()),
        }

        let player = self.get_player_mut().ok_or(anyhow!("No player set"))?;
        let player_control: &mut dyn ControllableSpaceship = player
            .as_trait_mut()
            .expect("Player should implement ControllableSpaceship.");
        player_control.handle_player_input(input);

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
                if let Some(player) = self.get_player() {
                    let player_control: &dyn ControllableSpaceship = player
                        .as_trait_ref()
                        .expect("Player should implement ControllableSpaceship.");

                    if player_control.current_durability() == 0 {
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
        for layer in 0..MAX_LAYER {
            for (_, entity) in self.entities[layer].iter_mut() {
                callbacks.append(&mut entity.update(deltatime));
            }
        }

        // Resolve collisions (only if state is running)
        match self.state {
            SpaceAdventureState::Running { .. } => {
                for layer in 0..MAX_LAYER {
                    let layer_entities = self.entities[layer].keys().collect_vec();
                    if layer_entities.len() == 0 {
                        continue;
                    }

                    for idx in 0..layer_entities.len() - 1 {
                        let entity = self.entities[layer]
                            .get(layer_entities[idx])
                            .expect("Entity should exist.");
                        for other_idx in idx + 1..layer_entities.len() {
                            let other = self.entities[layer]
                                .get(layer_entities[other_idx])
                                .expect("Entity should exist.");
                            callbacks.append(&mut resolve_collision_between(entity, other));
                        }
                    }
                }
            }
            _ => {}
        }

        // Execute callbacks
        for cb in callbacks {
            cb.call(self);
        }

        // Generate asteroids
        let difficulty_level = time.elapsed().as_secs() as usize;
        if self.entity_count() < difficulty_level.min(250)
            && self.rng.gen_bool(ASTEROID_GENERATION_PROBABILITY)
        {
            let asteroid = AsteroidEntity::new_at_screen_edge(self.gold_fragment_probability);
            self.insert_entity(Box::new(asteroid));
        }

        let mut ui_callbacks = vec![];

        if difficulty_level > DIFFICULTY_FOR_ASTEROID_PLANET_GENERATION {
            match self.asteroid_planet_state {
                AsteroidPlanetState::NotSpawned {
                    should_spawn_asteroid,
                } => {
                    if should_spawn_asteroid {
                        let asteroid = AsteroidEntity::planet();
                        let id = self.insert_entity(Box::new(asteroid));
                        self.asteroid_planet_state = AsteroidPlanetState::Spawned {
                            image_number: id % MAX_ASTEROID_PLANET_IMAGE_TYPE,
                        };
                        ui_callbacks.push(UiCallback::PushUiPopup { popup_message:
                            PopupMessage::Ok {
                            message: "You've found an asteroid! Bring the spaceship in touch with it to claim it.".to_string(),
                                is_skippable:true, tick:Tick::now()}
                            });
                    }
                }
                _ => {}
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
                if let Err(e) = Self::draw_entity(&mut base, entity, debug_view) {
                    log::error!("Error in draw_entity {}: {}", entity.id(), e);
                }
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
