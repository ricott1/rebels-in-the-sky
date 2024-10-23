use image::Rgb;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
pub struct ColorMap {
    pub red: Rgb<u8>,
    pub green: Rgb<u8>,
    pub blue: Rgb<u8>,
}
impl Default for ColorMap {
    fn default() -> Self {
        Self {
            red: Rgb([255, 0, 0]),
            green: Rgb([0, 255, 0]),
            blue: Rgb([0, 0, 255]),
        }
    }
}
impl Serialize for ColorMap {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as serde::Serializer>::Ok, <S as serde::Serializer>::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.hex_format())
    }
}

impl<'de> Deserialize<'de> for ColorMap {
    fn deserialize<D>(deserializer: D) -> Result<ColorMap, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let r_red = u8::from_str_radix(&s[0..2], 16).unwrap_or_default();
        let r_green = u8::from_str_radix(&s[2..4], 16).unwrap_or_default();
        let r_blue = u8::from_str_radix(&s[4..6], 16).unwrap_or_default();
        let g_red = u8::from_str_radix(&s[6..8], 16).unwrap_or_default();
        let g_green = u8::from_str_radix(&s[8..10], 16).unwrap_or_default();
        let g_blue = u8::from_str_radix(&s[10..12], 16).unwrap_or_default();
        let b_red = u8::from_str_radix(&s[12..14], 16).unwrap_or_default();
        let b_green = u8::from_str_radix(&s[14..16], 16).unwrap_or_default();
        let b_blue = u8::from_str_radix(&s[16..18], 16).unwrap_or_default();
        Ok(ColorMap {
            red: Rgb([r_red, r_green, r_blue]),
            green: Rgb([g_red, g_green, g_blue]),
            blue: Rgb([b_red, b_green, b_blue]),
        })
    }
}

impl ColorMap {
    pub fn random_color() -> Rgb<u8> {
        Rgb([rand::random(), rand::random(), rand::random()])
    }
    pub fn random() -> Self {
        let mut rng = ChaCha8Rng::from_entropy();
        let mut color_presets = ColorPreset::iter().collect::<Vec<_>>();
        color_presets.shuffle(&mut rng);
        Self {
            red: color_presets[0].to_rgb(),
            green: color_presets[1].to_rgb(),
            blue: color_presets[2].to_rgb(),
        }
    }

    pub fn hex_format(&self) -> String {
        format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.red[0],
            self.red[1],
            self.red[2],
            self.green[0],
            self.green[1],
            self.green[2],
            self.blue[0],
            self.blue[1],
            self.blue[2],
        )
    }
}

#[derive(Debug, Clone, Copy, Default, EnumIter, PartialEq, Hash)]
#[repr(u8)]
pub enum ColorPreset {
    #[default]
    Red,
    Orange,
    Yellow,
    Lime,
    Green,
    Teal,
    Cyan,
    Turquoise,
    SkyBlue,
    Blue,
    Navy,
    Purple,
    Magenta,
    Pink,
    Brown,
    Maroon,
    Olive,
    Gold,
    Silver,
    Gray,
}

impl ColorPreset {
    pub fn next(&self) -> Self {
        match self {
            Self::Red => Self::Orange,
            Self::Orange => Self::Yellow,
            Self::Yellow => Self::Lime,
            Self::Lime => Self::Green,
            Self::Green => Self::Teal,
            Self::Teal => Self::Cyan,
            Self::Cyan => Self::Turquoise,
            Self::Turquoise => Self::SkyBlue,
            Self::SkyBlue => Self::Blue,
            Self::Blue => Self::Navy,
            Self::Navy => Self::Purple,
            Self::Purple => Self::Magenta,
            Self::Magenta => Self::Pink,
            Self::Pink => Self::Brown,
            Self::Brown => Self::Maroon,
            Self::Maroon => Self::Olive,
            Self::Olive => Self::Gold,
            Self::Gold => Self::Silver,
            Self::Silver => Self::Gray,
            Self::Gray => Self::Red,
        }
    }

