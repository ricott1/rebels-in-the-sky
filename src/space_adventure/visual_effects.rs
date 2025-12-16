use crate::space_adventure::GameEntity;
use image::RgbaImage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VisualEffect {
    FadeIn,
    FadeOut,
    ColorMask { color: [u8; 3] },
}

impl VisualEffect {
    pub const COLOR_MASK_LIFETIME: f32 = 2.0;
    pub const FADE_IN_LIFETIME: f32 = 2.0;
    pub const FADE_OUT_LIFETIME: f32 = 2.0;
    pub fn apply<T>(&self, entity: &T, img: &mut RgbaImage, time: f32)
    where
        T: GameEntity,
    {
        match &self {
            VisualEffect::FadeIn => {
                VisualEffect::FadeIn.apply_global_effect(img, time);
            }

            VisualEffect::FadeOut => {
                VisualEffect::FadeOut.apply_global_effect(img, time);
            }

            VisualEffect::ColorMask { color } => {
                for (point, &is_border) in entity.hit_box().iter() {
                    if is_border {
                        let mut pixel = *img.get_pixel(point.x as u32, point.y as u32);

                        for (idx, &v) in color.iter().enumerate() {
                            if v > 0 {
                                pixel.0[idx] = ((1.0 - time / Self::COLOR_MASK_LIFETIME)
                                    * pixel.0[idx] as f32
                                    + time / Self::COLOR_MASK_LIFETIME * color[idx] as f32)
                                    as u8;
                            }
                        }

                        img.put_pixel(point.x as u32, point.y as u32, pixel);
                    }
                }
            }
        }
    }

    pub fn apply_global_effect(&self, img: &mut RgbaImage, time: f32) {
        match &self {
            VisualEffect::FadeIn => {
                let modifier = (time / Self::FADE_IN_LIFETIME).clamp(0.0, 1.0);
                for x in 0..img.width() {
                    for y in 0..img.height() {
                        let mut pixel = *img.get_pixel(x, y);

                        for idx in 0..4 {
                            pixel.0[idx] = (modifier * pixel.0[idx] as f32) as u8;
                        }
                        img.put_pixel(x, y, pixel);
                    }
                }
            }

            VisualEffect::FadeOut => {
                let modifier = (1.0 - time / Self::FADE_IN_LIFETIME).clamp(0.0, 1.0);

                for x in 0..img.width() {
                    for y in 0..img.height() {
                        let mut pixel = *img.get_pixel(x, y);
                        for idx in 0..4 {
                            pixel.0[idx] = (modifier * pixel.0[idx] as f32) as u8;
                        }
                        img.put_pixel(x, y, pixel);
                    }
                }
            }
            _ => {}
        }
    }
}
