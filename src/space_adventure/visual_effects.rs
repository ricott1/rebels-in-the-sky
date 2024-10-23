use image::{Rgba, RgbaImage};

use super::Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisualEffect {
    FadeIn,
    FadeOut,
    ColorMask { color: Rgba<u8> },
}

impl VisualEffect {
    pub const COLOR_MASK_LIFETIME: f32 = 2.0;
    pub const FADE_IN_LIFETIME: f32 = 2.0;
    pub const FADE_OUT_LIFETIME: f32 = 2.0;
    pub fn apply<T>(&self, entity: &T, img: &mut RgbaImage, time: f32)
    where
        T: Entity,
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
                        let mut pixel = img.get_pixel(point.x as u32, point.y as u32).clone();

                        for idx in 0..4 {
                            if color[idx] > 0 {
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
                for x in 0..img.width() {
                    for y in 0..img.height() {
                        let mut pixel = img.get_pixel(x, y).clone();
                        for idx in 0..4 {
                            pixel.0[idx] = (time / Self::FADE_IN_LIFETIME * pixel.0[idx] as f32)
                                .min(255.0)
                                .max(0.0) as u8;
                        }
                        img.put_pixel(x, y, pixel);
                    }
                }
            }

            VisualEffect::FadeOut => {
                for x in 0..img.width() {
                    for y in 0..img.height() {
                        let mut pixel = img.get_pixel(x, y).clone();
                        for idx in 0..4 {
                            pixel.0[idx] = ((1.0 - time / Self::FADE_OUT_LIFETIME)
                                * pixel.0[idx] as f32)
                                .min(255.0)
                                .max(0.0) as u8;
                        }
                        img.put_pixel(x, y, pixel);
                    }
                }
            }
            _ => {}
        }
    }
}
