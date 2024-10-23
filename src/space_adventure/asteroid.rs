use super::space_callback::SpaceCallback;
use super::visual_effects::VisualEffect;
use super::{constants::*, traits::*};
use crate::image::types::Gif;
use crate::image::utils::open_image;
use crate::register_impl;
use crate::space_adventure::utils::{body_data_from_image, EntityState};
use crate::world::resources::Resource;
use glam::{I16Vec2, Vec2};
use image::imageops::{rotate180, rotate270, rotate90};
use image::{Rgba, RgbaImage};
use once_cell::sync::Lazy;
use rand::seq::IteratorRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use strum::{Display, EnumIter, IntoEnumIterator};

const MAX_ASTEROID_TYPE_INDEX: usize = 3;
const MAX_ROTATION: usize = 4;

// Calculate astroid gifs, hit boxes, and contours once to be more efficient.
static ASTEROID_IMAGE_DATA: Lazy<HashMap<(AsteroidSize, usize), (Gif, Vec<HitBox>)>> =
    Lazy::new(|| {
        let mut data = HashMap::new();

        for size in AsteroidSize::iter() {
            for n_idx in 1..=MAX_ASTEROID_TYPE_INDEX {
                let mut gif = vec![];
                let mut hit_boxes = vec![];
                let path = format!(
                    "space_adventure/asteroid_{}{}.png",
                    size.to_string().to_ascii_lowercase(),
                    n_idx
                );
                let base_img = open_image(&path).expect("Should open asteroid image");
                for rotation_idx in 0..MAX_ROTATION {
                    let image = match rotation_idx {
                        0 => base_img.clone(),
                        1 => rotate90(&base_img),
                        2 => rotate180(&base_img),
                        3 => rotate270(&base_img),
                        _ => unreachable!(),
                    };
                    let (image, hit_box) = body_data_from_image(&image);
                    gif.push(image);
                    hit_boxes.push(hit_box);
                }
                data.insert((size, n_idx), (gif, hit_boxes));
            }
        }

        data
    });

#[derive(Default, Debug, Display, EnumIter, PartialEq, Eq, Clone, Copy, Hash)]
pub enum AsteroidSize {
    #[default]
    Huge,
    Big,
    Small,
}

impl AsteroidSize {
    pub fn collision_damage(&self) -> f32 {
        match self {
            AsteroidSize::Huge => 3.0,
            AsteroidSize::Big => 1.0,
            AsteroidSize::Small => 0.2,
        }
    }

    pub fn durability(&self) -> f32 {
        match self {
            AsteroidSize::Huge => 24.0,
            AsteroidSize::Big => 12.0,
            AsteroidSize::Small => 3.0,
        }
    }
}

#[derive(Default, Debug)]
pub struct AsteroidEntity {
    id: usize,
    previous_position: Vec2,
    position: Vec2,
    velocity: Vec2,
    size: AsteroidSize,
    image_type: usize,
    durability: f32,
    tick: usize,
    visual_effects: VisualEffectMap,
}

impl Body for AsteroidEntity {
    fn previous_position(&self) -> I16Vec2 {
        self.previous_position.as_i16vec2()
    }

    fn position(&self) -> I16Vec2 {
        self.position.as_i16vec2()
    }

    fn velocity(&self) -> I16Vec2 {
        self.velocity.as_i16vec2()
    }

    fn update_body(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        self.previous_position = self.position;
        self.position = self.position + self.velocity * deltatime;

        if self.position.x < 0.0 || self.position.x > MAX_SCREEN_WIDTH as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }
        if self.position.y < 0.0 || self.position.y > MAX_SCREEN_HEIGHT as f32 {
            return vec![SpaceCallback::DestroyEntity { id: self.id() }];
        }

        vec![]
    }
}

impl Sprite for AsteroidEntity {
    fn layer(&self) -> usize {
        1
    }

    fn image(&self) -> &RgbaImage {
        let (gif, _) = ASTEROID_IMAGE_DATA
            .get(&(self.size, self.image_type))
            .expect("Asteroid image data should be available");

        &gif[self.frame()]
    }

    fn hit_box(&self) -> &HitBox {
        let (_, hit_boxes) = ASTEROID_IMAGE_DATA
            .get(&(self.size, self.image_type))
            .expect("Asteroid image data should be available");

        &hit_boxes[self.frame()]
    }

    fn should_apply_visual_effects<'a>(&self) -> bool {
        self.visual_effects.len() > 0
    }

    fn apply_visual_effects<'a>(&'a self, image: &'a RgbaImage) -> RgbaImage {
        let mut image = image.clone();
        if self.visual_effects.len() > 0 {
            for (effect, time) in self.visual_effects.iter() {
                effect.apply(self, &mut image, *time);
            }
        }
        image
    }

    fn add_visual_effect(&mut self, duration: f32, effect: VisualEffect) {
        self.visual_effects.insert(effect, duration);
    }

    fn remove_visual_effect(&mut self, effect: &VisualEffect) {
        self.visual_effects.remove(&effect);
    }

    fn update_sprite(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        for (_, lifetime) in self.visual_effects.iter_mut() {
            *lifetime -= deltatime;
        }

        self.visual_effects.retain(|_, lifetime| *lifetime > 0.0);

        vec![]
    }
}

register_impl!(!PlayerControlled for AsteroidEntity);
register_impl!(!ResourceFragment for AsteroidEntity);

impl Collider for AsteroidEntity {
    fn collision_damage(&self) -> f32 {
        self.size.collision_damage()
    }

    fn collider_type(&self) -> ColliderType {
        ColliderType::Asteroid
    }
}

