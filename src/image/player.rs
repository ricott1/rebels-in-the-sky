use super::color_map::{ColorMap, HairColorMap};
use super::components::*;
use super::utils::{open_image, ExtraImageUtils};
use crate::core::jersey::{Jersey, JerseyStyle};
use crate::core::player::InfoStats;
use crate::core::role::CrewRole;
use crate::core::types::{Population, Pronoun};
use crate::image::utils::{Gif, LightMaskStyle};
use crate::types::AppResult;
use image::RgbaImage;
use rand::seq::IteratorRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde;
use serde::{Deserialize, Serialize};

pub const PLAYER_IMAGE_WIDTH: u32 = 18;
pub const PLAYER_IMAGE_HEIGHT: u32 = 40;

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PlayerImage {
    head: HeadImage,
    body: BodyImage,
    legs: LegsImage,
    hair: Option<HairImage>,
    beard: Option<BeardImage>,
    shirt: Option<ShirtImage>,
    shorts: Option<ShortsImage>,
    shoes: Option<ShoesImage>,
    pub hat: Option<HatImage>,
    pub wooden_leg: Option<WoodenLegImage>,
    pub eye_patch: Option<EyePatchImage>,
    pub hook: Option<HookImage>,
    skin_color_map: ColorMap,
    hair_color_map: ColorMap,
    jersey_color_map: Option<ColorMap>,
    pub blinking_bitmap: u16,
}

impl PlayerImage {
    pub const IMAGE_SIZE_LARGE_OFFSET: u8 = 7;
    pub fn size_from_info(info: &InfoStats) -> u8 {
        let mut size = match info.height {
            x if x <= 184.0 => 0,
            x if x <= 190.0 => 1,
            x if x <= 196.0 => 2,
            x if x <= 202.0 => 3,
            x if x <= 208.0 => 4,
            x if x <= 214.0 => 5,
            _ => 6,
        };
        let bmi = info.weight as u32 * 10_000 / (info.height as u32 * info.height as u32);
        if bmi >= 27 || info.population == Population::Pupparoll {
            size += Self::IMAGE_SIZE_LARGE_OFFSET;
        }
        size
    }

