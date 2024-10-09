use super::{asteroid::AsteroidSize, particle::ParticleState, space::Space, Body};
use crate::types::AppResult;
use image::Rgba;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub enum SpaceCallback {
    DestroyAsteroidEntity {
        id: usize,
    },
    DestroyParticle {
        id: usize,
    },

    GenerateParticle {
        x: f64,
        y: f64,
        vx: f64,
        vy: f64,
        color: Rgba<u8>,
        particle_state: ParticleState,
        layer: usize,
    },
}

impl SpaceCallback {
    pub fn call(&self, space: &mut Space) -> AppResult<()> {
        match *self {
            Self::DestroyAsteroidEntity { id } => {
                if let Some(asteroid) = space.get_asteroid(&id) {
                    match asteroid.size {
                        AsteroidSize::Big => {
                            let [x, y] = asteroid.position();
                            let [vx, vy] = asteroid.velocity();
                            let rng = &mut ChaCha8Rng::from_entropy();
                            let [rx, ry] = [rng.gen_range(0.5..1.5), rng.gen_range(0.5..1.5)];
                            let s = (rng.gen_range(0..=2) - 1) as f64;

                            space.generate_asteroid(
                                x as f64,
                                y as f64,
                                vx as f64 + rx,
                                vy as f64 + s * ry,
                                AsteroidSize::Small,
                            );

                            space.generate_asteroid(
                                x as f64,
                                y as f64,
                                vx as f64 - rx,
                                vy as f64 - s * ry,
                                AsteroidSize::Small,
                            );

                            space.generate_asteroid(
                                x as f64,
                                y as f64,
                                vx as f64 / 4.0,
                                vy as f64 / 4.0,
                                AsteroidSize::Fragment,
                            );

                            if rng.gen_bool(0.25) {
                                space.generate_asteroid(
                                    x as f64,
                                    y as f64,
                                    vx as f64 / 4.0 - ry / 2.0,
                                    vy as f64 / 4.0 + s * rx / 2.0,
                                    AsteroidSize::Fragment,
                                );
                            }
                        }
                        AsteroidSize::Small => {
                            let [x, y] = asteroid.position();
                            let [vx, vy] = asteroid.velocity();
                            space.generate_asteroid(
                                x as f64,
                                y as f64,
                                vx as f64,
                                vy as f64,
                                AsteroidSize::Fragment,
                            );
                        }
                        AsteroidSize::Fragment => {}
                    }

                    space.remove_asteroid(&id);
                }
            }

            Self::DestroyParticle { id } => {
                space.remove_particle(&id);
            }

            Self::GenerateParticle {
                x,
                y,
                vx,
                vy,
                color,
                particle_state,
                layer,
            } => {
                space.generate_particle(x, y, vx, vy, color, particle_state, layer);
            }
        }
        Ok(())
    }
}
