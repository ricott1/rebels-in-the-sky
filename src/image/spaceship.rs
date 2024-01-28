use super::color_map::ColorMap;
use super::components::*;
use super::types::Gif;
use super::utils::{read_image, ExtraImageUtils};
use crate::types::AppResult;
use crate::world::spaceship::{Engine, Hull};
use image::RgbaImage;
use serde;
use serde::{Deserialize, Serialize};

pub const SPACESHIP_IMAGE_WIDTH: u32 = 30;
pub const SPACESHIP_IMAGE_HEIGHT: u32 = 24;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub struct SpaceshipImage {
    color_map: ColorMap,
}

impl SpaceshipImage {
    pub fn new(color_map: ColorMap) -> Self {
        Self { color_map }
    }

    pub fn set_color_map(&mut self, color_map: ColorMap) {
        self.color_map = color_map;
    }

    pub fn compose(&self, hull: Hull, engine: Engine) -> AppResult<Gif> {
        let mut base = RgbaImage::new(SPACESHIP_IMAGE_WIDTH, SPACESHIP_IMAGE_HEIGHT);
        let mut gif = Gif::new();

        let mut hull = read_image(hull.select_file(0).as_str())?;
        hull.apply_color_map(self.color_map);
        let hull_x = (base.width() - hull.width()) / 2;
        let hull_y = (base.height() - hull.height()) / 2;

        let engine = read_image(engine.select_file(0).as_str())?;
        let eng_x = (base.width() - engine.width()) / 2;
        let eng_y = 20;

        base.copy_non_trasparent_from(&engine, eng_x, eng_y)?;
        base.copy_non_trasparent_from(&hull, hull_x, hull_y)?;

        for _ in 0..16 {
            gif.push(base.clone());
        }
        Ok(gif)
    }
}
