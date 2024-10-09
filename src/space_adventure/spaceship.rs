use super::particle::ParticleState;
use super::space_callback::SpaceCallback;
use super::{constants::*, traits::*};
use crate::image::components::ImageComponent;
use crate::image::utils::open_image;
use crate::{image::types::Gif, types::AppResult, world::spaceship::Spaceship};
use image::imageops::crop_imm;
use image::{buffer::ConvertBuffer, GrayImage, Rgba, RgbaImage};
use imageproc::{
    contours::{find_contours, BorderType},
    geometric_transformations::{rotate_about_center, Interpolation},
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::{HashMap, HashSet};

#[derive(Default, Debug)]
pub struct SpaceshipEntity {
    id: usize,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    ax: f64,
    ay: f64,
    contours: HashSet<(i16, i16)>, // Could be u16 but we keep i16 for convenience
    // We need a single hit box since it does not change with the gif.
    // Maps with hit box points, Point -> is_border
    hit_box: HashMap<(i16, i16), bool>,
    thrust: f64,
    fuel: f64,
    fuel_capacity: u32,
    fuel_consumption: f64,
    friction_coeff: f64,
    tick: usize,
    gif: Gif,
    engine_exhaust: Vec<(i16, i16)>, // Position of exhaust in relative coords
}

impl Body for SpaceshipEntity {
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
        [self.ax as i16, self.ay as i16]
    }

    fn push_left(&mut self) {
        if self.fuel == 0.0 {
            return;
        }

        if self.vx > 0.0 {
            self.ax = -1.0 * self.thrust * BREAKING_EXTRA_MOD;
        } else {
            self.ax = -1.0 * self.thrust;
        }

        self.fuel = (self.fuel - self.fuel_consumption).max(0.0);
    }

    fn push_right(&mut self) {
        if self.fuel == 0.0 {
            return;
        }

        if self.vx >= 0.0 {
            self.ax = 1.0 * self.thrust;
        } else {
            self.ax = 1.0 * self.thrust * BREAKING_EXTRA_MOD;
        }

        self.fuel = (self.fuel - self.fuel_consumption).max(0.0);
    }

    fn push_up(&mut self) {
        if self.fuel == 0.0 {
            return;
        }

        if self.vy > 0.0 {
            self.ay = -1.0 * self.thrust * BREAKING_EXTRA_MOD;
        } else {
            self.ay = -1.0 * self.thrust;
        }

        self.fuel = (self.fuel - self.fuel_consumption).max(0.0);
    }

    fn push_down(&mut self) {
        if self.fuel == 0.0 {
            return;
        }

        if self.vy >= 0.0 {
            self.ay = 1.0 * self.thrust;
        } else {
            self.ay = 1.0 * self.thrust * BREAKING_EXTRA_MOD;
        }

        self.fuel = (self.fuel - self.fuel_consumption).max(0.0);
    }

    fn update_body(&mut self, deltatime: f64) -> Vec<SpaceCallback> {
        self.tick += 1;

        // Get current parameters
        let [x, y] = [self.x, self.y];
        let [vx, vy] = [self.vx, self.vy];
        let [mut ax, mut ay] = [self.ax, self.ay];

        let mut callbacks = vec![];
        if ax != 0.0 || ay != 0.0 {
            let rng = &mut ChaCha8Rng::from_entropy();
            for (ex, ey) in self.engine_exhaust.iter() {
                if vx.powf(2.0) + vy.powf(2.0) < 2.0 * self.thrust || rng.gen_bool(0.3) {
                    let layer = rng.gen_range(0..2);
                    callbacks.push(SpaceCallback::GenerateParticle {
                        x: *ex as f64 + x,
                        y: *ey as f64 + y,
                        vx: -3.0 * ax / ax.abs().max(1.0) + rng.gen_range(-0.5..0.5),
                        vy: -3.0 * ay / ay.abs().max(1.0) + rng.gen_range(-0.5..0.5),
                        color: Rgba([
                            205 + rng.gen_range(0..50),
                            55 + rng.gen_range(0..200),
                            rng.gen_range(0..55),
                            255,
                        ]),
                        particle_state: ParticleState::Decaying {
                            lifetime: 2.0 + rng.gen_range(0.0..1.5),
                        },
                        layer,
                    });
                }
            }
        }

        ax = ax
            - self.friction_coeff
                * if vx.powf(2.0) > self.thrust {
                    vx.powf(2.0) * vx.signum()
                } else {
                    vx
                };
        ay = ay
            - self.friction_coeff
                * if vy.powf(2.0) > self.thrust {
                    vy.powf(2.0) * vy.signum()
                } else {
                    vy
                };
        let [mut nvx, mut nvy] = [vx + ax * deltatime, vy + ay * deltatime];

        let [mut nx, mut ny] = [(x + nvx * deltatime), (y + nvy * deltatime)];

        let bounds = self.bounds();
        if nx < 0.0 {
            nx = 0.0;
            nvx = 0.0;
        } else if bounds.1[0] > SCREEN_WIDTH as i16 {
            nx = (SCREEN_WIDTH - self.image().width() as u16) as f64;
            nvx = 0.0;
        }
        if ny < 0.0 {
            ny = 0.0;
            nvy = 0.0;
        } else if bounds.1[1] > SCREEN_HEIGHT as i16 {
            ny = (SCREEN_HEIGHT - self.image().height() as u16) as f64;
            nvy = 0.0;
        }

        // Update parameters
        [self.ax, self.ay] = [0.0, 0.0];
        [self.vx, self.vy] = [nvx, nvy];
        [self.x, self.y] = [nx, ny];

        callbacks
    }
}

