use crate::image::color_map::ColorMap;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use strum::Display;
use strum_macros::EnumIter;

// FIXME: migrate to repr
#[derive(
    Debug, Default, Clone, Copy, Display, Serialize, Deserialize, PartialEq, Hash, EnumIter,
)]
pub enum JerseyStyle {
    #[default]
    Classic,
    Stripe,
    Fancy,
    Gilet,
    Horizontal,
    Pirate,
}

impl JerseyStyle {
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        match rng.random_range(0..=4) {
            0 => Self::Classic,
            1 => Self::Stripe,
            2 => Self::Fancy,
            3 => Self::Gilet,
            4 => Self::Horizontal,
            _ => unreachable!(),
        }
    }

    pub fn is_available_at_creation(&self) -> bool {
        !matches!(self, Self::Pirate)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Jersey {
    pub color: ColorMap,
    pub style: JerseyStyle,
}

impl Jersey {
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        let color = ColorMap::random(rng);
        let style: JerseyStyle = JerseyStyle::random(rng);
        Self { color, style }
    }
}
