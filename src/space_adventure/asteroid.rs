use super::collisions::HitBox;
use super::networking::ImageType;
use super::space_callback::SpaceCallback;
use super::visual_effects::VisualEffect;
use super::{constants::*, traits::*};
use crate::image::color_map::AsteroidColorMap;
use crate::image::types::Gif;
use crate::image::utils::{open_image, ExtraImageUtils};
use crate::register_impl;
use crate::space_adventure::utils::{body_data_from_image, EntityState};
use crate::world::resources::Resource;
use glam::{I16Vec2, Vec2};
use image::imageops::{rotate180, rotate270, rotate90};
use image::{Pixel, Rgba, RgbaImage};
use once_cell::sync::Lazy;
use rand::seq::IteratorRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;
use strum::{Display, EnumIter, IntoEnumIterator};

const MAX_ROTATION: usize = 4;

// Calculate astroid gifs, hit boxes, and contours once to be more efficient.
static ASTEROID_IMAGE_DATA: Lazy<HashMap<(AsteroidSize, usize), (Gif, Vec<HitBox>)>> =
    Lazy::new(|| {
        let mut data = HashMap::new();

        for size in AsteroidSize::iter() {
            for n_idx in 0..size.max_image_type() {
                let mut gif = vec![];
                let mut hit_boxes = vec![];

                let path = if size == AsteroidSize::Planet {
                    format!(
                        "asteroids/asteroid{}.png",
                        rand::thread_rng().gen_range(0..30)
                    )
                } else {
                    format!(
                        "space_adventure/asteroid_{}{}.png",
                        size.to_string().to_ascii_lowercase(),
                        n_idx
                    )
                };
                let mut base_img = open_image(&path).expect("Should open asteroid image");
                if size == AsteroidSize::Planet {
                    base_img.apply_color_map(AsteroidColorMap::Base.color_map());
                }

                for rotation_idx in 0..MAX_ROTATION {
                    let image = match rotation_idx {
                        0 => base_img.clone(),
                        1 => rotate90(&base_img),
                        2 => rotate180(&base_img),
                        3 => rotate270(&base_img),
                        _ => unreachable!(),
                    };
                    let (image, hit_box) =
                        body_data_from_image(&image, size == AsteroidSize::Planet);
                    gif.push(image);
                    hit_boxes.push(hit_box);
                }
                data.insert((size, n_idx), (gif, hit_boxes));
            }
        }

        data
    });

#[derive(
    Default,
    Debug,
    Display,
    EnumIter,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Serialize_repr,
    Deserialize_repr,
)]
#[repr(u8)]
pub enum AsteroidSize {
    Small,
    Big,
    #[default]
    Huge,
    Planet,
}

impl AsteroidSize {
    fn collision_damage(&self) -> f32 {
        match self {
            AsteroidSize::Small => 0.2,
            AsteroidSize::Big => 1.0,
            AsteroidSize::Huge => 4.0,
            AsteroidSize::Planet => 0.0,
        }
    }

    fn durability(&self) -> f32 {
        match self {
            AsteroidSize::Small => 3.0,
            AsteroidSize::Big => 12.0,
            AsteroidSize::Huge => 34.0,
            AsteroidSize::Planet => 0.0,
        }
    }

    fn max_image_type(&self) -> usize {
        match self {
            Self::Planet => MAX_ASTEROID_PLANET_IMAGE_TYPE,
            _ => 3,
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
    durability: f32,
    orientation: f32,
    rotation_speed: f32,
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

        if self.size != AsteroidSize::Planet {
            if self.position.x < 0.0 || self.position.x > MAX_ENTITY_POSITION.x as f32 {
                return vec![SpaceCallback::DestroyEntity { id: self.id() }];
            }
            if self.position.y < 0.0 || self.position.y > MAX_ENTITY_POSITION.y as f32 {
                return vec![SpaceCallback::DestroyEntity { id: self.id() }];
            }
        }

        self.orientation += self.rotation_speed * deltatime;

        vec![]
    }
}

impl Sprite for AsteroidEntity {
    fn image(&self) -> &RgbaImage {
        let (gif, _) = ASTEROID_IMAGE_DATA
            .get(&(self.size, self.image_type()))
            .expect("Asteroid image data should be available");

        &gif[self.frame()]
    }

