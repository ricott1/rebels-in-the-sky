use image::{buffer::ConvertBuffer, GrayImage, RgbaImage};
use imageproc::contours::{find_contours, BorderType};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
};

use super::space_callback::SpaceCallback;

pub type Vector2D = [i16; 2]; // u16 should be plenty

pub trait Body {
    fn is_collider(&self) -> bool {
        false
    }
    fn is_dynamical(&self) -> bool {
        false
    }
    fn bounds(&self) -> (Vector2D, Vector2D);
    fn position(&self) -> Vector2D {
        [0, 0]
    }

    fn velocity(&self) -> Vector2D {
        [0, 0]
    }

    fn acceleration(&self) -> Vector2D {
        [0, 0]
    }
    fn push_left(&mut self) {}
    fn push_right(&mut self) {}
    fn push_up(&mut self) {}
    fn push_down(&mut self) {}
    fn collides_with(&self, other: Box<dyn Body>) -> bool {
        if !self.is_collider() || !other.is_collider() {
            return false;
        }

        // Broad phase detection
        let ([min_x, min_y], [max_x, max_y]) = self.bounds();
        let ([o_min_x, o_min_y], [o_max_x, o_max_y]) = other.bounds();

        // Shortcut if rects cannot intersect
        if min_x > o_max_x || o_min_x > max_x || min_y > o_max_y || o_min_y > max_y {
            return false;
        }

        // Granular phase detection

        true
    }
    fn update_body(&mut self, _: f64) -> Vec<SpaceCallback> {
        vec![]
    }
}

pub trait Sprite {
    fn image(&self) -> &RgbaImage;

    fn layer(&self) -> usize {
        0
    }

    fn mask(&self) -> HashSet<(i16, i16)> {
        let contours = {
            let gray_img = ConvertBuffer::<GrayImage>::convert(self.image());
            find_contours::<i16>(&gray_img)
        };

        let mut mask = HashSet::new();
        for contour in contours.iter() {
            if contour.border_type == BorderType::Outer {
                for &point in contour.points.iter() {
                    mask.insert((point.x, point.y));
                }
            }
        }
        mask
    }

    fn hit_box(&self) -> HashMap<(i16, i16), bool> {
        self.mask()
            .iter()
            .map(|&point| (point, true))
            .collect::<HashMap<_, _>>()
    }

    fn update_sprite(&mut self, _: f64) -> Vec<SpaceCallback> {
        vec![]
    }
}

pub trait Entity: Body + Sprite + Debug + Send + Sync {
    fn set_id(&mut self, id: usize);
    fn id(&self) -> usize;
    fn update(&mut self, deltatime: f64) -> Vec<SpaceCallback> {
        let mut callbacks = vec![];
        callbacks.append(&mut self.update_body(deltatime));
        callbacks.append(&mut self.update_sprite(deltatime));

        callbacks
    }
}
