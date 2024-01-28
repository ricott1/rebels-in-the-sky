use super::utils::read_image;
use crate::engine::types::GameStatsMap;
use crate::types::AppResult;
use image::{Rgba, RgbaImage};
use std::cmp::min;
use std::collections::HashMap;

pub const FLOOR_COLOR: Rgba<u8> = Rgba([254, 229, 165, 255]);
pub const PITCH_WIDTH: u16 = 75;
pub const PITCH_HEIGHT: u16 = 41;

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
            PitchStyle::PitchClassic => "pitch/pitch_classic.png",
            PitchStyle::PitchBall => "pitch/pitch_ball.png",
            PitchStyle::HomeCloseShotMask => "pitch/home_close_shot_mask.png",
            PitchStyle::AwayCloseShotMask => "pitch/away_close_shot_mask.png",
            PitchStyle::HomeMediumShotMask => "pitch/home_medium_shot_mask.png",
            PitchStyle::AwayMediumShotMask => "pitch/away_medium_shot_mask.png",
            PitchStyle::HomeLongShotMask => "pitch/home_long_shot_mask.png",
            PitchStyle::AwayLongShotMask => "pitch/away_long_shot_mask.png",
            PitchStyle::HomeImpossibleShotMask => "pitch/home_impossible_shot_mask.png",
            PitchStyle::AwayImpossibleShotMask => "pitch/away_impossible_shot_mask.png",
        }
    }

    pub fn image(&self) -> AppResult<RgbaImage> {
        read_image(self.asset_filename())
    }
}

pub fn set_shot_pixels(mut pitch_image: RgbaImage, stats_map: &GameStatsMap) -> RgbaImage {
    let mut shots_map: HashMap<(u32, u32), (u8, u8)> = HashMap::new();
    for stats in stats_map.values() {
        let shots = stats.shot_positions.clone();
        for shot in shots {
            let x = shot.0 as u32;
            let y = shot.1 as u32;
            if x < PITCH_WIDTH as u32 && y < PITCH_WIDTH as u32 {
                if let Some(count) = shots_map.get(&(x, y)) {
                    let new_count = if shot.2 {
                        (count.0, count.1 + 1)
                    } else {
                        (count.0 + 1, count.1)
                    };
                    shots_map.insert((x, y), new_count);
                } else {
                    let new_count = if shot.2 { (0, 1) } else { (1, 0) };
                    shots_map.insert((x, y), new_count);
                }
            }
        }
    }
    for (position, count) in shots_map.iter() {
        let x = position.0 as u32;
        let y = position.1 as u32;
        if x < PITCH_WIDTH as u32 && y < PITCH_WIDTH as u32 {
            pitch_image.put_pixel(
                x,
                y,
                image::Rgba([
                    (255.0 * count.0 as f32 / (count.0 as f32 + count.1 as f32)).round() as u8,
                    (255.0 * count.1 as f32 / (count.0 as f32 + count.1 as f32)).round() as u8,
                    0,
                    min(255, 100 + count.0 as u16 + count.1 as u16) as u8,
                ]),
            );
        }
    }
    pitch_image
}