impl Sprite for SpaceshipEntity {
    fn layer(&self) -> usize {
        1
    }

    fn image(&self) -> &RgbaImage {
        &self.gif[self.tick % self.gif.len()]
    }

    fn update_sprite(&mut self, _: f64) -> Vec<SpaceCallback> {
        self.tick += 1;
        vec![]
    }

    fn mask(&self) -> HashSet<(i16, i16)> {
        self.contours
            .iter()
            .map(|&p| (p.0 + self.x as i16, p.1 + self.y as i16))
            .collect::<HashSet<_>>()
    }

    fn hit_box(&self) -> HashMap<(i16, i16), bool> {
        self.hit_box
            .iter()
            .map(|(&p, &is_border)| ((p.0 + self.x as i16, p.1 + self.y as i16), is_border))
            .collect::<HashMap<_, _>>()
    }
}

impl Entity for SpaceshipEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }
    fn id(&self) -> usize {
        self.id
    }
}

impl SpaceshipEntity {
    pub fn from_spaceship(spaceship: &Spaceship, storage_units: u32, fuel: u32) -> AppResult<Self> {
        let mut gif = spaceship
            .compose_image()?
            .iter()
            .map(|img| {
                rotate_about_center(
                    img,
                    std::f32::consts::PI / 2.0,
                    Interpolation::Nearest,
                    Rgba([0, 0, 0, 0]),
                )
            })
            .collect::<Gif>();

        let mut engine_img = open_image(&spaceship.engine.select_mask_file(0))?;
        engine_img = rotate_about_center(
            &engine_img,
            std::f32::consts::PI / 2.0,
            Interpolation::Nearest,
            Rgba([0, 0, 0, 0]),
        );

        let gray_img = ConvertBuffer::<GrayImage>::convert(&gif[0]);
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
        for idx in 0..gif.len() {
            gif[idx] = crop_imm(
                &gif[idx],
                min_x as u32,
                min_y as u32,
                (max_x - min_x) as u32 + 1,
                (max_y - min_y) as u32 + 1,
            )
            .to_image();
        }

        let mut engine_exhaust = vec![];
        for x in 0..engine_img.width() {
            for y in 0..engine_img.height() {
                if let Some(pixel) = engine_img.get_pixel_checked(x, y) {
                    // If pixel is blue, it is at the exhaust position.
                    if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 255 && pixel[3] > 0 {
                        engine_exhaust.push((x as i16, y as i16));
                    }
                }
            }
        }

        assert!(engine_exhaust.len() > 0);

        // Translate contours.
        let contours = contours_vec
            .iter()
            .map(|&point| (point.x - min_x, point.y - min_y))
            .collect::<HashSet<_>>();

        let mut hit_box = HashMap::new();

        for x in 0..gif[0].width() {
            for y in 0..gif[0].height() {
                if let Some(pixel) = gif[0].get_pixel_checked(x, y) {
                    // If pixel is non-transparent.
                    if pixel[3] > 0 {
                        let is_border = contours.contains(&(x as i16, y as i16));
                        hit_box.insert((x as i16, y as i16), is_border);
                    }
                }
            }
        }

        Ok(Self {
            id: 0,
            gif,
            contours,
            hit_box,
            thrust: spaceship.speed(storage_units) as f64 * ACCELERATION_MOD,
            fuel: fuel as f64,
            fuel_capacity: spaceship.fuel_capacity(),
            fuel_consumption: spaceship.fuel_consumption(storage_units) as f64,
            friction_coeff: FRICTION_COEFF,
            engine_exhaust,
            ..Default::default()
        })
    }

    pub fn fuel(&self) -> u32 {
        self.fuel.round() as u32
    }

    pub fn fuel_capacity(&self) -> u32 {
        self.fuel_capacity
    }
}
