use std::hash::Hash;

use super::color_map::{ColorMap, ColorPreset};
use super::components::*;
use super::types::Gif;
use super::utils::{open_image, ExtraImageUtils};
use crate::types::AppResult;
use crate::world::spaceship::{Engine, Hull, Shooter, SpaceshipComponent, Storage};
use image::{Rgba, RgbaImage};
use serde;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SPACESHIP_IMAGE_WIDTH: u32 = 30;
pub const SPACESHIP_IMAGE_HEIGHT: u32 = 24;

pub type SpaceshipImageId = Vec<u8>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Hash, Default)]
pub struct SpaceshipImage {
    pub color_map: ColorMap,
}

impl SpaceshipImage {
    pub fn new(color_map: ColorMap) -> Self {
        Self { color_map }
    }

    pub fn id(&self, hull: Hull, engine: Engine, storage: Storage) -> SpaceshipImageId {
        let mut hasher = Sha256::new();

        hasher.update(format!(
            "{}{}{}{}",
            hull,
            engine,
            storage,
            self.color_map.hex_format()
        ));

        hasher.finalize().to_vec()
    }

    pub fn set_color_map(&mut self, color_map: ColorMap) {
        self.color_map = color_map;
    }

    pub fn size(hull: &Hull) -> u8 {
        match hull {
            Hull::ShuttleSmall => 0,
            Hull::ShuttleStandard => 1,
            Hull::ShuttleLarge => 2,
            Hull::PincherStandard => 1,
            Hull::PincherLarge => 2,
            Hull::JesterStandard => 1,
        }
    }

    pub fn compose(
        &self,
        hull: Hull,
        engine: Engine,
        storage: Storage,
        shooter: Shooter,
        in_shipyard: bool,
        shooting: bool,
    ) -> AppResult<Gif> {
        let mut gif = Gif::new();
        let size = Self::size(&hull);

        let mut hull_img = hull.image()?;
        let mask = hull.mask()?;
        hull_img.apply_color_map_with_shadow_mask(self.color_map, &mask);
        let hull_x = (SPACESHIP_IMAGE_WIDTH - hull_img.width()) / 2;
        let hull_y = (SPACESHIP_IMAGE_HEIGHT - hull_img.height()) / 2;

        let engine_color_presets: Vec<[ColorPreset; 3]> = vec![
            [ColorPreset::Red, ColorPreset::Red, ColorPreset::Orange],
            [ColorPreset::Red, ColorPreset::Orange, ColorPreset::Yellow],
            [
                ColorPreset::Red,
                ColorPreset::Yellow,
                ColorPreset::SandyBrown,
            ],
            [ColorPreset::Orange, ColorPreset::Yellow, ColorPreset::Red],
            [
                ColorPreset::Red,
                ColorPreset::SandyBrown,
                ColorPreset::Yellow,
            ],
            [
                ColorPreset::Red,
                ColorPreset::SandyBrown,
                ColorPreset::Yellow,
            ],
            [ColorPreset::Red, ColorPreset::Orange, ColorPreset::Yellow],
            [
                ColorPreset::Red,
                ColorPreset::SandyBrown,
                ColorPreset::Orange,
            ],
            [ColorPreset::Orange, ColorPreset::Red, ColorPreset::Orange],
        ];

        let max_tick = if in_shipyard { 1 } else { 72 };
        for tick in 0..max_tick {
            let color_presets = &engine_color_presets[(tick / 4) % engine_color_presets.len()];
            let color_map = ColorMap {
                red: color_presets[0].to_rgb(),
                green: color_presets[1].to_rgb(),
                blue: color_presets[2].to_rgb(),
            };

            let mut engine = engine.image()?;
            let eng_x = (SPACESHIP_IMAGE_WIDTH - engine.width()) / 2;
            let eng_y = 0;
            engine.apply_color_map(color_map);

            let mut base = RgbaImage::new(SPACESHIP_IMAGE_WIDTH, SPACESHIP_IMAGE_HEIGHT);
            base.copy_non_trasparent_from(&engine, eng_x, eng_y)?;

            let mut storage_img = storage.image(size)?;
            let mask = storage.mask(size)?;
            storage_img.apply_color_map_with_shadow_mask(self.color_map, &mask);
            let stg_x = (SPACESHIP_IMAGE_WIDTH - storage_img.width()) / 2;
            let stg_y = (SPACESHIP_IMAGE_HEIGHT - storage_img.height()) / 2;
            storage_img.apply_color_map(self.color_map);
            base.copy_non_trasparent_from(&storage_img, stg_x, stg_y)?;
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

            if shooting {
                let shooter_img = shooter.image(size)?;
                let mut shooter_positions = vec![];
                for x in 0..shooter_img.width() {
                    for y in 0..shooter_img.height() {
                        if let Some(pixel) = shooter_img.get_pixel_checked(x, y) {
                            // If pixel is blue, it is at the shooter position.
                            if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 255 && pixel[3] > 0 {
                                shooter_positions.push((x, y));
                            }
                        }
                    }
                }

                let x_offset = (base.width() - shooter_img.width()) / 2;
                let y_offset = (tick as u32 / 2) % (36 / shooter.fire_rate() as u32) + 1;
                // Projectiles last for 4 ticks and are generated depending on the shooter firerate.
                for (x, y) in shooter_positions.iter() {
                    if *y >= y_offset {
                        base.put_pixel(*x + x_offset, *y - y_offset, Rgba([0, 0, 255, 255]));
                    }
                }
            }

            gif.push(base);
        }
        Ok(gif)
    }
}
