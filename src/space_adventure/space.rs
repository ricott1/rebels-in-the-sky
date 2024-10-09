use super::{
    asteroid::{AsteroidEntity, AsteroidSize},
    constants::{SCREEN_HEIGHT, SCREEN_WIDTH},
    particle::{ParticleEntity, ParticleState},
    spaceship::SpaceshipEntity,
    traits::{Entity, Sprite},
};
use crate::{
    image::utils::{ExtraImageUtils, TRAVELLING_BACKGROUND},
    types::AppResult,
    world::spaceship::Spaceship,
};
use image::{Rgba, RgbaImage};
use imageproc::{drawing::draw_hollow_rect_mut, rect::Rect};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

const MAX_LAYER: usize = 5;

#[derive(Default, Debug)]
pub struct Space {
    id: usize,
    tick: usize,
    background: RgbaImage,
    // entities: [HashMap<usize, Box<dyn Entity>>; MAX_LAYER], // Layered entities, to allow to print on different layers.
    id_to_layer: HashMap<usize, usize>,
    player_id: Option<usize>,
    player: Option<SpaceshipEntity>,
    asteroids: [HashMap<usize, AsteroidEntity>; MAX_LAYER], // Layered entities, to allow to print on different layers.
    particles: [HashMap<usize, ParticleEntity>; MAX_LAYER], // Layered entities, to allow to print on different layers.
}

impl Space {
    fn draw_entity<T: Entity>(base: &mut RgbaImage, entity: &T, debug_view: bool) -> AppResult<()> {
        let [x, y] = entity.position();
        base.copy_non_trasparent_from(entity.image(), x as u32, y as u32)?;

        if debug_view {
            let red = Rgba([255, 0, 5, 255]);
            let blue = Rgba([0, 0, 255, 255]);

            for (point, is_border) in entity.hit_box() {
                base.put_pixel(
                    point.0 as u32,
                    point.1 as u32,
                    if is_border { red } else { blue },
                );
            }
        }

        Ok(())
    }

    pub fn get_player(&self) -> Option<&SpaceshipEntity> {
        self.player.as_ref()
    }

    pub fn get_player_mut(&mut self) -> Option<&mut SpaceshipEntity> {
        self.player.as_mut()
    }

    pub fn generate_asteroid(
        &mut self,
        x: f64,
        y: f64,
        vx: f64,
        vy: f64,
        size: AsteroidSize,
    ) -> usize {
        self.insert_asteroid(AsteroidEntity::new(x, y, vx, vy, size))
    }

    fn insert_asteroid(&mut self, mut asteroid: AsteroidEntity) -> usize {
        let id = self.id.clone();
        let layer = asteroid.layer().clone();
        asteroid.set_id(id);
        self.asteroids[layer].insert(asteroid.id(), asteroid);
        self.id_to_layer.insert(id, layer);
        self.id += 1;
        id
    }

    pub fn remove_asteroid(&mut self, id: &usize) {
        if let Some(&layer) = self.id_to_layer.get(id) {
            self.asteroids[layer].remove(id);
        }
    }

    pub fn get_asteroid(&mut self, id: &usize) -> Option<&AsteroidEntity> {
        if let Some(&layer) = self.id_to_layer.get(id) {
            return self.asteroids[layer].get(id);
        }

        None
    }

    pub fn get_asteroid_mut(&mut self, id: &usize) -> Option<&mut AsteroidEntity> {
        if let Some(&layer) = self.id_to_layer.get(id) {
            return self.asteroids[layer].get_mut(id);
        }

        None
    }

    pub fn generate_particle(
        &mut self,
        x: f64,
        y: f64,
        vx: f64,
        vy: f64,
        color: Rgba<u8>,
        particle_state: ParticleState,
        layer: usize,
    ) -> usize {
        self.insert_particle(ParticleEntity::new(
            x,
            y,
            vx,
            vy,
            color,
            particle_state,
            layer,
        ))
    }

    fn insert_particle(&mut self, mut particle: ParticleEntity) -> usize {
        let id = self.id.clone();
        let layer = particle.layer().clone();
        particle.set_id(id);
        self.particles[layer].insert(particle.id(), particle);
        self.id_to_layer.insert(id, layer);
        self.id += 1;
        id
    }

    pub fn remove_particle(&mut self, id: &usize) {
        if let Some(&layer) = self.id_to_layer.get(id) {
            self.particles[layer].remove(id);
        }
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
            ..Default::default()
        })
    }

    pub fn with_spaceship(
        mut self,
        spaceship: &Spaceship,
        storage_units: u32,
        fuel: u32,
    ) -> AppResult<Self> {
        let mut spaceship = SpaceshipEntity::from_spaceship(spaceship, storage_units, fuel)?;
        let id = self.id.clone();
        self.player_id = Some(id);
        spaceship.set_id(id);
        self.id_to_layer.insert(id, spaceship.layer());
        self.player = Some(spaceship);
        self.id += 1;

        Ok(self)
    }

    pub fn update(&mut self, deltatime: f64) -> AppResult<()> {
        self.tick += 1;

        let mut callbacks = vec![];

        // Update from lowest layer
        for layer in 0..MAX_LAYER {
            if let Some(player) = self.player.as_mut() {
                if player.layer() == layer {
                    callbacks.append(&mut player.update(deltatime))
                }
            };

            for (_, entity) in self.asteroids[layer].iter_mut() {
                callbacks.append(&mut entity.update(deltatime));
            }

            for (_, entity) in self.particles[layer].iter_mut() {
                callbacks.append(&mut entity.update(deltatime));
            }
        }

        if callbacks.len() > 0 {
            log::info!("Space callbacks: {}", callbacks.len());
        }

        // Execute callbacks
        for cb in callbacks {
            if let Err(err) = cb.call(self) {
                log::error!("Space callback error: {err}");
            }
        }

        // Generate asteroids
        let rng = &mut ChaCha8Rng::from_entropy();
        if rng.gen_bool(0.0025) {
            let asteroid = AsteroidEntity::new_at_screen_edge();
            self.insert_asteroid(asteroid);
        }

        Ok(())
    }

    pub fn image(&self, debug_view: bool) -> AppResult<RgbaImage> {
        let mut base = self.background.clone();
        if debug_view {
            let white = Rgba([255, 255, 255, 255]);
            draw_hollow_rect_mut(
                &mut base,
                Rect::at(0, 0).of_size(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
                white,
            );
        }

        // Draw starting from lowest layer
        for layer in 0..MAX_LAYER {
            if let Some(player) = self.player.as_ref() {
                if player.layer() == layer {
                    Self::draw_entity(&mut base, player, debug_view)?;
                }
            };

            for (_, entity) in self.asteroids[layer].iter() {
                Self::draw_entity(&mut base, entity, debug_view)?;
            }

            for (_, entity) in self.particles[layer].iter() {
                Self::draw_entity(&mut base, entity, debug_view)?;
            }
        }

        Ok(base)
    }
}
