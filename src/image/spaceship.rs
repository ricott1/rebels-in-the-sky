use super::color_map::{ColorMap, ColorPreset};
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
        let mut gif = Gif::new();

        let mut hull_img = read_image(hull.select_file(0).as_str())?;
        let mask = read_image(hull.select_mask_file(0).as_str())?;
        hull_img.apply_color_map_with_shadow_mask(self.color_map, &mask);
        let hull_x = (SPACESHIP_IMAGE_WIDTH - hull_img.width()) / 2;
        let hull_y = (SPACESHIP_IMAGE_HEIGHT - hull_img.height()) / 2;

        let engine_color_presets: Vec<[ColorPreset; 3]> = vec![
            [ColorPreset::Red, ColorPreset::Red, ColorPreset::Orange],
            [ColorPreset::Red, ColorPreset::Orange, ColorPreset::Yellow],
            [ColorPreset::Red, ColorPreset::Yellow, ColorPreset::Gold],
            [ColorPreset::Orange, ColorPreset::Yellow, ColorPreset::Red],
            [ColorPreset::Red, ColorPreset::Gold, ColorPreset::Yellow],
            [ColorPreset::Red, ColorPreset::Gold, ColorPreset::Yellow],
            [ColorPreset::Red, ColorPreset::Gold, ColorPreset::Red],
            [ColorPreset::Orange, ColorPreset::Red, ColorPreset::Orange],
        ];

        for tick in 0..32 {
            let color_presets = engine_color_presets[tick / 4].clone();
            let color_map = ColorMap {
                red: color_presets[0].to_rgb(),
                green: color_presets[1].to_rgb(),
                blue: color_presets[2].to_rgb(),
            };

            // let color_map = ColorMap::random();
            let mut engine = read_image(engine.select_file(0).as_str())?;
            let eng_x = (SPACESHIP_IMAGE_WIDTH - engine.width()) / 2;
            let eng_y = 20;
            engine.apply_color_map(color_map);
            let mut base = RgbaImage::new(SPACESHIP_IMAGE_WIDTH, SPACESHIP_IMAGE_HEIGHT);
            base.copy_non_trasparent_from(&engine, eng_x, eng_y)?;
            base.copy_non_trasparent_from(&hull_img, hull_x, hull_y)?;
            gif.push(base);
        }
        Ok(gif)
    }
}