    pub fn from_info(info: &InfoStats, rng: &mut ChaCha8Rng) -> Self {
        let body = match info.population {
            Population::Polpett => BodyImage::Polpett,
            Population::Pupparoll => BodyImage::Pupparoll,
            Population::Yardalaim => BodyImage::Yardalaim,
            Population::Octopulp => BodyImage::Octopulp,
            _ => BodyImage::Normal,
        };

        let legs = match info.population {
            Population::Polpett => LegsImage::Polpett,
            Population::Pupparoll => LegsImage::Pupparoll,
            Population::Octopulp => LegsImage::Octopulp,
            _ => LegsImage::Normal,
        };

        let head = match rng.random_range(0..=1) {
            0 => match info.population {
                Population::Polpett => HeadImage::Polpett1,
                Population::Galdari => HeadImage::Gald1,
                Population::Yardalaim => HeadImage::Yardalaim1,
                Population::Juppa => HeadImage::Juppa1,
                Population::Pupparoll => HeadImage::Pupparoll1,
                Population::Octopulp => HeadImage::Octopulp1,
                _ => HeadImage::Human1,
            },
            _ => match info.population {
                Population::Polpett => HeadImage::Polpett2,
                Population::Galdari => HeadImage::Gald2,
                Population::Yardalaim => HeadImage::Yardalaim2,
                Population::Juppa => HeadImage::Juppa2,
                Population::Pupparoll => HeadImage::Pupparoll2,
                Population::Octopulp => HeadImage::Octopulp2,
                _ => HeadImage::Human2,
            },
        };

        let hair = if info.population == Population::Galdari {
            match rng.random_range(0..=4) {
                0 => Some(HairImage::Hair1),
                1 => Some(HairImage::Hair3),
                _ => None,
            }
        } else if info.population == Population::Pupparoll {
            match rng.random_range(0..=1) {
                0 => Some(HairImage::Hair8),
                _ => None,
            }
        } else if info.population == Population::Octopulp {
            None
        } else {
            let max_hair = if info.pronouns == Pronoun::She { 9 } else { 10 };
            match rng.random_range(0..=max_hair) {
                0 => Some(HairImage::Hair1),
                1 => Some(HairImage::Hair2),
                2 => Some(HairImage::Hair3),
                3 => Some(HairImage::Hair4),
                4 => Some(HairImage::Hair5),
                5 => Some(HairImage::Hair6),
                6 => Some(HairImage::Hair7),
                7 => Some(HairImage::Hair8),
                8 => Some(HairImage::Hair9),
                9 => Some(HairImage::Hair10),
                _ => None,
            }
        };

        let max_hair_color = if info.population == Population::Juppa {
            7
        } else {
            8
        };
        let hair_color_map = match rng.random_range(0..=max_hair_color) {
            0 => HairColorMap::Black,
            1 => HairColorMap::Blonde,
            2 => HairColorMap::BlondeInverted,
            3 => HairColorMap::Brown,
            4 => HairColorMap::Orange,
            5 => HairColorMap::OrangeInverted,
            6 => HairColorMap::White,
            7 => HairColorMap::Brizzolato,
            _ => HairColorMap::Blue,
        }
        .color_map();

        let beard = if info.pronouns == Pronoun::She || info.population == Population::Pupparoll {
            None
        } else if info.population == Population::Galdari {
            match rng.random_range(1..=4) {
                0 => Some(BeardImage::Beard1),
                1 => Some(BeardImage::Beard3),
                2 => Some(BeardImage::Beard4),
                3 => Some(BeardImage::Beard5),
                _ => None,
            }
        } else if info.population == Population::Octopulp {
            match rng.random_range(0..=1) {
                0 => Some(BeardImage::Octobeard1),
                1 => Some(BeardImage::Octobeard2),
                _ => unreachable!(),
            }
        } else {
            match rng.random_range(1..=6) {
                0 => Some(BeardImage::Beard1),
                1 => Some(BeardImage::Beard2),
                2 => Some(BeardImage::Beard3),
                3 => Some(BeardImage::Beard4),
                4 => Some(BeardImage::Beard5),
                _ => None,
            }
        };

        // set two random bits to 1
        let bits = (0..8).choose_multiple(rng, 2);
        let blinking_bitmap = (1 << bits[0]) | (1 << bits[1]);

        Self {
            head,
            body,
            legs,
            hair,
            beard,
            shirt: None,
            shorts: None,
            shoes: None,
            hat: None,
            wooden_leg: None,
            eye_patch: None,
            hook: None,
            skin_color_map: info.population.random_skin_map(rng).color_map(),
            hair_color_map,
            jersey_color_map: None,
            blinking_bitmap,
        }
    }

