use super::collector::CollectorEntity;
use super::shield::ShieldEntity;
use super::{
    asteroid::AsteroidEntity, fragment::FragmentEntity, particle::ParticleEntity,
    projectile::ProjectileEntity, traits::*, SpaceshipEntity,
};
use super::{collisions::HitBox, space_callback::SpaceCallback, visual_effects::VisualEffect};
use crate::types::AppResult;
use anyhow::anyhow;
use glam::I16Vec2;
use image::RgbaImage;
use std::fmt::Debug;
use strum::Display;

#[derive(Debug, Display)]
#[allow(clippy::large_enum_variant)]
pub enum Entity {
    Asteroid(AsteroidEntity),
    Collector(CollectorEntity),
    Fragment(FragmentEntity),
    Particle(ParticleEntity),
    Projectile(ProjectileEntity),
    Shield(ShieldEntity),
    Spaceship(SpaceshipEntity),
}

macro_rules! delegate {
    ($self:expr, $method:ident ( $($args:expr),* )) => {
        match $self {
            Self::Asteroid(e)   => e.$method($($args),*),
            Self::Collector(e)   => e.$method($($args),*),
            Self::Fragment(e)   => e.$method($($args),*),
            Self::Particle(e)   => e.$method($($args),*),
            Self::Projectile(e) => e.$method($($args),*),
            Self::Shield(e)  => e.$method($($args),*),
            Self::Spaceship(e)  => e.$method($($args),*),
        }
    };
}

macro_rules! delegate_mut {
    ($self:expr, $method:ident ( $($args:expr),* )) => {
        match $self {
            Self::Asteroid(e)   => e.$method($($args),*),
            Self::Collector(e)   => e.$method($($args),*),
            Self::Fragment(e)   => e.$method($($args),*),
            Self::Particle(e)   => e.$method($($args),*),
            Self::Projectile(e) => e.$method($($args),*),
            Self::Shield(e)  => e.$method($($args),*),
            Self::Spaceship(e)  => e.$method($($args),*),
        }
    };
}
impl Body for Entity {
    fn center(&self) -> glam::I16Vec2 {
        delegate!(self, center())
    }

    fn position(&self) -> glam::I16Vec2 {
        delegate!(self, position())
    }

    fn previous_position(&self) -> glam::I16Vec2 {
        delegate!(self, previous_position())
    }

    fn previous_rect(&self) -> (glam::I16Vec2, glam::I16Vec2) {
        delegate!(self, previous_rect())
    }

    fn rect(&self) -> (glam::I16Vec2, glam::I16Vec2) {
        delegate!(self, rect())
    }

    fn update_body(&mut self, deltatime: f32) -> Vec<super::SpaceCallback> {
        delegate_mut!(self, update_body(deltatime))
    }

    fn velocity(&self) -> glam::I16Vec2 {
        delegate!(self, velocity())
    }
}

impl Sprite for Entity {
    fn image(&self) -> &RgbaImage {
        delegate!(self, image())
    }

    fn should_apply_visual_effects(&self) -> bool {
        delegate!(self, should_apply_visual_effects())
    }

    fn apply_visual_effects<'a>(&'a self, image: &'a RgbaImage) -> RgbaImage {
        delegate!(self, apply_visual_effects(image))
    }

    fn add_visual_effect(&mut self, duration: f32, effect: VisualEffect) {
        delegate_mut!(self, add_visual_effect(duration, effect))
    }

    fn remove_visual_effect(&mut self, effect: &VisualEffect) {
        delegate_mut!(self, remove_visual_effect(effect))
    }

    fn update_sprite(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        delegate_mut!(self, update_sprite(deltatime))
    }
}

impl Collider for Entity {
    fn collision_damage(&self) -> f32 {
        delegate!(self, collision_damage())
    }

    fn collider_type(&self) -> ColliderType {
        delegate!(self, collider_type())
    }

    fn hit_box(&self) -> &HitBox {
        delegate!(self, hit_box())
    }

    fn size(&self) -> I16Vec2 {
        delegate!(self, size())
    }
}

impl GameEntity for Entity {
    fn set_id(&mut self, id: usize) {
        delegate_mut!(self, set_id(id))
    }

    fn id(&self) -> usize {
        delegate!(self, id())
    }

    fn layer(&self) -> usize {
        delegate!(self, layer())
    }

    fn update(&mut self, deltatime: f32) -> Vec<SpaceCallback> {
        delegate_mut!(self, update(deltatime))
    }

    fn handle_space_callback(&mut self, callback: SpaceCallback) -> Vec<SpaceCallback> {
        delegate_mut!(self, handle_space_callback(callback))
    }
}

impl Entity {
    pub fn as_collector(&self) -> AppResult<&CollectorEntity> {
        match self {
            Entity::Collector(inner) => Ok(inner),
            _ => Err(anyhow!("Cannot convert {self} to CollectorEntity")),
        }
    }

    pub fn as_collector_mut(&mut self) -> AppResult<&mut CollectorEntity> {
        match self {
            Entity::Collector(inner) => Ok(inner),
            _ => Err(anyhow!("Cannot convert {self} to CollectorEntity")),
        }
    }

    pub fn as_fragment(&self) -> AppResult<&FragmentEntity> {
        match self {
            Entity::Fragment(inner) => Ok(inner),
            _ => Err(anyhow!("Cannot convert {self} to FragmentEntity")),
        }
    }

    pub fn as_fragment_mut(&mut self) -> AppResult<&mut FragmentEntity> {
        match self {
            Entity::Fragment(inner) => Ok(inner),
            _ => Err(anyhow!("Cannot convert {self} to FragmentEntity")),
        }
    }

    pub fn as_shield(&self) -> AppResult<&ShieldEntity> {
        match self {
            Entity::Shield(inner) => Ok(inner),
            _ => Err(anyhow!("Cannot convert {self} to ShieldEntity")),
        }
    }

    pub fn as_shield_mut(&mut self) -> AppResult<&mut ShieldEntity> {
        match self {
            Entity::Shield(inner) => Ok(inner),
            _ => Err(anyhow!("Cannot convert {self} to ShieldEntity")),
        }
    }

    pub fn as_spaceship(&self) -> AppResult<&SpaceshipEntity> {
        match self {
            Entity::Spaceship(inner) => Ok(inner),
            _ => Err(anyhow!("Cannot convert {self} to SpaceshipEntity")),
        }
    }

    pub fn as_spaceship_mut(&mut self) -> AppResult<&mut SpaceshipEntity> {
        match self {
            Entity::Spaceship(inner) => Ok(inner),
            _ => Err(anyhow!("Cannot convert {self} to SpaceshipEntity")),
        }
    }
}
