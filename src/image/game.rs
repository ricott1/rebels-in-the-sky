use super::utils::open_image;
use crate::types::AppResult;
use image::{Rgba, RgbaImage};
use std::cmp::min;
use std::collections::HashMap;

pub const FLOOR_COLOR: Rgba<u8> = Rgba([254, 229, 165, 255]);
pub const PITCH_WIDTH: u16 = 75;
pub const PITCH_HEIGHT: u16 = 41;
const BLINKING_STEP: usize = 15;

pub fn floor_from_size(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_pixel(width, height, FLOOR_COLOR)
}

#[derive(Debug, Default)]
pub enum PitchStyle {
    #[default]
    PitchClassic,
    PitchBall,
    HomeCloseShotMask,
    AwayCloseShotMask,
    HomeMediumShotMask,
    AwayMediumShotMask,
    HomeLongShotMask,
    AwayLongShotMask,
    HomeImpossibleShotMask,
    AwayImpossibleShotMask,
}

impl PitchStyle {
    fn asset_filename(&self) -> &str {
        match self {
            PitchStyle::PitchClassic => "game/pitch_classic.png",
            PitchStyle::PitchBall => "game/pitch_ball.png",
            PitchStyle::HomeCloseShotMask => "game/home_close_shot_mask.png",
            PitchStyle::AwayCloseShotMask => "game/away_close_shot_mask.png",
            PitchStyle::HomeMediumShotMask => "game/home_medium_shot_mask.png",
            PitchStyle::AwayMediumShotMask => "game/away_medium_shot_mask.png",
            PitchStyle::HomeLongShotMask => "game/home_long_shot_mask.png",
            PitchStyle::AwayLongShotMask => "game/away_long_shot_mask.png",
            PitchStyle::HomeImpossibleShotMask => "game/home_impossible_shot_mask.png",
            PitchStyle::AwayImpossibleShotMask => "game/away_impossible_shot_mask.png",
        }
    }

    pub fn image(&self) -> AppResult<RgbaImage> {
        open_image(self.asset_filename())
    }

    pub fn image_with_shot_pixels(
        &self,
        shots_map: HashMap<(u32, u32), (u8, u8)>,
        last_shot: Option<(u8, u8, bool)>,
        tick: usize,
    ) -> AppResult<RgbaImage> {
        let mut img = self.image()?;
        for (position, count) in shots_map.iter() {
            let x = position.0;
            let y = position.1;
            // Blink the indicator at last shot position by selectively not displaying the shot.
            let pixel = if let Some(shot) = last_shot {
                if x == shot.0 as u32 && y == shot.1 as u32 {
                    if (tick / BLINKING_STEP) % 2 == 0 {
                        continue;
                    }
                    // If last shot was made, add green pixel,
                    if shot.2 {
                        image::Rgba([0, 255, 0, 255])
                    }
                    // Else add a red pixel.
                    else {
                        image::Rgba([255, 0, 0, 255])
                    }
                } else {
                    image::Rgba([
                        (255.0 * count.0 as f32 / (count.0 as f32 + count.1 as f32)).round() as u8,
                        (255.0 * count.1 as f32 / (count.0 as f32 + count.1 as f32)).round() as u8,
                        0,
                        min(255, 100 + count.0 as u16 + count.1 as u16) as u8,
                    ])
                }
            } else {
                image::Rgba([
                    (255.0 * count.0 as f32 / (count.0 as f32 + count.1 as f32)).round() as u8,
                    (255.0 * count.1 as f32 / (count.0 as f32 + count.1 as f32)).round() as u8,
                    0,
                    min(255, 100 + count.0 as u16 + count.1 as u16) as u8,
                ])
            };

            img.put_pixel(x as u32, y as u32, pixel);
        }
        Ok(img)
    }
}