    pub fn set_jersey(&mut self, jersey: &Jersey, info: &InfoStats) {
        // This abomination is to guarantee that pirates always have the same Pirate Jersey style.
        let mut seed = [0; 8];
        for (i, c) in info.first_name.as_bytes().iter().take(4).enumerate() {
            seed[i % 8] ^= c;
        }
        for (i, c) in info.last_name.as_bytes().iter().take(4).enumerate() {
            seed[(4 + i) % 8] ^= c;
        }
        let mut rng = ChaCha8Rng::seed_from_u64(u64::from_le_bytes(seed));
        let r = rng.random_bool(0.5);

        self.shirt = match jersey.style {
            JerseyStyle::Classic => Some(ShirtImage::Classic),
            JerseyStyle::Fancy => Some(ShirtImage::Fancy),
            JerseyStyle::Gilet => Some(ShirtImage::Gilet),
            JerseyStyle::Stripe => Some(ShirtImage::Stripe),
            JerseyStyle::Horizontal => Some(ShirtImage::Horizontal),
            JerseyStyle::Pirate => {
                if info.population == Population::Galdari {
                    Some(ShirtImage::PirateGald)
                } else if r {
                    Some(ShirtImage::PirateAlt)
                } else {
                    Some(ShirtImage::Pirate)
                }
            }
        };

        self.shorts = if info.population == Population::Pupparoll {
            Some(ShortsImage::Pupparoll)
        } else {
            match jersey.style {
                JerseyStyle::Classic => Some(ShortsImage::Classic),
                JerseyStyle::Fancy => Some(ShortsImage::Fancy),
                JerseyStyle::Gilet => Some(ShortsImage::Gilet),
                JerseyStyle::Stripe => Some(ShortsImage::Stripe),
                JerseyStyle::Horizontal => Some(ShortsImage::Horizontal),
                JerseyStyle::Pirate => {
                    if info.population == Population::Galdari {
                        Some(ShortsImage::PirateGald)
                    } else if r {
                        Some(ShortsImage::PirateAlt)
                    } else {
                        Some(ShortsImage::Pirate)
                    }
                }
            }
        };

        if info.population != Population::Polpett && info.population != Population::Pupparoll {
            self.shoes = Some(ShoesImage::Classic);
        }

        match info.crew_role {
            CrewRole::Captain => {
                if r {
                    self.set_hat(Some(HatImage::Classic));
                } else {
                    self.set_hat(Some(HatImage::Infernal));
                }
            }
            CrewRole::Doctor => {
                self.set_hat(Some(HatImage::Bandana));
            }
            CrewRole::Pilot => match info.population {
                Population::Yardalaim => {
                    self.set_hat(Some(HatImage::MaskYardalaim));
                }
                Population::Polpett => {
                    self.set_hat(Some(HatImage::MaskPolpett));
                }
                Population::Galdari => {
                    self.set_hat(Some(HatImage::MaskGaldari));
                }
                Population::Pupparoll => {
                    self.set_hat(Some(HatImage::MaskPupparoll));
                }

                Population::Octopulp => {
                    if self.head == HeadImage::Octopulp1 {
                        self.set_hat(Some(HatImage::MaskOctopulp1));
                    } else if self.head == HeadImage::Octopulp2 {
                        self.set_hat(Some(HatImage::MaskOctopulp2));
                    }
                }
                _ => {
                    self.set_hat(Some(HatImage::Mask));
                }
            },
            CrewRole::Engineer => {
                if info.population == Population::Galdari {
                    self.set_hat(Some(HatImage::HipsterBlue));
                } else if info.population == Population::Octopulp {
                    self.set_hat(Some(HatImage::HipsterSlim));
                } else {
                    self.set_hat(Some(HatImage::Hipster));
                }
            }
            CrewRole::Mozzo => self.set_hat(None),
        }

        self.jersey_color_map = Some(jersey.color);
    }

    pub fn remove_jersey(&mut self) {
        self.shirt = None;
        self.shorts = None;
        self.shoes = None;
        self.jersey_color_map = None;
        self.hat = None;
    }

    fn set_hat(&mut self, hat: Option<HatImage>) {
        self.hat = hat;
    }

    pub fn set_wooden_leg(&mut self, rng: &mut ChaCha8Rng) {
        self.wooden_leg = match rng.random_range(0..=1) {
            0 => Some(WoodenLegImage::Left),
            1 => Some(WoodenLegImage::Right),
            _ => unreachable!(),
        };
    }

    pub fn set_eye_patch(&mut self, rng: &mut ChaCha8Rng, population: Population) {
        self.eye_patch = match population {
            Population::Galdari => match rng.random_range(0..=2) {
                0 => Some(EyePatchImage::LeftLow),
                1 => Some(EyePatchImage::RightLow),
                2 => Some(EyePatchImage::Central),
                _ => unreachable!(),
            },
            Population::Polpett | Population::Yardalaim => match rng.random_range(0..=1) {
                0 => Some(EyePatchImage::LeftLow),
                1 => Some(EyePatchImage::RightLow),
                _ => unreachable!(),
            },
            Population::Octopulp => {
                if self.head == HeadImage::Octopulp1 {
                    Some(EyePatchImage::OctopulpCentral)
                } else if self.head == HeadImage::Octopulp2 {
                    match rng.random_range(0..=1) {
                        0 => Some(EyePatchImage::LeftLow),
                        1 => Some(EyePatchImage::RightLow),
                        _ => unreachable!(),
                    }
                } else {
                    unreachable!()
                }
            }
            Population::Pupparoll => Some(EyePatchImage::Pupparoll),
            _ => match rng.random_range(0..=3) {
                0 => Some(EyePatchImage::LeftHigh),
                1 => Some(EyePatchImage::LeftLow),
                2 => Some(EyePatchImage::RightHigh),
                3 => Some(EyePatchImage::RightLow),
                _ => unreachable!(),
            },
        };
    }

