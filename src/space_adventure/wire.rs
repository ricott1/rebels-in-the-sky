use super::{
    asteroid::AsteroidSize,
    player::{LocalPlayerView, PlayerOutcome, ShipLoadout},
    visual_effects::VisualEffect,
    PlayerInput, SpaceshipRole,
};
use crate::{
    core::{resources::Resource, spaceship::Spaceship},
    types::{AppResult, PlanetId, TeamId},
};
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub const MAX_FRAME_BYTES: usize = 1 << 20;

pub type NetId = u32;

pub mod json_bytes {
    use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T: Serialize, S: Serializer>(
        value: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let bytes = serde_json::to_vec(value).map_err(serde::ser::Error::custom)?;
        bytes.serialize(serializer)
    }

    pub fn deserialize<'de, T: DeserializeOwned, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        serde_json::from_slice(&bytes).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NetVec(pub i16, pub i16);

impl NetVec {
    const SCALE: f32 = 16.0;

    fn quantize(value: f32) -> i16 {
        (value * Self::SCALE)
            .round()
            .clamp(i16::MIN as f32, i16::MAX as f32) as i16
    }

    pub fn from_vec2(value: Vec2) -> Self {
        Self(Self::quantize(value.x), Self::quantize(value.y))
    }

    pub fn to_vec2(self) -> Vec2 {
        Vec2::new(self.0 as f32, self.1 as f32) / Self::SCALE
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpawnKind {
    Asteroid {
        size: AsteroidSize,
    },
    Spaceship {
        #[serde(with = "json_bytes")]
        spaceship: Spaceship,
        role: SpaceshipRole,
    },
    Fragment {
        resource: Resource,
        amount: u32,
    },
    Projectile {
        color: [u8; 4],
    },
    Shield,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySpawn {
    pub id: NetId,
    pub kind: SpawnKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShieldLook {
    Active,
    Recharging,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EntityLook {
    pub frame: u16,
    pub shield: Option<ShieldLook>,
    pub effects: Vec<(VisualEffect, f32)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetEntityState {
    pub id: NetId,
    pub pos: NetVec,
    pub vel: NetVec,
    pub look: EntityLook,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleSpawn {
    pub pos: NetVec,
    pub vel: NetVec,
    pub color: [u8; 4],
    pub lifetime: Option<f32>,
    pub layer: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoinRequest {
    pub version: [usize; 3],
    pub team_id: TeamId,
    pub team_name: String,
    pub planet_id: PlanetId,
    pub loadout: ShipLoadout,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Welcome {
    pub your_ship_id: NetId,
    pub spawns: Vec<EntitySpawn>,
    pub states: Vec<NetEntityState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub last_input_seq: u64,
    pub spawned: Vec<EntitySpawn>,
    pub removed: Vec<NetId>,
    pub states: Vec<NetEntityState>,
    pub particles: Vec<ParticleSpawn>,
    pub you: LocalPlayerView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    Closed,
    NotRunning,
    Full,
    VersionMismatch,
    NotOnPlanet,
    CantReachHost,
}

impl RejectReason {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Closed => "The adventure is closed",
            Self::NotRunning => "The adventure is not running",
            Self::Full => "The adventure is full",
            Self::VersionMismatch => "Game versions do not match",
            Self::NotOnPlanet => "You are not around the same planet",
            Self::CantReachHost => "Can't reach host",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndReason {
    HostEnded,
    Destroyed,
    LinkLost,
}

impl EndReason {
    pub fn message(&self) -> &'static str {
        match self {
            Self::HostEnded => {
                "The host went back to the base.\nYour crew follows with the cargo hold."
            }
            Self::Destroyed => super::HULL_BREACH_MESSAGE,
            Self::LinkLost => {
                "Lost contact with the host.\nYour crew goes back to the base with the cargo hold."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionMessage {
    Join(JoinRequest),
    Welcome(Welcome),
    Reject {
        reason: RejectReason,
    },
    Snapshot(Snapshot),
    Input {
        seq: u64,
        input: PlayerInput,
    },
    Heartbeat,
    Leave,
    Ended {
        reason: EndReason,
        outcome: PlayerOutcome,
    },
}

pub fn encode(message: &SessionMessage) -> AppResult<Vec<u8>> {
    Ok(postcard::to_stdvec(message)?)
}

pub fn decode(bytes: &[u8]) -> AppResult<SessionMessage> {
    Ok(postcard::from_bytes(bytes)?)
}

pub trait LinkSender: Debug + Send {
    fn send_control(&self, message: SessionMessage) -> bool;
    fn try_send_snapshot(&self, message: SessionMessage) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ResourceMap;
    use crate::core::spaceship::SpaceshipPrefab;
    use crate::space_adventure::ShipLoadout;
    use strum::IntoEnumIterator;

    fn round_trip(message: SessionMessage) -> AppResult<()> {
        let bytes = encode(&message)?;
        assert_eq!(decode(&bytes)?, message);
        Ok(())
    }

    #[test]
    fn test_every_message_round_trips() -> AppResult<()> {
        let view = LocalPlayerView::default();
        let outcome = PlayerOutcome {
            resources: ResourceMap::new(),
            durability: 3,
        };
        let state = NetEntityState {
            id: 7,
            pos: NetVec::from_vec2(Vec2::new(10.5, -3.25)),
            vel: NetVec::from_vec2(Vec2::new(-12.5, 0.0625)),
            look: EntityLook {
                frame: 3,
                shield: Some(ShieldLook::Recharging),
                effects: vec![(VisualEffect::ColorMask { color: [255, 0, 0] }, 1.5)],
            },
        };
        let spawns = vec![
            EntitySpawn {
                id: 1,
                kind: SpawnKind::Asteroid {
                    size: AsteroidSize::Planet,
                },
            },
            EntitySpawn {
                id: 2,
                kind: SpawnKind::Spaceship {
                    spaceship: SpaceshipPrefab::Ibarruri.spaceship(),
                    role: SpaceshipRole::Enemy,
                },
            },
            EntitySpawn {
                id: 3,
                kind: SpawnKind::Fragment {
                    resource: Resource::GOLD,
                    amount: 2,
                },
            },
            EntitySpawn {
                id: 4,
                kind: SpawnKind::Projectile {
                    color: [1, 2, 3, 4],
                },
            },
            EntitySpawn {
                id: 5,
                kind: SpawnKind::Shield,
            },
        ];

        round_trip(SessionMessage::Join(JoinRequest {
            version: crate::app_version(),
            team_id: TeamId::new_v4(),
            team_name: "Era Destino".to_string(),
            planet_id: PlanetId::new_v4(),
            loadout: ShipLoadout::test_default(),
        }))?;
        round_trip(SessionMessage::Welcome(Welcome {
            your_ship_id: 9,
            spawns: spawns.clone(),
            states: vec![state.clone()],
        }))?;
        round_trip(SessionMessage::Reject {
            reason: RejectReason::Full,
        })?;
        round_trip(SessionMessage::Snapshot(Snapshot {
            tick: 4,
            last_input_seq: 11,
            spawned: spawns,
            removed: vec![8],
            states: vec![state],
            particles: vec![ParticleSpawn {
                pos: NetVec(16, 32),
                vel: NetVec(-16, 0),
                color: [9, 9, 9, 255],
                lifetime: Some(1.5),
                layer: 2,
            }],
            you: view,
        }))?;
        round_trip(SessionMessage::Input {
            seq: 12,
            input: PlayerInput::Shoot,
        })?;
        round_trip(SessionMessage::Heartbeat)?;
        round_trip(SessionMessage::Leave)?;
        round_trip(SessionMessage::Ended {
            reason: EndReason::Destroyed,
            outcome,
        })?;
        Ok(())
    }

    #[test]
    fn test_every_spaceship_prefab_round_trips() -> AppResult<()> {
        for prefab in SpaceshipPrefab::iter() {
            let mut loadout = ShipLoadout::test_default();
            loadout.spaceship = prefab.spaceship();
            round_trip(SessionMessage::Join(JoinRequest {
                version: crate::app_version(),
                team_id: TeamId::new_v4(),
                team_name: String::new(),
                planet_id: PlanetId::new_v4(),
                loadout,
            }))?;
        }
        Ok(())
    }

    #[test]
    fn test_net_vec_quantizes_to_sixteenths() {
        let v = NetVec::from_vec2(Vec2::new(1.03, -2.5));
        assert_eq!(v, NetVec(16, -40));
        assert_eq!(v.to_vec2(), Vec2::new(1.0, -2.5));
        assert_eq!(
            NetVec::from_vec2(Vec2::new(1e9, -1e9)),
            NetVec(i16::MAX, i16::MIN)
        );
    }

    #[test]
    fn test_garbage_does_not_decode() {
        assert!(decode(&[0xff, 0xff, 0xff]).is_err());
    }
}
