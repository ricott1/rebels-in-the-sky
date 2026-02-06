use super::color_map::ColorMap;
use crate::store::ASSETS_DIR;
use crate::types::AppResult;
use anyhow::anyhow;
use image::error::{ParameterError, ParameterErrorKind};
use image::{ImageBuffer, ImageError, ImageReader, ImageResult, Rgb, Rgba, RgbaImage};
use std::sync::LazyLock;
use std::io::Cursor;

pub type Gif = Vec<RgbaImage>;

pub static UNIVERSE_BACKGROUND: LazyLock<RgbaImage> = LazyLock::new(|| {
    fn try_blit(mut background: RgbaImage) -> AppResult<RgbaImage> {
        for star_layer in STAR_LAYERS.iter().take(2) {
            for x_idx in 0..WIDTH_MUL {
                for y_idx in 0..HEIGHT_MUL {
                    background.copy_non_trasparent_from(
                        star_layer,
                        x_idx * star_layer.width(),
                        y_idx * star_layer.height(),
                    )?;
                }
            }
        }

        Ok(background)
    }

    const WIDTH_MUL: u32 = 2;
    const HEIGHT_MUL: u32 = 6;

    let background = RgbaImage::new(
        STAR_LAYERS[0].width() * WIDTH_MUL,
        STAR_LAYERS[0].height() * HEIGHT_MUL,
    );
    try_blit(background).expect("Should blit on background")
});

pub static STAR_LAYERS: LazyLock<[RgbaImage; 3]> = LazyLock::new(|| {
    [
        open_image("universe/star_layer_1.png").expect("Cannot open star_layer_1.png."),
        open_image("universe/star_layer_2.png").expect("Cannot open star_layer_2.png."),
        open_image("universe/star_layer_3.png").expect("Cannot open star_layer_3.png."),
    ]
});

#[derive(Debug)]
pub enum LightMaskStyle {
    Horizontal {
        from_background: Rgb<u8>,
        to_background: Option<Rgb<u8>>,
        from_alpha: u8,
        to_alpha: u8,
    },
    Vertical {
        from_background: Rgb<u8>,
        to_background: Option<Rgb<u8>>,
        from_alpha: u8,
        to_alpha: u8,
    },
    Radial {
        from_background: Rgb<u8>,
        to_background: Option<Rgb<u8>>,
        from_alpha: u8,
        to_alpha: u8,
        center: Option<(u32, u32)>,
    },
    Exponential {
        from_background: Rgb<u8>,
        to_background: Option<Rgb<u8>>,
        from_alpha: u8,
        to_alpha: u8,
        center: Option<(u32, u32)>,
    },
}

impl LightMaskStyle {
    pub fn horizontal() -> Self {
        Self::Horizontal {
            from_background: Rgb([0; 3]),
            to_background: None,
            from_alpha: 165,
            to_alpha: 255,
        }
    }

    pub fn vertical() -> Self {
        Self::Vertical {
            from_background: Rgb([0; 3]),
            to_background: None,
            from_alpha: 255,
            to_alpha: 155,
        }
    }

    pub fn radial() -> Self {
        Self::Radial {
            from_background: Rgb([0; 3]),
            to_background: None,
            from_alpha: 255,
            to_alpha: 155,
            center: None,
        }
    }

    pub fn pointer(center: (u32, u32)) -> Self {
        Self::Exponential {
            from_background: Rgb([0, 255, 0]),
            to_background: None,
            from_alpha: 255,
            to_alpha: 5,
            center: Some(center),
        }
    }

    pub fn star_zoom_out() -> Self {
        Self::Radial {
            from_background: Rgb([0; 3]),
            to_background: None,
            from_alpha: 255,
            to_alpha: 125,
            center: None,
        }
    }

    pub fn black_hole() -> Self {
        Self::Radial {
            from_background: Rgb([0; 3]),
            to_background: None,
            from_alpha: 215,
            to_alpha: 255,
            center: None,
        }
    }

