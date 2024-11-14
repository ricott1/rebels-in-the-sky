use super::{asteroid::AsteroidSize, constants::MAX_LAYER, visual_effects::VisualEffect, Entity};
use crate::{
    image::color_map::ColorMap,
    world::spaceship::{Engine, Hull, Storage},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ImageType {
    None,
    Asteroid {
        size: AsteroidSize,
        image_type: usize,
    },
    Spaceship {
        hull: Hull,
        engine: Engine,
        storage: Storage,
        color_map: ColorMap,
    },
    Fragment {
        color: [u8; 3],
    },
    Particle {
        color: [u8; 3],
    },
    Projectile {
        color: [u8; 3],
    },
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkSpaceData {
    new_entities: Vec<(usize, ImageType)>,      // [(id, image)]
    state: Vec<(usize, u8, u8)>,                // (id, x, y)
    visual_effects: Vec<(usize, VisualEffect)>, // [(id, visual_effect)]
}

#[allow(unused)]
impl NetworkSpaceData {
    pub fn insert_entity(&mut self, id: usize, image_type: ImageType) {
        self.new_entities.push((id, image_type));
    }

    pub fn update_state(&mut self, entities: &[HashMap<usize, Box<dyn Entity>>; MAX_LAYER]) {
        let mut state = vec![];
        // let mut visual_effects = vec![];
        for layer in 0..MAX_LAYER {
            for (id, entity) in entities[layer].iter() {
                let [x, y] = entity.position().to_array();
                state.push((id, x as u8, y as u8));
            }
        }
    }

    pub fn reset(&mut self) {
        self.new_entities = vec![];
        self.state = vec![];
        self.visual_effects = vec![];
    }
}