    pub fn set_hook(&mut self, rng: &mut ChaCha8Rng, population: Population) {
        self.hook = if population == Population::Pupparoll {
            match rng.random_range(0..=1) {
                0 => Some(HookImage::LeftPupparoll),
                1 => Some(HookImage::RightPupparoll),
                _ => unreachable!(),
            }
        } else {
            match rng.random_range(0..=1) {
                0 => Some(HookImage::Left),
                1 => Some(HookImage::Right),
                _ => unreachable!(),
            }
        };
    }

    pub fn compose(&self, info: &InfoStats) -> AppResult<Gif> {
        let size = Self::size_from_info(info);
        let mut base = RgbaImage::new(PLAYER_IMAGE_WIDTH, PLAYER_IMAGE_HEIGHT);
        let mut blinking_base = RgbaImage::new(PLAYER_IMAGE_WIDTH, PLAYER_IMAGE_HEIGHT);
        let img_height = base.height();
        let mut offset_y = 0;
        let skin_color_map = self.skin_color_map;
        let hair_color_map = self.hair_color_map;
        let jersey_color_map = self.jersey_color_map;

        let mut other = self.legs.image(size)?;
        let mask = self.legs.mask(size)?;

        other.apply_color_map_with_shadow_mask(skin_color_map, &mask);

        offset_y += other.height();
        let x = (base.width() - other.width()) / 2;
        base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;
        blinking_base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;

        if let Some(shoes) = self.shoes.as_ref() {
            let mut other = shoes.image(size)?;
            let x = (base.width() - other.width()) / 2;
            if let Some(color_map) = jersey_color_map {
                other.apply_color_map(color_map);
            }
            base.copy_non_trasparent_from(&other, x, img_height - other.height())?;
            blinking_base.copy_non_trasparent_from(&other, x, img_height - other.height())?;
        }

        if let Some(shorts) = self.shorts.as_ref() {
            let mut other = shorts.image(size)?;
            let x = (base.width() - other.width()) / 2;
            if let Some(color_map) = jersey_color_map {
                let mask = shorts.mask(size)?;
                other.apply_color_map_with_shadow_mask(color_map, &mask);
            }
            base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;
            blinking_base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;
        }

        if let Some(wooden_leg) = self.wooden_leg.as_ref() {
            //Polpett and Pupparoll have small legs regardless of size
            let leg_size = if info.population == Population::Polpett
                || info.population == Population::Pupparoll
            {
                0
            } else {
                size
            };

            let other = wooden_leg.image(leg_size)?;
            // Polpett have the wooden leg moved to the side
            let offset = if info.population == Population::Polpett {
                if size >= Self::IMAGE_SIZE_LARGE_OFFSET {
                    2
                } else {
                    1
                }
            } else if info.population == Population::Pupparoll {
                1
            } else {
                0
            };
            let x = match wooden_leg {
                WoodenLegImage::Left => base.width() / 2 - other.width() - offset,
                WoodenLegImage::Right => base.width() / 2 + offset,
            };

            // Clear the shoe/foot on the base
            match wooden_leg {
                WoodenLegImage::Left => {
                    for x in 0..base.width() / 2 {
                        for y in img_height - other.height()..img_height {
                            base.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                            blinking_base.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                        }
                    }
                }
                WoodenLegImage::Right => {
                    for x in base.width() / 2..base.width() {
                        for y in img_height - other.height()..img_height {
                            base.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                            blinking_base.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                        }
                    }
                }
            }

            base.copy_non_trasparent_from(&other, x, img_height - other.height())?;
            blinking_base.copy_non_trasparent_from(&other, x, img_height - other.height())?;
        }

        let mut other = self.body.image(size)?;
        let mask = self.body.mask(size)?;
        offset_y += other.height() - 1;
        let body_x = (base.width() - other.width()) / 2;
        other.apply_color_map_with_shadow_mask(skin_color_map, &mask);
        base.copy_non_trasparent_from(&other, body_x, img_height - offset_y)?;
        blinking_base.copy_non_trasparent_from(&other, body_x, img_height - offset_y)?;

        if let Some(hook) = self.hook.as_ref() {
            let mut hook_img = hook.image()?;

            if let Some(color_map) = jersey_color_map {
                hook_img.apply_color_map(color_map);
            }

            let x = match hook {
                HookImage::Left | HookImage::LeftPupparoll => body_x - 1,
                HookImage::Right | HookImage::RightPupparoll => {
                    body_x + other.width() - hook_img.width() + 1
                }
            };

            let y = img_height - offset_y + other.height() - 4;

            // Clear the arm on the base
            for cx in x + 1..x + hook_img.width() {
                for cy in y..y + hook_img.height() {
                    base.put_pixel(cx, cy, image::Rgba([0, 0, 0, 0]));
                    blinking_base.put_pixel(cx, cy, image::Rgba([0, 0, 0, 0]));
                }
            }

            base.copy_non_trasparent_from(&hook_img, x, y)?;
            blinking_base.copy_non_trasparent_from(&hook_img, x, y)?;
        }

        if let Some(shirt) = self.shirt.as_ref() {
            let mut other = shirt.image(size)?;
            let x = (base.width() - other.width()) / 2;
            if let Some(color_map) = jersey_color_map {
                let mask = shirt.mask(size)?;
                other.apply_color_map_with_shadow_mask(color_map, &mask);
            }
            base.copy_non_trasparent_from(&other, x, img_height - offset_y + 1)?;
            blinking_base.copy_non_trasparent_from(&other, x, img_height - offset_y + 1)?;
        }

        let mut other = self.head.image()?;
        let mut blinking = self.head.image()?;
        let mask = self.head.mask()?;
        offset_y += other.height() - 5;
        let x = (base.width() - other.width()) / 2;
        let mut cm = skin_color_map;
        other.apply_color_map_with_shadow_mask(cm, &mask);
        base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;
        cm.blue = cm.red;
        blinking.apply_color_map_with_shadow_mask(cm, &mask);
        blinking_base.copy_non_trasparent_from(&blinking, x, img_height - offset_y)?;

        if let Some(eye_patch) = self.eye_patch.as_ref() {
            let other = eye_patch.image()?;
            let x = (base.width() - other.width()) / 2;
            base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;
            blinking_base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;
        }

        if let Some(hair) = self.hair.as_ref() {
            let mut other = open_image(hair.select_file().as_str())?;
            let x = (base.width() - other.width()) / 2;
            other.apply_color_map(hair_color_map);

            let y = if info.population == Population::Pupparoll {
                img_height - offset_y - 1
            } else {
                img_height - offset_y
            };
            if let Some(HatImage::Classic | HatImage::Infernal | HatImage::Hipster) =
                self.hat.as_ref()
            {
                // Clear extra hair from upper half of hat if hat is Classic, Infernal or Hipster and population is not Polpett
                for cx in 0..other.width() {
                    for cy in 0..5 {
                        other.put_pixel(cx, cy, image::Rgba([0, 0, 0, 0]));
                    }
                }
            }

            base.copy_non_trasparent_from(&other, x, y)?;
            blinking_base.copy_non_trasparent_from(&other, x, y)?;
        }

        if let Some(beard) = self.beard.as_ref() {
            let mut other = open_image(beard.select_file().as_str())?;
            let x = (base.width() - other.width()) / 2;
            if info.population == Population::Octopulp {
                other.apply_color_map(skin_color_map);
            } else {
                other.apply_color_map(hair_color_map);
            }
            base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;
            blinking_base.copy_non_trasparent_from(&other, x, img_height - offset_y)?;
        }

        if let Some(hat) = self.hat.as_ref() {
            let other = open_image(hat.select_file().as_str())?;
            let x = (base.width() - other.width()) / 2;
            offset_y += 2;
            let y = if info.population == Population::Pupparoll {
                img_height - offset_y - 1
            } else {
                img_height - offset_y
            };
            base.copy_non_trasparent_from(&other, x, y)?;
            blinking_base.copy_non_trasparent_from(&other, x, y)?;
        }

        // Apply light masks
        base.apply_light_mask(&LightMaskStyle::player());
        blinking_base.apply_light_mask(&LightMaskStyle::player());

        let mut gif = Gif::new();
        for tick in 0..16 {
            if (self.blinking_bitmap >> tick) & 1 == 1 {
                gif.push(blinking_base.clone());
            } else {
                gif.push(base.clone());
            }
        }
        Ok(gif)
    }
}

