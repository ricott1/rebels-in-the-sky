use super::{asteroid::AsteroidSize, constants::MAX_LAYER, visual_effects::VisualEffect};
use crate::{
    core::{Engine, Hull, Storage},
    image::color_map::ColorMap,
    space_adventure::{utils::EntityMap, Body, Sprite},
};
use image::Rgba;

#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum ImageType {
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
        color: Rgba<u8>,
    },
    Particle {
        color: Rgba<u8>,
    },
    Projectile {
        color: Rgba<u8>,
    },
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct NetworkSpaceData {
    new_entities: Vec<(usize, ImageType)>,      // [(id, image)]
    state: Vec<(usize, u8, u8)>,                // (id, x, y)
    visual_effects: Vec<(usize, VisualEffect)>, // [(id, visual_effect)]
}

#[allow(unused)]
impl NetworkSpaceData {
    fn insert_entity(&mut self, id: usize, image_type: ImageType) {
        self.new_entities.push((id, image_type));
    }

    fn update_state(&mut self, entities: &[EntityMap; MAX_LAYER]) {
        let mut state = vec![];
        for layer_entities in entities.iter().take(MAX_LAYER) {
            for (id, entity) in layer_entities.iter() {
                let [x, y] = entity.position().to_array();
                state.push((id, x as u8, y as u8));
            }
        }
    }

    fn reset(&mut self) {
        self.new_entities = vec![];
        self.state = vec![];
        self.visual_effects = vec![];
    }
}

#[allow(unused)]
trait NetworkSprite: Sprite {
    fn network_image_type(&self) -> ImageType;
}
