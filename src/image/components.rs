use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::EnumIter;
use strum_macros::Display;

use crate::world::spaceship::{Engine, Hull};

pub trait ImageComponent {
    fn select_file(&self, size: u8) -> String;
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum BeardImage {
    Beard1,
    Beard2,
    Beard3,
    Beard4,
    Beard5,
}

impl ImageComponent for BeardImage {
    fn select_file(&self, _size: u8) -> String {
        format!("beard/{}.png", self.to_string().to_lowercase())
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq, EnumIter)]
#[repr(u8)]
pub enum HairImage {
    Hair1,
    Hair2,
    Hair3,
    Hair4,
    Hair5,
    Hair6,
    Hair7,
    Hair8,
    Hair9,
    Hair10,
}

impl ImageComponent for HairImage {
    fn select_file(&self, _size: u8) -> String {
        format!("hair/{}.png", self.to_string().to_lowercase())
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum HeadImage {
    Devil1,
    Devil2,
    Elf1,
    Elf2,
    Human1,
    Human2,
    Orc1,
    Orc2,
    Gald1,
    Gald2,
}

impl ImageComponent for HeadImage {
    fn select_file(&self, _size: u8) -> String {
        format!("head/{}.png", self.to_string().to_lowercase())
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum BodyImage {
    Devil,
    Normal,
}

impl ImageComponent for BodyImage {
    fn select_file(&self, size: u8) -> String {
        let number = match size {
            x if x <= 2 => 0,
            x if x <= 4 => 3,
            x if x <= 6 => 5,
            x if x <= 9 => 7,
            x if x <= 11 => 10,
            _ => 12,
        };

        format!("body/{}{}.png", self.to_string().to_lowercase(), number)
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum LegsImage {
    Devil,
    Normal,
}

impl ImageComponent for LegsImage {
    fn select_file(&self, size: u8) -> String {
        let number = match size {
            0 => 0,
            1 => 1,
            x if x <= 3 => 2,
            x if x <= 5 => 4,
            6 => 6,
            7 => 7,
            8 => 8,
            x if x <= 10 => 9,
            x if x <= 12 => 11,
            _ => 13,
        };

        format!("legs/{}{}.png", self.to_string().to_lowercase(), number)
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum ShirtImage {
    Classic,
    Fancy,
    Gilet,
    Stripe,
    Pirate,
    PirateAlt,
    Mask,
}

impl ImageComponent for ShirtImage {
    fn select_file(&self, size: u8) -> String {
        let number = match size {
            x if x <= 2 => 0,
            x if x <= 4 => 3,
            x if x <= 6 => 5,
            x if x <= 9 => 7,
            x if x <= 11 => 10,
            _ => 12,
        };

        format!("shirt/{}{}.png", self.to_string().to_lowercase(), number)
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum ShortsImage {
    Classic,
    Fancy,
    Gilet,
    Stripe,
    Pirate,
    PirateAlt,
    Mask,
}

impl ImageComponent for ShortsImage {
    fn select_file(&self, size: u8) -> String {
        let number = match size {
            x if x <= 2 => 0,
            x if x <= 6 => 3,
            x if x <= 8 => 7,
            x if x <= 9 => 9,
            _ => 10,
        };

        format!("shorts/{}{}.png", self.to_string().to_lowercase(), number)
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum ShoesImage {
    Classic,
}

impl ImageComponent for ShoesImage {
    fn select_file(&self, size: u8) -> String {
        let number = match size {
            x if x < 7 => 0,
            _ => 7,
        };

        format!("shoes/{}{}.png", self.to_string().to_lowercase(), number)
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum HatImage {
    Bandana,
    Infernal,
    Classic,
    Straw,
    Mask,
    MaskYardalaim,
    MaskPolpett,
    MaskGaldari,
}

impl ImageComponent for HatImage {
    fn select_file(&self, _size: u8) -> String {
        format!("hat/{}.png", self.to_string().to_lowercase())
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum WoodenLegImage {
    Left,
    Right,
}

impl ImageComponent for WoodenLegImage {
    fn select_file(&self, size: u8) -> String {
        match size {
            x if x < 7 => "wooden_leg/slim.png".into(),
            _ => "wooden_leg/large.png".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum EyePatchImage {
    LeftLow,
    RightLow,
    LeftHigh,
    RightHigh,
    Central,
}

impl ImageComponent for EyePatchImage {
    fn select_file(&self, _size: u8) -> String {
        match self {
            EyePatchImage::LeftLow => "accessories/eye_patch_left_low.png".into(),
            EyePatchImage::RightLow => "accessories/eye_patch_right_low.png".into(),
            EyePatchImage::LeftHigh => "accessories/eye_patch_left_high.png".into(),
            EyePatchImage::RightHigh => "accessories/eye_patch_right_high.png".into(),
            EyePatchImage::Central => "accessories/eye_patch_central.png".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Display, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum HookImage {
    Left,
    Right,
}

impl ImageComponent for HookImage {
    fn select_file(&self, _size: u8) -> String {
        match self {
            HookImage::Left => "accessories/hook_left.png".into(),
            HookImage::Right => "accessories/hook_right.png".into(),
        }
    }
}

impl ImageComponent for Hull {
    fn select_file(&self, _size: u8) -> String {
        match self {
            Hull::ShuttleSmall => "hull/shuttle_small.png".into(),
            Hull::ShuttleStandard => "hull/shuttle_standard.png".into(),
            Hull::ShuttleLarge => "hull/shuttle_large.png".into(),
            Hull::PincherStandard => "hull/pincher_standard.png".into(),
            Hull::PincherLarge => "hull/pincher_large.png".into(),
        }
    }
}

impl ImageComponent for Engine {
    fn select_file(&self, _size: u8) -> String {
        match self {
            Engine::ShuttleSingle => "engine/shuttle_single.png".into(),
            Engine::ShuttleDouble => "engine/shuttle_double.png".into(),
            Engine::ShuttleTriple => "engine/shuttle_triple.png".into(),
            Engine::PincherSingle => "engine/pincher_single.png".into(),
            Engine::PincherDouble => "engine/pincher_double.png".into(),
            Engine::PincherTriple => "engine/pincher_triple.png".into(),
        }
    }
}