#[cfg(test)]
mod tests {
    use super::{PLAYER_IMAGE_HEIGHT, PLAYER_IMAGE_WIDTH};
    use crate::{
        core::{
            jersey::Jersey,
            player::{InfoStats, Player},
            role::CrewRole,
            types::Population,
        },
        image::{player::PlayerImage, utils::ExtraImageUtils},
        types::AppResult,
    };
    use image::{self, RgbaImage};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use std::path::Path;
    use strum::IntoEnumIterator;

    #[ignore]
    #[test]
    fn test_generate_naked_player_image() -> AppResult<()> {
        let rng = &mut ChaCha8Rng::seed_from_u64(0);
        let n = 5;
        for population in Population::iter() {
            let mut base = RgbaImage::new(PLAYER_IMAGE_WIDTH * n, PLAYER_IMAGE_HEIGHT);
            for i in 0..n {
                let info = InfoStats {
                    population,
                    height: 190.0 + 5.0 * i as f32,
                    weight: 100.0,
                    ..Default::default()
                };

                let player_image = PlayerImage::from_info(&info, rng);
                base.copy_non_trasparent_from(
                    &player_image.compose(&info)?[0],
                    (PLAYER_IMAGE_WIDTH * i) as u32,
                    0,
                )?;
            }
            image::save_buffer(
                &Path::new(
                    format!(
                        "tests/images/player_image_{}.png",
                        population.to_string().to_lowercase()
                    )
                    .as_str(),
                ),
                &base,
                PLAYER_IMAGE_WIDTH * n,
                PLAYER_IMAGE_HEIGHT,
                image::ColorType::Rgba8,
            )?;
        }
        Ok(())
    }

