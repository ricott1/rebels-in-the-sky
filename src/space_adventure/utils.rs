use super::traits::HitBox;
use glam::{I16Vec2, Vec2};
use image::imageops::crop_imm;
use image::{buffer::ConvertBuffer, GrayImage, RgbaImage};
use imageproc::contours::{find_contours, BorderType};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub enum EntityState {
    Immortal,
    Decaying { lifetime: f32 },
}

pub enum Direction {}

impl Direction {
    pub const LEFT: Vec2 = Vec2::NEG_X;
    pub const RIGHT: Vec2 = Vec2::X;
    pub const UP: Vec2 = Vec2::NEG_Y;
    pub const DOWN: Vec2 = Vec2::Y;
}

pub fn body_data_from_image(image: &RgbaImage) -> (RgbaImage, HitBox) {
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
        .min_by(|pa, pb| pa.cmp(&pb))
        .unwrap_or_default();

    let max_x = contours_vec
        .iter()
        .map(|p| p.x)
        .max_by(|pa, pb| pa.cmp(&pb))
        .unwrap_or_default();

    let min_y = contours_vec
        .iter()
        .map(|p| p.y)
        .min_by(|pa, pb| pa.cmp(&pb))
        .unwrap_or_default();

    let max_y = contours_vec
        .iter()
        .map(|p| p.y)
        .max_by(|pa, pb| pa.cmp(&pb))
        .unwrap_or_default();

    // Crop image to minimum rect.
    let cropped_image = crop_imm(
        image,
        min_x as u32,
        min_y as u32,
        (max_x - min_x) as u32 + 1,
        (max_y - min_y) as u32 + 1,
    )
    .to_image();

    // Translate contours.
    let contour = contours_vec
        .iter()
        .map(|&point| I16Vec2::new(point.x - min_x, point.y - min_y))
        .collect::<HashSet<_>>();

    let mut hit_box = HashMap::new();

    for x in 0..cropped_image.width() {
        for y in 0..cropped_image.height() {
            if let Some(pixel) = cropped_image.get_pixel_checked(x, y) {
                // If pixel is non-transparent.
                if pixel[3] > 0 {
                    let point = I16Vec2::new(x as i16, y as i16);
                    let is_border = contour.contains(&point);
                    hit_box.insert(point, is_border);
                }
            }
        }
    }

    (cropped_image, hit_box.into())
}
