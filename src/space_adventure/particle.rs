use super::{space_callback::SpaceCallback, traits::*};
use crate::space_adventure::constants::*;
use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy)]
pub enum ParticleState {
    Immortal,
    Decaying { lifetime: f64 },
}

#[derive(Debug, Clone)]
pub struct ParticleEntity {
    id: usize,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    particle_state: ParticleState,
    image: RgbaImage,
    layer: usize,
}

impl Body for ParticleEntity {
    fn is_dynamical(&self) -> bool {
        true
    }

    fn bounds(&self) -> (Vector2D, Vector2D) {
        (
            [self.x as i16, self.y as i16],
            [self.x as i16, self.y as i16],
        )
    }

    fn position(&self) -> Vector2D {
        [self.x as i16, self.y as i16]
    }

    fn velocity(&self) -> Vector2D {
        [self.vx as i16, self.vy as i16]
    }

    fn update_body(&mut self, deltatime: f64) -> Vec<SpaceCallback> {
        let [x, y] = [self.x, self.y];
        let [vx, vy] = [self.vx, self.vy];

        let [nx, ny] = [(x + vx * deltatime), (y + vy * deltatime)];

        let mut should_destroy = false;
        if nx < 0.0 || nx > SCREEN_WIDTH as f64 {
            should_destroy = true;
        }
        if ny < 0.0 || ny > SCREEN_HEIGHT as f64 {
            should_destroy = true;
        }

        // Update parameters
        [self.x, self.y] = [nx, ny];

        match self.particle_state {
            ParticleState::Decaying { lifetime } => {
                let new_lifetime = lifetime - deltatime;
                if new_lifetime > 0.0 {
                    self.particle_state = ParticleState::Decaying {
                        lifetime: new_lifetime,
                    };
                } else {
                    should_destroy = true;
                }
            }
            _ => {}
        }

        if should_destroy {
            return vec![SpaceCallback::DestroyParticle { id: self.id() }];
        }

        vec![]
    }
}

impl Sprite for ParticleEntity {
    fn layer(&self) -> usize {
        self.layer
    }
    fn image(&self) -> &RgbaImage {
        &self.image
    }
}

impl Entity for ParticleEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }
    fn id(&self) -> usize {
        self.id
    }
}

impl ParticleEntity {
    pub fn new(
        x: f64,
        y: f64,
        vx: f64,
        vy: f64,
        color: Rgba<u8>,
        particle_state: ParticleState,
        layer: usize,
    ) -> Self {
        let image = RgbaImage::from_pixel(1, 1, color);
        Self {
            id: 0,
            x,
            y,
            vx,
            vy,
            particle_state,
            image,
            layer,
        }
    }
}