    #[ignore]
    #[test]
    fn test_generate_jerseyed_player_image() -> AppResult<()> {
        let rng = &mut ChaCha8Rng::seed_from_u64(0);
        let n = 5;
        for population in Population::iter() {
            let mut base = RgbaImage::new(PLAYER_IMAGE_WIDTH * n, PLAYER_IMAGE_HEIGHT);
            for i in 0..n {
                let mut player = Player::default()
                    .with_population(population)
                    .randomize(None);

                let crew_role = match i {
                    0 => CrewRole::Captain,
                    1 => CrewRole::Doctor,
                    2 => CrewRole::Pilot,
                    3 => CrewRole::Engineer,
                    _ => CrewRole::Mozzo,
                };
                player.info.crew_role = crew_role;
                player.set_jersey(&Jersey::random(rng));

                let player_image = player.compose_image()?;
                base.copy_non_trasparent_from(
                    &player_image[0],
                    (PLAYER_IMAGE_WIDTH * i) as u32,
                    0,
                )?;
            }
            image::save_buffer(
                &Path::new(
                    format!(
                        "tests/images/player_image_{}_with_jersey.png",
                        population.to_string().to_lowercase()
                    )
                    .as_str(),
                ),
                &base,
                PLAYER_IMAGE_WIDTH * n,
                PLAYER_IMAGE_HEIGHT,
                image::ColorType::Rgba8,
            )?;
        }
        Ok(())
    }
}
