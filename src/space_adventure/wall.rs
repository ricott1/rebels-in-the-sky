use super::traits::*;
use image::RgbaImage;

#[derive(Default, Debug, Clone)]
pub struct Wall {
    id: usize,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    image: RgbaImage,
}

impl Body for Wall {
    fn is_collider(&self) -> bool {
        true
    }

    fn bounds(&self) -> (Vector2D, Vector2D) {
        (
            [self.x, self.y],
            [self.x + self.width as i16, self.y + self.height as i16],
        )
    }

    fn position(&self) -> Vector2D {
        [self.x, self.y]
    }
}

impl Sprite for Wall {
    fn image(&self) -> &RgbaImage {
        &self.image
    }
}

impl Entity for Wall {
    fn set_id(&mut self, id: usize) {
        self.id = id;
    }
    fn id(&self) -> usize {
        self.id
    }
}

impl Wall {
    pub fn new(x: i16, y: i16, width: u16, height: u16) -> Self {
        Self {
            id: 0,
            x,
            y,
            width,
            height,
            image: RgbaImage::new(0, 0),
        }
    }
}
