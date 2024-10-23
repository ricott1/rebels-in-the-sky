use super::{
    asteroid::{AsteroidEntity, AsteroidSize}, constants::{MAX_SCREEN_HEIGHT, MAX_SCREEN_WIDTH}, fragment::FragmentEntity, particle::ParticleEntity, projectile::ProjectileEntity, spaceship::SpaceshipEntity, traits::{resolve_collision_between, Entity}, utils::EntityState, visual_effects::VisualEffect, PlayerControlled, PlayerInput
};
use crate::{
    image::utils::{ExtraImageUtils, TRAVELLING_BACKGROUND},
    types::{AppResult, ResourceMap, SystemTimeTick, Tick},
    ui::{popup_message::PopupMessage, ui_callback::UiCallback},
    world::{resources::Resource, spaceship::Spaceship},
};
use anyhow::anyhow;
use glam::Vec2;
use image::{Rgba, RgbaImage};
use itertools::Itertools;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use strum::Display;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

const MAX_LAYER: usize = 5;

#[derive(Default, Debug, Display,Clone, Copy, PartialEq)]
enum SpaceState {
    Starting {
        time: Instant,
    },
    #[default]
    Running,
    Ending {
        time: Instant,
    },
}

impl SpaceState {
    pub const STARTING_DURATION:Duration = Duration::from_millis(2500);
    pub const ENDING_DURATION:Duration = Duration::from_millis(2500);
}

#[derive(Default, Debug)]
pub struct SpaceAdventure {
    id: usize,
    state: SpaceState,
    tick: usize,
    background: RgbaImage,
    // Layered entities, to allow to draw/interact on separate layers.
    entities: [HashMap<usize, Box<dyn Entity>>; MAX_LAYER],
    id_to_layer: HashMap<usize, usize>,
    player_id: Option<usize>,
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
        base.copy_non_trasparent_from(image, x as u32, y as u32)?;

        if debug_view {
            let red = Rgba([255, 0, 5, 255]);
            let gray = Rgba([105, 105, 105, 255]);

            for (point, &is_border) in entity.hit_box().iter() {
                let g_point = entity.position() + point;
                base.put_pixel(
                    g_point.x.max(0).min(MAX_SCREEN_WIDTH as i16) as u32,
                    g_point.y.max(0).min(MAX_SCREEN_HEIGHT as i16) as u32,
                    if is_border { red } else { gray },
                );
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
            SpaceState::Starting { .. } => true,
            _ => false,
        }
    }

    pub fn is_ending(&self) -> bool {
        match self.state {
            SpaceState::Ending { .. } => true,
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

    pub fn generate_asteroid(
        &mut self,
        position: Vec2,
        velocity: Vec2,
        size: AsteroidSize,
    ) -> usize {
        self.insert_entity(Box::new(AsteroidEntity::new(position, velocity, size)))
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

    pub fn new() -> AppResult<Self> {
        let bg = TRAVELLING_BACKGROUND.clone();
        let mut background = RgbaImage::new(bg.width() * 2, bg.height() * 2);
        background.copy_non_trasparent_from(&bg, 0, 0)?;
        background.copy_non_trasparent_from(&bg, bg.width(), 0)?;
        background.copy_non_trasparent_from(&bg, 0, bg.height())?;
        background.copy_non_trasparent_from(&bg, bg.width(), bg.height())?;

        Ok(Self {
            background,
            state: SpaceState::Starting { time: Instant::now() },
            ..Default::default()
        })
    }

    pub fn with_spaceship(
        mut self,
        spaceship: &Spaceship,
        resources: ResourceMap,
        fuel: u32,
    ) -> AppResult<Self> {
        let id = self.insert_entity(Box::new(SpaceshipEntity::from_spaceship(
            spaceship, resources, fuel,
        )?));
        self.player_id = Some(id);

        for _ in 0..10 {
            let asteroid = AsteroidEntity::new_at_screen_edge();
            self.insert_entity(Box::new(asteroid));
        }

        Ok(self)
    }

    pub fn handle_player_input(&mut self, input: PlayerInput) -> AppResult<()> {
        if self.state != SpaceState::Running {
            return Ok(());
        }

        let player = self.get_player_mut().ok_or(anyhow!("No player set"))?;
        let player_control: &mut dyn PlayerControlled = player
            .as_trait_mut()
            .expect("Player should implement PlayerControlled.");
        player_control.handle_player_input(input);

        Ok(())
    }

    pub fn stop_space_adventure(&mut self) {
        self.state = SpaceState::Ending {
            time: Instant::now(),
        };
    }

    pub fn update(&mut self, deltatime: f32) -> AppResult<Vec<UiCallback>> {

        match self.state {
            SpaceState::Starting { time } => {
                if time.elapsed() >= SpaceState::STARTING_DURATION {
                    self.state = SpaceState::Running;
                    return Ok(vec![]);
                }

            }

            SpaceState::Running => {
                if let Some(player) = self.get_player() {
                    let player_control: &dyn PlayerControlled = player
                        .as_trait_ref()
                        .expect("Player should implement PlayerControlled.");

                    if player_control.current_durability() == 0 {
                        self.stop_space_adventure();

                        return Ok(vec![
                        UiCallback::PushUiPopup { popup_message: 
                            PopupMessage::Ok{
                               message: "Danger! There's a breach in the hull.\nAll the resources in the stiva have been lost,\nyou need to go back to the base...".to_string() 
                                , is_skippable:true, tick:Tick::now()}
                            }
                    ]);
                    }
                }
            }

            SpaceState::Ending { time } => {
                if time.elapsed() >= SpaceState::ENDING_DURATION {
                    return Ok(vec![UiCallback::ReturnFromSpaceAdventure]);
                }
            }
        }

        self.tick += 1;

        let mut callbacks = vec![];

        // Update from lowest layer
        for layer in 0..MAX_LAYER {
            for (_, entity) in self.entities[layer].iter_mut() {
                callbacks.append(&mut entity.update(deltatime));
            }
        }

        // Resolve collisions
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

        // Execute callbacks
        for cb in callbacks {
            cb.call(self);
        }

        // Generate asteroids
        let rng = &mut ChaCha8Rng::from_entropy();
        if self.entity_count() < 50 && rng.gen_bool(0.01) {
            let asteroid = AsteroidEntity::new_at_screen_edge();
            self.insert_entity(Box::new(asteroid));
        }

        Ok(vec![])
    }

    pub fn image(&self, debug_view: bool) -> AppResult<RgbaImage> {
        let mut base = self.background.clone();

        // Draw starting from lowest layer
        for layer in 0..MAX_LAYER {
            for (_, entity) in self.entities[layer].iter() {
                Self::draw_entity(&mut base, entity, debug_view)?;
            }
        }

        
        match  self.state {
            // If adventure is starting, fade in.
            SpaceState::Starting { time } => {
                VisualEffect::FadeIn.apply_global_effect(&mut base, time.elapsed().as_millis() as f32/1000.0);
            }
             // If adventure is ending, fade out.
            SpaceState::Ending { time } => {
                VisualEffect::FadeOut.apply_global_effect(&mut base, time.elapsed().as_millis() as f32/1000.0);
            }
            SpaceState::Running=>{}
        }

        Ok(base)
    }
}
