use crate::image::color_map::ColorMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use strum::Display;
use strum_macros::EnumIter;

#[derive(
    Debug, Default, Clone, Copy, Display, Serialize, Deserialize, PartialEq,  Hash, EnumIter,
)]
pub enum JerseyStyle {
    #[default]
    Classic,
    Stripe,
    Fancy,
    Gilet,
    Pirate,
}

impl JerseyStyle {
    pub fn random() -> Self {
        match rand::thread_rng().gen_range(0..=3) {
            0 => Self::Classic,
            1 => Self::Stripe,
            2 => Self::Fancy,
            _ => Self::Gilet,
        }
    }

    pub fn is_available_at_creation(&self) -> bool {
        match self {
            Self::Pirate => false,
            _ => true,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Jersey {
    pub color: ColorMap,
    pub style: JerseyStyle,
}

impl Jersey {
    pub fn random() -> Self {
        let color = ColorMap::random();
        let style: JerseyStyle = JerseyStyle::random();
        Self { color, style }
    }
}
