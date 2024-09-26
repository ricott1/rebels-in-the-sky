use super::traits::*;
use crate::{
    image::types::{Gif, GifFrame},
    types::AppResult,
    world::spaceship::Spaceship,
};
use image::Rgba;
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};

#[derive(Default, Debug, Clone)]
pub struct SpaceshipEntity {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    ax: f64,
    ay: f64,
    gif: Gif,
}

impl Body for SpaceshipEntity {
    fn position(&self) -> Vector2D {
        [self.x, self.y]
    }

    fn set_position(&mut self, position: Vector2D) {
        [self.x, self.y] = position;
    }

    fn velocity(&self) -> Vector2D {
        [self.vx, self.vy]
    }

    fn set_velocity(&mut self, velocity: Vector2D) {
        [self.vx, self.vy] = velocity;
    }

    fn accelleration(&self) -> Vector2D {
        [self.ax, self.ay]
    }
    fn set_accelleration(&mut self, accelleration: Vector2D) {
        self.ax = accelleration[0] * 100.0;
        self.ay = accelleration[1] * 100.0;
    }
}

impl SpaceGif for SpaceshipEntity {
    fn gif(&self) -> Gif {
        self.gif.clone()
    }
    fn gif_frame(&self, idx: usize) -> GifFrame {
        self.gif[idx % self.gif.len()].clone()
    }
}

impl Entity for SpaceshipEntity {}

impl SpaceshipEntity {
    pub fn from_spaceship(spaceship: &Spaceship) -> AppResult<Self> {
        let gif = spaceship
            .compose_image()?
            .iter()
            .map(|img| {
                rotate_about_center(
                    img,
                    std::f32::consts::PI / 2.0,
                    Interpolation::Nearest,
                    Rgba([255, 0, 0, 0]),
                )
            })
            .collect::<Gif>();

        Ok(Self {
            gif,
            ..Default::default()
        })
    }
}
