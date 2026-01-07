use super::utils::open_image;
use crate::types::AppResult;
use image::{Rgba, RgbaImage};
use std::cmp::min;
use std::collections::HashMap;
use strum::Display;

const FLOOR_COLOR: Rgba<u8> = Rgba([254, 229, 165, 255]);
const BLINKING_STEP: usize = 15;

pub fn floor_from_size(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_pixel(width, height, FLOOR_COLOR)
}

#[derive(Debug, Display, Default)]
pub enum PitchImage {
    #[default]
    PitchClassic,
    PitchBall,
    PitchPlanet,
    PitchFancy,
    HomeCloseShotMask,
    AwayCloseShotMask,
    HomeMediumShotMask,
    AwayMediumShotMask,
    HomeLongShotMask,
    AwayLongShotMask,
    HomeImpossibleShotMask,
    AwayImpossibleShotMask,
}

impl PitchImage {
    fn asset_filename(&self) -> &str {
        match self {
            PitchImage::PitchClassic => "game/pitch_classic.png",
            PitchImage::PitchBall => "game/pitch_ball.png",
            PitchImage::PitchPlanet => "game/pitch_planet.png",
            PitchImage::PitchFancy => "game/pitch_fancy.png",
            PitchImage::HomeCloseShotMask => "game/home_close_shot_mask.png",
            PitchImage::AwayCloseShotMask => "game/away_close_shot_mask.png",
            PitchImage::HomeMediumShotMask => "game/home_medium_shot_mask.png",
            PitchImage::AwayMediumShotMask => "game/away_medium_shot_mask.png",
            PitchImage::HomeLongShotMask => "game/home_long_shot_mask.png",
            PitchImage::AwayLongShotMask => "game/away_long_shot_mask.png",
            PitchImage::HomeImpossibleShotMask => "game/home_impossible_shot_mask.png",
            PitchImage::AwayImpossibleShotMask => "game/away_impossible_shot_mask.png",
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
                    if (tick / BLINKING_STEP).is_multiple_of(2) {
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

            img.put_pixel(x, y, pixel);
        }
        Ok(img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const PITCH_WIDTH: u32 = 75;
    const PITCH_HEIGHT: u32 = 41;

    #[test]
    #[ignore]
    fn test_generate_pitch_image() -> AppResult<()> {
        for pitch in vec![
            PitchImage::PitchClassic,
            PitchImage::PitchBall,
            PitchImage::PitchPlanet,
            PitchImage::PitchFancy,
        ] {
            let img = pitch.image()?;

            image::save_buffer(
                &Path::new(
                    format!("tests/{}_image.png", pitch.to_string().to_lowercase()).as_str(),
                ),
                &img,
                PITCH_WIDTH,
                PITCH_HEIGHT,
                image::ColorType::Rgba8,
            )?;
        }

        Ok(())
    }
}
