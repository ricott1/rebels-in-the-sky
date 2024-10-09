use super::space_callback::SpaceCallback;
use super::{constants::*, traits::*};
use crate::image::types::Gif;
use crate::image::utils::open_image;
use image::imageops::crop_imm;
use image::{buffer::ConvertBuffer, GrayImage, Rgba, RgbaImage};
use imageproc::{
    contours::{find_contours, BorderType},
    geometric_transformations::{rotate_about_center, Interpolation},
};
use once_cell::sync::Lazy;
use rand::seq::IteratorRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::{HashMap, HashSet};
use strum::{Display, EnumIter, IntoEnumIterator};

const MAX_ASTEROID_TYPE_INDEX: usize = 3;

// Calculate astroid gifs, hit boxes, and contours once to be more efficient.
static ASTEROID_IMAGE_DATA: Lazy<
    HashMap<
        (AsteroidSize, usize),
        (
            Gif,
            Vec<HashMap<(i16, i16), bool>>,
            Vec<HashSet<(i16, i16)>>,
        ),
    >,
> = Lazy::new(|| {
    fn asteroid_data(
        size: AsteroidSize,
        n_idx: usize,
        rotation_idx: usize,
    ) -> (RgbaImage, HashMap<(i16, i16), bool>, HashSet<(i16, i16)>) {
        let path = format!(
            "space_adventure/asteroid_{}{}.png",
            size.to_string().to_ascii_lowercase(),
            n_idx
        );
        let base_img = open_image(&path).expect("Should open asteroid image");

        let mut image = rotate_about_center(
            &base_img,
            std::f32::consts::PI / 8.0 * rotation_idx as f32,
            Interpolation::Nearest,
            Rgba([0, 0, 0, 0]),
        );

        let gray_img = ConvertBuffer::<GrayImage>::convert(&image);
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
        image = crop_imm(
            &image,
            min_x as u32,
            min_y as u32,
            (max_x - min_x) as u32 + 1,
            (max_y - min_y) as u32 + 1,
        )
        .to_image();

        // Translate contours.
        let contour = contours_vec
            .iter()
            .map(|&point| (point.x - min_x, point.y - min_y))
            .collect::<HashSet<_>>();

        let mut hit_box = HashMap::new();

        for x in 0..image.width() {
            for y in 0..image.height() {
                if let Some(pixel) = image.get_pixel_checked(x, y) {
                    // If pixel is non-transparent.
                    if pixel[3] > 0 {
                        let is_border = contour.contains(&(x as i16, y as i16));
                        hit_box.insert((x as i16, y as i16), is_border);
                    }
                }
            }
        }

        (image, hit_box, contour)
    }

    let mut data = HashMap::new();

    for size in AsteroidSize::iter() {
        for n_idx in 1..=MAX_ASTEROID_TYPE_INDEX {
            let mut gif = vec![];
            let mut hit_boxes = vec![];
            let mut contours = vec![];
            for rotation_idx in 0..8 {
                let (image, hit_box, contour) = asteroid_data(size, n_idx, rotation_idx);
                gif.push(image);
                hit_boxes.push(hit_box);
                contours.push(contour);
            }
            data.insert((size, n_idx), (gif, hit_boxes, contours));
        }
    }

    data
});

#[derive(Default, Debug, Display, EnumIter, PartialEq, Eq, Clone, Copy, Hash)]
pub enum AsteroidSize {
    #[default]
    Big,
    Small,
    Fragment,
}

#[derive(Default, Debug, Clone)]
pub struct AsteroidEntity {
    id: usize,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rotation_speed: f64,
    pub size: AsteroidSize,
    // Vector of contours set since we need a different contour for each frame in the gif.
    contours: Vec<HashSet<(i16, i16)>>,
    // Vector of hit_box maps since we need a different hit box for each frame in the gif.
    // Maps with hit box points, Point -> is_border.
    hit_boxes: Vec<HashMap<(i16, i16), bool>>,
    tick: usize,
    gif: Gif,
}