    pub fn random() -> Self {
        let rng = &mut ChaCha8Rng::from_entropy();
        match rng.gen_range(0..19) {
            0 => Self::Red,
            1 => Self::Green,
            2 => Self::Blue,
            3 => Self::Yellow,
            4 => Self::Orange,
            5 => Self::Purple,
            6 => Self::Pink,
            7 => Self::Cyan,
            8 => Self::Magenta,
            9 => Self::Lime,
            10 => Self::Teal,
            11 => Self::Olive,
            12 => Self::Maroon,
            13 => Self::Navy,
            14 => Self::Silver,
            15 => Self::Gray,
            16 => Self::Brown,
            17 => Self::SkyBlue,
            18 => Self::Turquoise,
            _ => Self::Gold,
        }
    }

    pub fn to_rgb(&self) -> Rgb<u8> {
        match self {
            ColorPreset::Red => Rgb([200, 50, 50]),
            ColorPreset::Orange => Rgb([200, 120, 50]),
            ColorPreset::Yellow => Rgb([200, 200, 50]),
            ColorPreset::Lime => Rgb([50, 200, 50]),
            ColorPreset::Green => Rgb([20, 210, 30]),
            ColorPreset::Teal => Rgb([50, 100, 100]),
            ColorPreset::Cyan => Rgb([50, 200, 200]),
            ColorPreset::Turquoise => Rgb([50, 180, 170]),
            ColorPreset::SkyBlue => Rgb([100, 170, 200]),
            ColorPreset::Blue => Rgb([50, 50, 200]),
            ColorPreset::Navy => Rgb([50, 50, 100]),
            ColorPreset::Purple => Rgb([120, 50, 120]),
            ColorPreset::Magenta => Rgb([200, 50, 200]),
            ColorPreset::Pink => Rgb([200, 150, 160]),
            ColorPreset::Brown => Rgb([130, 70, 70]),
            ColorPreset::Maroon => Rgb([100, 50, 50]),
            ColorPreset::Olive => Rgb([100, 100, 50]),
            ColorPreset::Gold => Rgb([220, 190, 80]),
            ColorPreset::Silver => Rgb([160, 160, 160]),
            ColorPreset::Gray => Rgb([100, 100, 100]),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum SkinColorMap {
    Pale,
    Light,
    Medium,
    Dark,
    LightGreen,
    Green,
    LightRed,
    Red,
    LightBlue,
    Blue,
    LightPurple,
    Purple,
    LightYellow,
    Yellow,
    Orange,
    Rainbow,
}

impl SkinColorMap {
    pub fn color_map(&self) -> ColorMap {
        match self {
            Self::Pale => ColorMap {
                red: Rgb([234, 194, 190]),
                green: Rgb([237, 220, 213]),
                blue: Rgb([120, 120, 205]),
            },
            Self::Light => ColorMap {
                red: Rgb([183, 167, 138]),
                green: Rgb([225, 198, 182]),
                blue: Rgb([80, 122, 80]),
            },
            Self::Medium => ColorMap {
                red: Rgb([173, 137, 108]),
                green: Rgb([194, 161, 138]),
                blue: Rgb([90, 110, 205]),
            },
            Self::Dark => ColorMap {
                red: Rgb([106, 58, 48]),
                green: Rgb([168, 118, 83]),
                blue: Rgb([180, 180, 210]),
            },
            Self::LightGreen => ColorMap {
                red: Rgb([29, 178, 5]),
                green: Rgb([139, 216, 109]),
                blue: Rgb([30, 90, 125]),
            },
            Self::Green => ColorMap {
                red: Rgb([31, 84, 41]),
                green: Rgb([124, 167, 65]),
                blue: Rgb([30, 70, 105]),
            },
            Self::Red => ColorMap {
                red: Rgb([135, 8, 0]),
                green: Rgb([210, 49, 41]),
                blue: Rgb([10, 20, 30]),
            },
            Self::LightRed => ColorMap {
                red: Rgb([196, 49, 41]),
                green: Rgb([220, 89, 81]),
                blue: Rgb([10, 20, 30]),
            },
            Self::LightBlue => ColorMap {
                red: Rgb([90, 136, 186]),
                green: Rgb([78, 228, 251]),
                blue: Rgb([10, 10, 205]),
            },
            Self::Blue => ColorMap {
                red: Rgb([40, 36, 86]),
                green: Rgb([59, 48, 135]),
                blue: Rgb([10, 10, 205]),
            },
            Self::LightPurple => ColorMap {
                red: Rgb([205, 167, 203]),
                green: Rgb([234, 170, 205]),
                blue: Rgb([128, 63, 154]),
            },
            Self::Purple => ColorMap {
                red: Rgb([88, 33, 134]),
                green: Rgb([88, 137, 253]),
                blue: Rgb([204, 170, 205]),
            },
            Self::LightYellow => ColorMap {
                red: Rgb([226, 217, 0]),
                green: Rgb([230, 230, 144]),
                blue: Rgb([131, 141, 131]),
            },
            Self::Yellow => ColorMap {
                red: Rgb([203, 188, 55]),
                green: Rgb([241, 235, 75]),
                blue: Rgb([151, 151, 181]),
            },
            Self::Orange => ColorMap {
                red: Rgb([255, 194, 38]),
                green: Rgb([250, 227, 115]),
                blue: Rgb([88, 88, 88]),
            },
            Self::Rainbow => ColorMap {
                red: Rgb([245, 131, 48]),
                green: Rgb([48, 245, 209]),
                blue: Rgb([231, 189, 255]),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum HairColorMap {
    Black,
    Blonde,
    BlondeInverted,
    Brown,
    Orange,
    OrangeInverted,
    White,
    Brizzolato,
    Blue,
}

impl HairColorMap {
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        match rng.gen_range(0..9) {
            0 => Self::Black,
            1 => Self::Blonde,
            2 => Self::BlondeInverted,
            3 => Self::Brown,
            4 => Self::Orange,
            5 => Self::OrangeInverted,
            6 => Self::White,
            7 => Self::Brizzolato,
            _ => Self::Blue,
        }
    }
    pub fn color_map(&self) -> ColorMap {
        match self {
            Self::Black => ColorMap {
                red: Rgb([0, 0, 6]),
                green: Rgb([59, 48, 35]),
                blue: Rgb([0, 0, 0]),
            },
            Self::Blonde => ColorMap {
                red: Rgb([184, 151, 120]),
                green: Rgb([230, 228, 196]),
                blue: Rgb([120, 80, 110]),
            },
            Self::BlondeInverted => ColorMap {
                red: Rgb([220, 208, 186]),
                green: Rgb([120, 80, 110]),
                blue: Rgb([184, 151, 140]),
            },
            Self::Brown => ColorMap {
                red: Rgb([145, 85, 61]),
                green: Rgb([165, 137, 70]),
                blue: Rgb([120, 80, 110]),
            },
            Self::Orange => ColorMap {
                red: Rgb([222, 137, 75]),
                green: Rgb([111, 110, 138]),
                blue: Rgb([120, 80, 110]),
            },
            Self::OrangeInverted => ColorMap {
                red: Rgb([141, 110, 138]),
                green: Rgb([120, 80, 110]),
                blue: Rgb([232, 137, 75]),
            },
            Self::White => ColorMap {
                red: Rgb([255, 244, 187]),
                green: Rgb([255, 255, 255]),
                blue: Rgb([181, 181, 181]),
            },
            Self::Brizzolato => ColorMap {
                red: Rgb([57, 55, 44]),
                green: Rgb([255, 255, 255]),
                blue: Rgb([181, 181, 181]),
            },
            Self::Blue => ColorMap {
                red: Rgb([0, 222, 212]),
                green: Rgb([30, 194, 232]),
                blue: Rgb([19, 74, 180]),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum AsteroidColorMap {
    Base,
}

impl AsteroidColorMap {
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        match rng.gen_range(0..9) {
            0 => Self::Base,
            _ => Self::Base,
        }
    }
    pub fn color_map(&self) -> ColorMap {
        match self {
            Self::Base => ColorMap {
                red: Rgb([163, 167, 194]),
                green: Rgb([76, 104, 133]),
                blue: Rgb([58, 63, 94]),
            },
        }
    }
}
