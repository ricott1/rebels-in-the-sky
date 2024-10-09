use super::color_map::{ColorMap, ColorPreset};
use super::components::*;
use super::types::Gif;
use super::utils::{open_image, ExtraImageUtils};
use crate::types::AppResult;
use crate::world::spaceship::{Engine, Hull, SpaceshipComponent, Storage};
use image::RgbaImage;
use serde;
use serde::{Deserialize, Serialize};

pub const SPACESHIP_IMAGE_WIDTH: u32 = 30;
pub const SPACESHIP_IMAGE_HEIGHT: u32 = 24;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Hash, Default)]
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

    pub fn compose(
        &self,
        size: u8,
        hull: Hull,
        engine: Engine,
        storage: Storage,
        in_shipyard: bool,
    ) -> AppResult<Gif> {
        let mut gif = Gif::new();

        let mut hull_img = open_image(hull.select_file(size).as_str())?;
        let mask = open_image(hull.select_mask_file(size).as_str())?;
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

        let max_tick = if in_shipyard { 1 } else { 32 };
        for tick in 0..max_tick {
            let color_presets = engine_color_presets[tick / 4].clone();
            let color_map = ColorMap {
                red: color_presets[0].to_rgb(),
                green: color_presets[1].to_rgb(),
                blue: color_presets[2].to_rgb(),
            };

            let mut engine = open_image(engine.select_file(size).as_str())?;
            let eng_x = (SPACESHIP_IMAGE_WIDTH - engine.width()) / 2;
            let eng_y = 0;
            engine.apply_color_map(color_map);

            let mut base = RgbaImage::new(SPACESHIP_IMAGE_WIDTH, SPACESHIP_IMAGE_HEIGHT);
            base.copy_non_trasparent_from(&engine, eng_x, eng_y)?;

            match storage {
                Storage::PincherNone | Storage::ShuttleNone | Storage::JesterNone => {}
                _ => {
                    let mut storage_img = open_image(storage.select_file(size).as_str())?;
                    let mask = open_image(storage.select_mask_file(size).as_str())?;
                    storage_img.apply_color_map_with_shadow_mask(self.color_map, &mask);
                    let stg_x = (SPACESHIP_IMAGE_WIDTH - storage_img.width()) / 2;
                    let stg_y = (SPACESHIP_IMAGE_HEIGHT - storage_img.height()) / 2;
                    storage_img.apply_color_map(self.color_map);
                    base.copy_non_trasparent_from(&storage_img, stg_x, stg_y)?;
                }
            }
            base.copy_non_trasparent_from(&hull_img, hull_x, hull_y)?;

            if in_shipyard {
                let shipyard_img = open_image(
                    format!(
                        "hull/shipyard_{}.png",
                        hull.style().to_string().to_lowercase()
                    )
                    .as_str(),
                )?;

                let x = (SPACESHIP_IMAGE_WIDTH - shipyard_img.width()) / 2;
                let y = 0;

                base.copy_non_trasparent_from(&shipyard_img, x, y)?;
            }

            gif.push(base);
        }
        Ok(gif)
    }
}