impl Body for AsteroidEntity {
    fn is_collider(&self) -> bool {
        true
    }
    fn bounds(&self) -> (Vector2D, Vector2D) {
        let img = self.image();
        (
            [self.x as i16, self.y as i16],
            [
                self.x as i16 + img.width() as i16,
                self.y as i16 + img.height() as i16,
            ],
        )
    }

    fn position(&self) -> Vector2D {
        [self.x as i16, self.y as i16]
    }

    fn velocity(&self) -> Vector2D {
        [self.vx as i16, self.vy as i16]
    }

    fn acceleration(&self) -> Vector2D {
        [0, 0]
    }

    fn update_body(&mut self, deltatime: f64) -> Vec<SpaceCallback> {
        self.tick += 1;

        // Get current parameters
        let [x, y] = [self.x, self.y];
        let [vx, vy] = [self.vx, self.vy];

        let callbacks = vec![];

        let [mut nx, mut ny] = [(x + vx * deltatime), (y + vy * deltatime)];

        // FIXME: When completely out of bounds, destroy it.
        let bounds = self.bounds();
        if nx < 0.0 {
            nx = 0.0;
        } else if bounds.1[0] > SCREEN_WIDTH as i16 {
            nx = (SCREEN_WIDTH - self.image().width() as u16) as f64;
        }
        if ny < 0.0 {
            ny = 0.0;
        } else if bounds.1[1] > SCREEN_HEIGHT as i16 {
            ny = (SCREEN_HEIGHT - self.image().height() as u16) as f64;
        }

        // Update parameters
        [self.x, self.y] = [nx, ny];

        callbacks
    }
}

impl Sprite for AsteroidEntity {
    fn layer(&self) -> usize {
        match self.size {
            AsteroidSize::Fragment => {
                let rng = &mut rand::thread_rng();
                rng.gen_range(0..=2)
            }
            _ => 1,
        }
    }

    fn image(&self) -> &RgbaImage {
        &self.gif[self.frame()]
    }

    fn update_sprite(&mut self, _: f64) -> Vec<SpaceCallback> {
        self.tick += 1;
        vec![]
    }

    fn mask(&self) -> HashSet<(i16, i16)> {
        self.contours[self.frame()]
            .iter()
            .map(|&p| (p.0 + self.x as i16, p.1 + self.y as i16))
            .collect::<HashSet<_>>()
    }

    fn hit_box(&self) -> HashMap<(i16, i16), bool> {
        self.hit_boxes[self.frame()]
            .iter()
            .map(|(&p, &is_border)| ((p.0 + self.x as i16, p.1 + self.y as i16), is_border))
            .collect::<HashMap<_, _>>()
    }
}

impl Entity for AsteroidEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }
    fn id(&self) -> usize {
        self.id
    }
}

impl AsteroidEntity {
    fn frame(&self) -> usize {
        (self.tick as f64 * self.rotation_speed) as usize % self.gif.len()
    }

    pub fn new(x: f64, y: f64, vx: f64, vy: f64, size: AsteroidSize) -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();

        let n_idx = rng.gen_range(1..=MAX_ASTEROID_TYPE_INDEX);
        let (gif, hit_boxes, contours) = ASTEROID_IMAGE_DATA
            .get(&(size, n_idx))
            .expect("Asteroid image data should be available")
            .clone();

        // Smaller asteorid can rotate quicker.
        let rotation_speed =
            rng.gen_range(-0.075 * (size as u8 + 1) as f64..0.075 * (size as u8 + 1) as f64);

        Self {
            id: 0,
            size,
            gif,
            contours,
            hit_boxes,
            x,
            y,
            vx,
            vy,
            rotation_speed,
            ..Default::default()
        }
    }

    pub fn new_at_screen_edge() -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();

        let size = AsteroidSize::iter()
            .choose_stable(rng)
            .expect("There should be at least an asteroid size");

        let x = SCREEN_WIDTH as f64 - 4.0;
        let y = rng.gen_range(0.15 * SCREEN_HEIGHT as f64..0.85 * SCREEN_HEIGHT as f64);
        let vx = rng.gen_range(-0.35..-0.2);
        let vy = rng.gen_range(-0.15..0.15);

        Self::new(x, y, vx, vy, size)
    }
}
