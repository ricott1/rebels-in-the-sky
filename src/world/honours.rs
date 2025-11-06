use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::{Display, EnumIter};

use crate::{
    types::{SystemTimeTick, Tick},
    world::{Team, LIGHT_YEAR, SATOSHI_PER_BITCOIN, WEEKS},
};

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize_repr, Deserialize_repr, EnumIter, Display,
)]
#[repr(u8)]
pub enum Honour {
    Maximalist,
    MultiKulti,
    Traveller,
    Veteran,
}

impl Honour {
    pub fn conditions_met(self, team: &Team) -> bool {
        match self {
            Self::Maximalist => team.balance() >= 1 * SATOSHI_PER_BITCOIN,
            Self::MultiKulti => false, // FIXME: pass world to check this
            Self::Traveller => team.total_travelled >= 1 * LIGHT_YEAR,
            Self::Veteran => {
                team.creation_time != Tick::default()
                    && (Tick::now() - team.creation_time) > 52 * WEEKS
            }
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Maximalist => "Held at least 1 BTC at some point in time.",
            Self::MultiKulti => "Have pirates from 7 different populations in the crew.",
            Self::Traveller => "Travel through the galaxy for at least 1 light year.",
            Self::Veteran => "Played for a year.",
        }
        .to_string()
    }

    pub fn symbol(&self) -> char {
        match self {
            Self::Maximalist => 'M',
            Self::MultiKulti => 'K',
            Self::Traveller => 'T',
            Self::Veteran => 'V',
        }
    }
}
