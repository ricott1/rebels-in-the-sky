use super::collisions::HitBox;
use super::entity::Entity;
use super::traits::*;
use glam::{I16Vec2, Vec2};
use image::imageops::crop_imm;
use image::Rgba;
use image::{buffer::ConvertBuffer, GrayImage, RgbaImage};
use imageproc::contours::{find_contours, BorderType};
use std::collections::{HashMap, HashSet};

pub type EntityMap = HashMap<usize, Entity>;

#[derive(Debug, Clone, Copy)]
pub enum EntityState {
    Immortal,
    Decaying { lifetime: f32 },
}

pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub fn as_vec2(&self) -> Vec2 {
        match self {
            Self::Left => Vec2::NEG_X,
            Self::Right => Vec2::X,
            Self::Up => Vec2::NEG_Y,
            Self::Down => Vec2::Y,
        }
    }

    pub fn as_i16vec2(&self) -> I16Vec2 {
        match self {
            Self::Left => I16Vec2::NEG_X,
            Self::Right => I16Vec2::X,
            Self::Up => I16Vec2::NEG_Y,
            Self::Down => I16Vec2::Y,
        }
    }
}

pub fn body_data_from_image(image: &RgbaImage, should_crop: bool) -> (RgbaImage, HitBox) {
    let gray_img = ConvertBuffer::<GrayImage>::convert(image);
    // Find contours to get minimum rect enclosing image.
    let mut contours_vec = vec![];
    for contour in find_contours::<i16>(&gray_img).iter() {
        if contour.border_type == BorderType::Outer {
            for &point in contour.points.iter() {
                contours_vec.push(point);
            }
        }
    }

    let min_x = contours_vec
        .iter()
        .map(|p| p.x)
        .min_by(|pa, pb| pa.cmp(pb))
        .unwrap_or_default();

    let max_x = contours_vec
        .iter()
        .map(|p| p.x)
        .max_by(|pa, pb| pa.cmp(pb))
        .unwrap_or_default();

    let min_y = contours_vec
        .iter()
        .map(|p| p.y)
        .min_by(|pa, pb| pa.cmp(pb))
        .unwrap_or_default();

    let max_y = contours_vec
        .iter()
        .map(|p| p.y)
        .max_by(|pa, pb| pa.cmp(pb))
        .unwrap_or_default();

    let final_image = if should_crop {
        // Crop image to minimum rect.
        crop_imm(
            image,
            min_x as u32,
            min_y as u32,
            (max_x - min_x) as u32 + 1,
            (max_y - min_y) as u32 + 1,
        )
        .to_image()
    } else {
        image.clone()
    };

    // Translate contours.
    let contour = contours_vec
        .iter()
        .map(|&point| I16Vec2::new(point.x - min_x, point.y - min_y))
        .collect::<HashSet<_>>();

    let mut hit_box = HashMap::new();

    for x in 0..final_image.width() {
        for y in 0..final_image.height() {
            if let Some(pixel) = final_image.get_pixel_checked(x, y) {
                // If pixel is non-transparent.
                if pixel[3] > 0 {
                    let point = I16Vec2::new(x as i16, y as i16);
                    let is_border = contour.contains(&point);
                    hit_box.insert(point, is_border);
                }
            }
        }
    }

    let hit_box: HitBox = hit_box.into();
    log::debug!("Created hitbox with size {:#?}", hit_box.size());
    (final_image, hit_box)
}

pub fn draw_hitbox(base: &mut RgbaImage, entity: &Entity) {
    let gray = Rgba([105, 105, 105, 255]);

    let bw = base.width() as i32;
    let bh = base.height() as i32;

    for (point, &is_border) in entity.hit_box().iter() {
        if !is_border {
            continue;
        }
        let g = entity.position() + point;
        let x = g.x as i32;
        let y = g.y as i32;

        if (0..bw).contains(&x) && (0..bh).contains(&y) {
            base.put_pixel(x as u32, y as u32, gray);
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{core::spaceship::SpaceshipPrefab, types::AppResult};
    use glam::I16Vec2;
    use image::imageops::rotate90;

    use super::body_data_from_image;

    #[test]
    fn test_hitbox_size() -> AppResult<()> {
        let spaceship = SpaceshipPrefab::Ibarruri.spaceship();
        let base_gif = spaceship.compose_image(None)?;
        let base_image = rotate90(&base_gif[0]);
        let (_, hit_box) = body_data_from_image(&base_image, false);
        assert!(hit_box.size() == I16Vec2::new(16, 20));

        Ok(())
    }
}