impl Entity for AsteroidEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }
    fn id(&self) -> usize {
        self.id
    }

    fn handle_space_callback(&mut self, callback: SpaceCallback) -> Vec<SpaceCallback> {
        match callback {
            SpaceCallback::DamageEntity { damage, .. } => {
                self.add_damage(damage);
                if self.durability() == 0.0 {
                    return vec![SpaceCallback::DestroyEntity { id: self.id() }];
                }

                self.add_visual_effect(
                    VisualEffect::COLOR_MASK_LIFETIME,
                    VisualEffect::ColorMask {
                        color: Rgba([255, 0, 0, 0]),
                    },
                );
            }
            SpaceCallback::DestroyEntity { .. } => {
                let position = self.position().as_vec2();
                let rng = &mut ChaCha8Rng::from_entropy();
                let mut callbacks = vec![];
                match self.size {
                    AsteroidSize::Huge => {
                        for _ in 0..3 {
                            if rng.gen_bool(0.95) {
                                callbacks.push(SpaceCallback::GenerateAsteroid {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-4.5..4.5),
                                        rng.gen_range(-4.5..4.5),
                                    ),
                                    size: AsteroidSize::Big,
                                });
                            }
                        }

                        for _ in 0..4 {
                            if rng.gen_bool(0.75) {
                                callbacks.push(SpaceCallback::GenerateAsteroid {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-5.5..5.5),
                                        rng.gen_range(-5.5..5.5),
                                    ),
                                    size: AsteroidSize::Small,
                                });
                            }
                        }

                        for _ in 0..8 {
                            if rng.gen_bool(0.85) {
                                callbacks.push(SpaceCallback::GenerateParticle {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-3.5..3.5),
                                        rng.gen_range(-3.5..3.5),
                                    ),
                                    color: Rgba([
                                        55 + rng.gen_range(0..25),
                                        55 + rng.gen_range(0..25),
                                        55 + rng.gen_range(0..25),
                                        255,
                                    ]),
                                    particle_state: EntityState::Decaying {
                                        lifetime: 2.0 + rng.gen_range(0.0..1.5),
                                    },
                                    layer: rng.gen_range(0..=2),
                                });
                            }
                            if rng.gen_bool(0.15) {
                                callbacks.push(SpaceCallback::GenerateFragment {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-3.5..3.5),
                                        rng.gen_range(-3.5..3.5),
                                    ),
                                    resource: Resource::SCRAPS,
                                    amount: rng.gen_range(1..=4),
                                });
                            }
                        }
                    }
                    AsteroidSize::Big => {
                        for _ in 0..3 {
                            if rng.gen_bool(0.85) {
                                callbacks.push(SpaceCallback::GenerateAsteroid {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-4.5..4.5),
                                        rng.gen_range(-4.5..4.5),
                                    ),
                                    size: AsteroidSize::Small,
                                });
                            }
                        }

                        for _ in 0..6 {
                            if rng.gen_bool(0.65) {
                                callbacks.push(SpaceCallback::GenerateParticle {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-3.5..3.5),
                                        rng.gen_range(-3.5..3.5),
                                    ),
                                    color: Rgba([
                                        55 + rng.gen_range(0..25),
                                        55 + rng.gen_range(0..25),
                                        55 + rng.gen_range(0..25),
                                        255,
                                    ]),
                                    particle_state: EntityState::Decaying {
                                        lifetime: 2.0 + rng.gen_range(0.0..1.5),
                                    },
                                    layer: rng.gen_range(0..=2),
                                });
                            }
                            if rng.gen_bool(0.75) {
                                callbacks.push(SpaceCallback::GenerateFragment {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-3.5..3.5),
                                        rng.gen_range(-3.5..3.5),
                                    ),
                                    resource: Resource::SCRAPS,
                                    amount: rng.gen_range(1..=4),
                                });
                            }
                        }
                    }
                    AsteroidSize::Small => {
                        for _ in 0..4 {
                            if rng.gen_bool(0.75) {
                                callbacks.push(SpaceCallback::GenerateFragment {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-3.5..3.5),
                                        rng.gen_range(-3.5..3.5),
                                    ),
                                    resource: Resource::SCRAPS,
                                    amount: rng.gen_range(1..=4),
                                });
                            }
                        }
                    }
                }
                return callbacks;
            }
            _ => {}
        }

        vec![]
    }
}

impl AsteroidEntity {
    fn frame(&self) -> usize {
        self.tick
    }

    pub fn new(position: Vec2, velocity: Vec2, size: AsteroidSize) -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();
        let image_type = rng.gen_range(1..=MAX_ASTEROID_TYPE_INDEX);

        // TODO: decide if we like them rotating or not.
        let tick = rng.gen_range(0..MAX_ROTATION);

        Self {
            id: 0,
            tick,
            size,
            image_type,
            durability: size.durability(),
            position,
            velocity,
            ..Default::default()
        }
    }

    pub fn new_at_screen_edge() -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();

        let size = AsteroidSize::iter()
            .choose_stable(rng)
            .expect("There should be at least an asteroid size");

        let x = (SCREEN_WIDTH + 2) as f32;
        let y = rng.gen_range(0.15 * SCREEN_HEIGHT as f32..0.85 * SCREEN_HEIGHT as f32);
        let vx = rng.gen_range(-1.5..-0.5);
        let vy = rng.gen_range(-0.15..0.15);

        let position = Vec2::new(x, y);
        let velocity = Vec2::new(vx, vy);

        Self::new(position, velocity, size)
    }

    pub fn durability(&self) -> f32 {
        self.durability
    }

    pub fn add_damage(&mut self, damage: f32) {
        self.durability = (self.durability - damage).max(0.0);
    }
}
