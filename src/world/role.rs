use serde_repr::{Deserialize_repr, Serialize_repr};
use strum_macros::{Display, EnumIter};

#[derive(
    Debug, Default, PartialEq, Clone, Copy, Display, EnumIter, Serialize_repr, Deserialize_repr,
)]
#[repr(u8)]
pub enum CrewRole {
    Captain,
    Doctor,
    Pilot,
    #[default]
    Mozzo,
    // Chef,
    // Engineer,
}