    fn network_image_type(&self) -> ImageType {
        ImageType::Asteroid {
            size: self.size,
            image_type: self.image_type(),
        }
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
        if self.size == AsteroidSize::Planet {
            return;
        }
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

register_impl!(!ControllableSpaceship for AsteroidEntity);
register_impl!(!ResourceFragment for AsteroidEntity);

impl Collider for AsteroidEntity {
    fn collision_damage(&self) -> f32 {
        self.size.collision_damage()
    }

    fn collider_type(&self) -> ColliderType {
        if self.size == AsteroidSize::Planet {
            ColliderType::AsteroidPlanet
        } else {
            ColliderType::Asteroid
        }
    }

    fn hit_box(&self) -> &HitBox {
        let (_, hit_boxes) = ASTEROID_IMAGE_DATA
            .get(&(self.size, self.image_type()))
            .expect("Asteroid image data should be available");

        &hit_boxes[self.frame()]
    }
}

impl Entity for AsteroidEntity {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }

    fn id(&self) -> usize {
        self.id
    }

    fn layer(&self) -> usize {
        1
    }

    fn handle_space_callback(&mut self, callback: SpaceCallback) -> Vec<SpaceCallback> {
        match callback {
            SpaceCallback::DamageEntity { damage, .. } => {
                self.add_damage(damage);
                if self.durability() == 0.0 && self.size != AsteroidSize::Planet {
                    return vec![SpaceCallback::DestroyEntity { id: self.id() }];
                }

                self.add_visual_effect(
                    VisualEffect::COLOR_MASK_LIFETIME,
                    VisualEffect::ColorMask {
                        color: Rgba([255, 0, 0, 0]).to_rgb().0,
                    },
                );
            }
            SpaceCallback::DestroyEntity { .. } => {
                // If the asteroid got destroyed by going out-of-screen, don't spawn smaller ones.
                let should_emit_fragments = if self.position.x < 0.0
                    || self.position.x > MAX_ENTITY_POSITION.x as f32
                    || self.position.y < 0.0
                    || self.position.y > MAX_ENTITY_POSITION.y as f32
                {
                    false
                } else {
                    true
                };

                let rng = &mut ChaCha8Rng::from_entropy();
                let mut callbacks = vec![];
                let position = self.position;

                match self.size {
                    AsteroidSize::Planet => {}
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
                            if should_emit_fragments && rng.gen_bool(0.15) {
                                callbacks.push(SpaceCallback::GenerateFragment {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-3.5..3.5),
                                        rng.gen_range(-3.5..3.5),
                                    ),
                                    resource: Resource::SCRAPS,
                                    amount: 1,
                                });
                            }
                            if should_emit_fragments && rng.gen_bool(0.01) {
                                callbacks.push(SpaceCallback::GenerateFragment {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-7.5..7.5),
                                        rng.gen_range(-7.5..7.5),
                                    ),
                                    resource: Resource::GOLD,
                                    amount: 1,
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
                            if rng.gen_bool(0.75) {
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
                            if should_emit_fragments && rng.gen_bool(0.65) {
                                callbacks.push(SpaceCallback::GenerateFragment {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-3.5..3.5),
                                        rng.gen_range(-3.5..3.5),
                                    ),
                                    resource: Resource::SCRAPS,
                                    amount: 1,
                                });
                            }
                        }
                    }
                    AsteroidSize::Small => {
                        for _ in 0..4 {
                            if rng.gen_bool(0.35) {
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
                            if should_emit_fragments && rng.gen_bool(0.75) {
                                callbacks.push(SpaceCallback::GenerateFragment {
                                    position,
                                    velocity: Vec2::new(
                                        rng.gen_range(-3.5..3.5),
                                        rng.gen_range(-3.5..3.5),
                                    ),
                                    resource: Resource::SCRAPS,
                                    amount: 1,
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
    fn image_type(&self) -> usize {
        self.id % self.size.max_image_type()
    }
    fn frame(&self) -> usize {
        self.orientation as usize % MAX_ROTATION
    }

    pub fn new(position: Vec2, velocity: Vec2, size: AsteroidSize) -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();

        let rotation_speed = if size == AsteroidSize::Planet {
            0.0
        } else {
            rng.gen_range(-0.75..0.75) / (1 + size as usize) as f32
        };

        Self {
            id: 0,
            orientation: rng.gen_range(0.0..MAX_ROTATION as f32),
            rotation_speed,
            size,
            durability: size.durability(),
            position,
            velocity,
            ..Default::default()
        }
    }

    pub fn new_at_screen_edge() -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();

        let &size = [AsteroidSize::Small, AsteroidSize::Big, AsteroidSize::Huge]
            .iter()
            .choose_stable(rng)
            .expect("There should be at least an asteroid size");

        let (position, velocity) = match rng.gen_range(0..3) {
            // Right Edge
            0 => {
                let x = MAX_ENTITY_POSITION.x as f32;
                let y = rng.gen_range(
                    0.15 * MAX_ENTITY_POSITION.y as f32..0.85 * MAX_ENTITY_POSITION.y as f32,
                );
                let vx = rng.gen_range(-12.5..-0.5);
                let vy = rng.gen_range(-4.5..4.5);

                (Vec2::new(x, y), Vec2::new(vx, vy))
            }
            //Top edge
            1 => {
                let x = rng.gen_range(0.45..0.85) * MAX_ENTITY_POSITION.x as f32;
                let y = 0.0;
                let vx = rng.gen_range(-4.5..4.5);
                let vy = rng.gen_range(0.5..12.5);

                (Vec2::new(x, y), Vec2::new(vx, vy))
            }
            // Bottom edge
            2 => {
                let x = rng.gen_range(0.45..0.85) * MAX_ENTITY_POSITION.x as f32;
                let y = MAX_ENTITY_POSITION.y as f32;
                let vx = rng.gen_range(-4.5..4.5);
                let vy = rng.gen_range(-12.5..-0.5);

                (Vec2::new(x, y), Vec2::new(vx, vy))
            }

            _ => unreachable!(),
        };

        Self::new(position, velocity, size)
    }

    pub fn planet() -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();

        let x = MAX_ENTITY_POSITION.x as f32;
        let y = rng.gen_range(0.25..0.45) * MAX_ENTITY_POSITION.y as f32;
        let vx = rng.gen_range(-4.0..-3.0);
        let vy = rng.gen_range(-0.25..0.25);

        Self::new(Vec2::new(x, y), Vec2::new(vx, vy), AsteroidSize::Planet)
    }

    pub fn durability(&self) -> f32 {
        self.durability
    }

    pub fn add_damage(&mut self, damage: f32) {
        self.durability = (self.durability - damage).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{AsteroidSize, ASTEROID_IMAGE_DATA};
    use crate::types::AppResult;
    use image::{self, GenericImage, RgbaImage};

    #[ignore]
    #[test]
    fn test_generate_asteroid_image() -> AppResult<()> {
        let mut base = RgbaImage::new(260, 80);
        for (&(size, image_type), (gif, _)) in ASTEROID_IMAGE_DATA.iter() {
            if size == AsteroidSize::Planet {
                continue;
            }

            for (idx, oriented_image) in gif.iter().enumerate() {
                base.copy_from(
                    oriented_image,
                    image_type as u32 * 88 + idx as u32 * 20,
                    size as u32 * 20,
                )?;
            }
        }

        image::save_buffer(
            &Path::new("tests/asteroid_images.png"),
            &base,
            260,
            80,
            image::ColorType::Rgba8,
        )?;
        Ok(())
    }
}