    pub fn player() -> Self {
        Self::Horizontal {
            from_background: Rgb([25, 25, 25]),
            to_background: None,
            from_alpha: 175,
            to_alpha: 255,
        }
    }

    pub fn space_cove() -> Self {
        Self::Horizontal {
            from_background: Rgb([0, 45, 235]),
            to_background: None,
            from_alpha: 255,
            to_alpha: 125,
        }
    }

    pub fn skull_eye(center: (u32, u32)) -> Self {
        Self::Radial {
            from_background: Rgb([235, 45, 0]),
            to_background: None,
            from_alpha: 215,
            to_alpha: 255,
            center: Some(center),
        }
    }

    pub fn mask(&self, width: u32, height: u32) -> RgbaImage {
        fn interpolate_pixel(
            v: f32,
            from_background: Rgb<u8>,
            to_background: Option<Rgb<u8>>,
            from_alpha: u8,
            to_alpha: u8,
        ) -> Rgba<u8> {
            let [mut r, mut g, mut b] = from_background.0;
            let a = (from_alpha as f32 * (1.0 - v) + to_alpha as f32 * v).clamp(0.0, 255.0) as u8;

            if let Some(to) = to_background {
                let [to_r, to_g, to_b] = to.0;
                r = (r as f32 * (1.0 - v) + to_r as f32 * v).clamp(0.0, 255.0) as u8;
                g = (g as f32 * (1.0 - v) + to_g as f32 * v).clamp(0.0, 255.0) as u8;
                b = (b as f32 * (1.0 - v) + to_b as f32 * v).clamp(0.0, 255.0) as u8;
            }

            Rgba::from([r, g, b, a])
        }
        RgbaImage::from_fn(width, height, |x, y| match *self {
            Self::Horizontal {
                from_alpha,
                to_alpha,
                from_background,
                to_background,
            } => {
                let v = x as f32 / width as f32;
                interpolate_pixel(v, from_background, to_background, from_alpha, to_alpha)
            }
            Self::Vertical {
                from_alpha,
                to_alpha,
                from_background,
                to_background,
            } => {
                let v = y as f32 / height as f32;
                interpolate_pixel(v, from_background, to_background, from_alpha, to_alpha)
            }
            Self::Radial {
                from_alpha,
                to_alpha,
                from_background,
                to_background,
                center,
            } => {
                let center = if let Some(pos) = center {
                    pos
                } else {
                    (width / 2, height / 2)
                };

                let v = 4.0
                    * ((x as i32 - center.0 as i32).pow(2) + (y as i32 - center.1 as i32).pow(2))
                        as f32
                    / (width.pow(2) + height.pow(2)) as f32;
                interpolate_pixel(v, from_background, to_background, from_alpha, to_alpha)
            }

            Self::Exponential {
                from_alpha,
                to_alpha,
                from_background,
                to_background,
                center,
            } => {
                let center = if let Some(pos) = center {
                    pos
                } else {
                    (width / 2, height / 2)
                };
                let d = (x as i32 - center.0 as i32).pow(2) + (y as i32 - center.1 as i32).pow(2);
                let v = (-d as f32 / 8.0).exp();
                interpolate_pixel(v, from_background, to_background, from_alpha, to_alpha)
            }
        })
    }
}

pub trait ExtraImageUtils {
    fn copy_non_trasparent_from(&mut self, other: &RgbaImage, x: u32, y: u32) -> ImageResult<()>;
    fn copy_non_transparent_from_clipped(
        &mut self,
        src: &RgbaImage,
        src_x: u32,
        src_y: u32,
        width: u32,
        height: u32,
        dst_x: u32,
        dst_y: u32,
    );
    fn apply_color_map(&mut self, color_map: ColorMap) -> &RgbaImage;

    // Applies a specified shadow mask by reading pixels from mask image
    fn apply_color_map_with_shadow_mask(
        &mut self,
        color_map: ColorMap,
        mask: &RgbaImage,
    ) -> &RgbaImage;

    // Applies global light mask created programmatically
    fn apply_light_mask(&mut self, light_style: &LightMaskStyle) -> &RgbaImage;
}

