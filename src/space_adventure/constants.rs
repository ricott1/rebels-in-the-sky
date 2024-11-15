use glam::UVec2;

use crate::ui::UI_SCREEN_SIZE;

pub(crate) const FRICTION_COEFF: f32 = 0.1;
pub(crate) const THRUST_MOD: f32 = 1.5;
pub(crate) const FUEL_CONSUMPTION_MOD: f32 = 215_000.0;
pub(crate) const MAX_SPACESHIP_SPEED_MOD: f32 = 0.135;

pub(crate) const ASTEROID_GENERATION_PROBABILITY: f64 = 0.05;
pub(crate) const DIFFICULTY_FOR_ASTEROID_PLANET_GENERATION: usize = 60;

// There are 3 relevant lengths for the space image:
//   1. the "screen size", which is the size of the cropped space image before rendering on the screen;
//   2. the "entity position size", which indicates where entities can be on the space image. It has an extra buffer
//      around the screen size in every four direction so that entities can smoothly 'appear' on screen from every direction;
//   3. the "background total size", which must accomodate drawing enitities at any possible position,
//      and hence must have an extra buffer around the max position size (2.) bottom and right.
pub(crate) const SCREEN_SIZE: UVec2 =
    UVec2::new(UI_SCREEN_SIZE.0 as u32, UI_SCREEN_SIZE.1 as u32 * 2 - 8);
pub(crate) const MAX_ENTITY_POSITION: UVec2 = UVec2::new(200, 128);
pub(crate) const BACKGROUND_IMAGE_SIZE: UVec2 = UVec2::new(240, 168);

pub(crate) const MAX_LAYER: usize = 5;

pub(crate) const MAX_ASTEROID_PLANET_IMAGE_TYPE: usize = 30;
