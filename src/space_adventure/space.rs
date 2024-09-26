use super::{spaceship::SpaceshipEntity, traits::Entity};
use crate::{
    image::{
        types::GifFrame,
        utils::{ExtraImageUtils, TRAVELLING_BACKGROUND},
    },
    types::{AppResult, Tick},
    world::{constants::SECONDS, spaceship::Spaceship},
};
use image::{GenericImageView, RgbaImage};
use ratatui::layout::Rect;
use std::collections::HashMap;

const FRICTION_COEFF: f64 = 0.05;
const MAX_WIDTH: u32 = 160;
const MAX_HEIGHT: u32 = 80;

#[derive(Default, Debug)]
pub struct Space {
    id: usize,
    tick: usize,
    background: GifFrame,
    entities: HashMap<usize, Box<dyn Entity>>,
}

impl Space {
    fn insert_entity(&mut self, entity: Box<dyn Entity>) -> usize {
        let id = self.id.clone();
        self.entities.insert(id, entity);
        self.id += 1;
        id
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

    pub fn with_spaceship(mut self, spaceship: &Spaceship) -> AppResult<Self> {
        let spaceship_entity = SpaceshipEntity::from_spaceship(spaceship)?;
        self.insert_entity(Box::new(spaceship_entity));
        Ok(self)
    }

    pub fn current_id(&self) -> usize {
        self.id
    }

    pub fn player(&self) -> &Box<dyn Entity> {
        self.entities
            .get(&0)
            .expect("There should be a player entity")
    }

    pub fn player_mut(&mut self) -> &mut Box<dyn Entity> {
        self.entities
            .get_mut(&0)
            .expect("There should be a player entity")
    }

    pub fn update(&mut self, deltatime_millis: Tick) -> AppResult<()> {
        self.tick += 1;
        let deltatime = deltatime_millis as f64 / SECONDS as f64;

        for (_, entity) in self.entities.iter_mut() {
            let [x, y] = entity.position();
            let [vx, vy] = entity.velocity();
            let [mut ax, mut ay] = entity.accelleration();
            ax = ax - FRICTION_COEFF * vx;
            ay = ay - FRICTION_COEFF * vy;
            let new_velocity = [vx + ax * deltatime, vy + ay * deltatime];
            entity.set_velocity(new_velocity);
            entity.set_accelleration([0.0, 0.0]);

            let [nvx, nvy] = entity.velocity();

            let size = entity.size(self.tick);
            let new_position = [
                (x + nvx * deltatime).min((MAX_WIDTH - size[0]) as f64),
                (y + nvy * deltatime).min((MAX_HEIGHT - size[1]) as f64),
            ];
            entity.set_position(new_position);
        }

        Ok(())
    }

    pub fn gif_frame(&self, frame_size: Rect) -> AppResult<GifFrame> {
        let mut base = self.background.clone();

        for (_, entity) in self.entities.iter() {
            let [x, y] = entity.position();
            base.copy_non_trasparent_from(&entity.gif_frame(self.tick), x as u32, y as u32)?;
        }

        let view = base.view(
            frame_size.x as u32,
            frame_size.y as u32,
            frame_size.width as u32,
            frame_size.height as u32 * 2, // Multiply by 2 because images are rendered in half the lines
        );

        Ok(view.to_image())
    }
}