impl ExtraImageUtils for RgbaImage {
    /// Copies all non-transparent the pixels from another image into this image.
    ///
    /// The other image is copied with the top-left corner of the
    /// other image placed at (x, y).
    ///
    /// In order to copy only a piece of the other image, use [`GenericImageView::view`].
    ///
    /// You can use [`FlatSamples`] to source pixels from an arbitrary regular raster of channel
    /// values, for example from a foreign interface or a fixed image.
    ///
    /// # Returns
    /// Returns an error if the image is too large to be copied at the given position
    ///
    /// [`GenericImageView::view`]: trait.GenericImageView.html#method.view
    /// [`FlatSamples`]: flat/struct.FlatSamples.html
    fn copy_non_trasparent_from(&mut self, other: &RgbaImage, x: u32, y: u32) -> ImageResult<()> {
        // Do bounds checking here so we can use the non-bounds-checking
        // functions to copy pixels.
        if self.width() < other.width() + x || self.height() < other.height() + y {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )));
        }

        for k in 0..other.height() {
            for i in 0..other.width() {
                let src_px = other.get_pixel(i, k);
                let a = src_px[3] as f32 / 255.0;

                if a == 0.0 {
                    continue;
                }

                let dst_px = self.get_pixel(i + x, k + y);

                let blended = Rgba([
                    (src_px[0] as f32 * a + dst_px[0] as f32 * (1.0 - a)) as u8,
                    (src_px[1] as f32 * a + dst_px[1] as f32 * (1.0 - a)) as u8,
                    (src_px[2] as f32 * a + dst_px[2] as f32 * (1.0 - a)) as u8,
                    255,
                ]);

                self.put_pixel(i + x, k + y, blended);
            }
        }
        Ok(())
    }

    fn copy_non_transparent_from_clipped(
        &mut self,
        src: &RgbaImage,
        src_x: u32,
        src_y: u32,
        width: u32,
        height: u32,
        dst_x: u32,
        dst_y: u32,
    ) {
        for y in 0..height {
            for x in 0..width {
                let src_px = src.get_pixel(src_x + x, src_y + y);
                let a = src_px[3] as f32 / 255.0;

                if a == 0.0 {
                    continue;
                }

                let dst_px = self.get_pixel(dst_x + x, dst_y + y);

                let blended = Rgba([
                    (src_px[0] as f32 * a + dst_px[0] as f32 * (1.0 - a)) as u8,
                    (src_px[1] as f32 * a + dst_px[1] as f32 * (1.0 - a)) as u8,
                    (src_px[2] as f32 * a + dst_px[2] as f32 * (1.0 - a)) as u8,
                    255,
                ]);

                self.put_pixel(dst_x + x, dst_y + y, blended);
            }
        }
    }

    fn apply_color_map(&mut self, color_map: ColorMap) -> &RgbaImage {
        for k in 0..self.height() {
            for i in 0..self.width() {
                let p = self.get_pixel(i, k);
                if p[3] > 0 {
                    let mapped_pixel = match *p {
                        _ if p[0] == 255 && p[1] == 0 && p[2] == 0 => {
                            let [r, g, b] = color_map.red.0;
                            Rgba([r, g, b, p[3]])
                        }
                        _ if p[0] == 0 && p[1] == 255 && p[2] == 0 => {
                            let [r, g, b] = color_map.green.0;
                            Rgba([r, g, b, p[3]])
                        }
                        _ if p[0] == 0 && p[1] == 0 && p[2] == 255 => {
                            let [r, g, b] = color_map.blue.0;
                            Rgba([r, g, b, p[3]])
                        }

                        _ => continue,
                    };
                    self.put_pixel(i, k, mapped_pixel);
                }
            }
        }
        self
    }

    fn apply_color_map_with_shadow_mask(
        &mut self,
        color_map: ColorMap,
        mask: &RgbaImage,
    ) -> &RgbaImage {
        for k in 0..self.height() {
            for i in 0..self.width() {
                let p = self.get_pixel(i, k);
                if p[3] > 0 {
                    let mask_p = mask.get_pixel_checked(i, k).unwrap_or_else(|| {
                        log::error!("Failed to get pixel from mask: {color_map:?}");
                        &Rgba([0, 0, 0, 0])
                    });
                    let mapped_pixel = match *p {
                        _ if p[0] == 255 && p[1] == 0 && p[2] == 0 => {
                            let [r, g, b] = color_map.red.0;
                            Rgba([r, g, b, p[3]])
                        }
                        _ if p[0] == 0 && p[1] == 255 && p[2] == 0 => {
                            let [r, g, b] = color_map.green.0;
                            Rgba([r, g, b, p[3]])
                        }
                        _ if p[0] == 0 && p[1] == 0 && p[2] == 255 => {
                            let [r, g, b] = color_map.blue.0;
                            Rgba([r, g, b, p[3]])
                        }

                        _ => *p,
                    };

                    let masked_mapped_pixel =
                        if mask_p[0] == 255 && mask_p[1] == 0 && mask_p[2] == 0 {
                            Rgba([
                                (0.75 * mapped_pixel[0] as f32) as u8,
                                (0.75 * mapped_pixel[1] as f32) as u8,
                                (0.75 * mapped_pixel[2] as f32) as u8,
                                p[3],
                            ])
                        } else if mask_p[0] == 0 && mask_p[1] == 0 && mask_p[2] == 255 {
                            Rgba([
                                (1.25 * mapped_pixel[0] as f32).min(255.0) as u8,
                                (1.25 * mapped_pixel[1] as f32).min(255.0) as u8,
                                (1.25 * mapped_pixel[2] as f32).min(255.0) as u8,
                                p[3],
                            ])
                        } else {
                            mapped_pixel
                        };

                    self.put_pixel(i, k, masked_mapped_pixel);
                }
            }
        }
        self
    }

    fn apply_light_mask(&mut self, light_style: &LightMaskStyle) -> &RgbaImage {
        let mask = light_style.mask(self.width(), self.height());
        for y in 0..mask.height() {
            for x in 0..mask.width() {
                let dst_px = self.get_pixel(x, y);
                // Apply light mask only if dst pixel is non-transparent
                if dst_px[3] > 0 {
                    let src_px = mask.get_pixel(x, y);
                    // if src_px[3] = alpha is large, paste more of the dst pixel.
                    let a = 1.0 - src_px[3] as f32 / 255.0;
                    let blended = Rgba([
                        (src_px[0] as f32 * a + dst_px[0] as f32 * (1.0 - a)) as u8,
                        (src_px[1] as f32 * a + dst_px[1] as f32 * (1.0 - a)) as u8,
                        (src_px[2] as f32 * a + dst_px[2] as f32 * (1.0 - a)) as u8,
                        255,
                    ]);

                    self.put_pixel(x, y, blended);
                }
            }
        }

        self
    }
}

pub fn open_image(path: &str) -> AppResult<RgbaImage> {
    let file = ASSETS_DIR
        .get_file(path)
        .ok_or_else(|| anyhow!("File {path} not found"))?;

    let img = ImageReader::new(Cursor::new(file.contents()))
        .with_guessed_format()?
        .decode()?
        .into_rgba8();
    Ok(img)
}

pub fn open_gif(filename: String) -> AppResult<Gif> {
    let mut decoder = gif::DecodeOptions::new();
    // Configure the decoder such that it will expand the image to RGBA.
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let file = ASSETS_DIR
        .get_file(filename.clone())
        .ok_or_else(|| anyhow!("Unable to open file {filename}"))?
        .contents();
    let mut decoder = decoder.read_info(file)?;
    let mut gif: Gif = vec![];
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        let img = ImageBuffer::from_raw(
            frame.width as u32,
            frame.height as u32,
            frame.buffer.to_vec(),
        )
        .ok_or_else(|| anyhow!("Unable to decode file {filename} into gif"))?;
        gif.push(img);
    }
    Ok(gif)
}
