use crate::image::types::{Gif, GifFrame};
use std::fmt::Debug;

pub type Vector2D = [f64; 2];

pub trait Body {
    fn position(&self) -> Vector2D;
    fn set_position(&mut self, position: Vector2D);
    fn velocity(&self) -> Vector2D;
    fn set_velocity(&mut self, velocity: Vector2D);
    fn accelleration(&self) -> Vector2D;
    fn set_accelleration(&mut self, accelleration: Vector2D);
}

pub trait SpaceGif {
    fn gif(&self) -> Gif;
    fn gif_frame(&self, idx: usize) -> GifFrame;
    fn size(&self, idx: usize) -> [u32; 2] {
        let f = self.gif_frame(idx);
        [f.width(), f.height()]
    }
}

pub trait Entity: Body + SpaceGif + Debug + Send + Sync {}
