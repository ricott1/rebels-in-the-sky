use super::color_map::ColorMap;
use crate::store::ASSETS_DIR;
use crate::types::AppResult;
use anyhow::anyhow;
use image::error::{ParameterError, ParameterErrorKind};
use image::ImageReader;
use image::{ImageError, ImageResult, Rgba, RgbaImage};
use once_cell::sync::Lazy;
use std::io::Cursor;

pub static UNIVERSE_BACKGROUND: Lazy<RgbaImage> =
    Lazy::new(|| open_image("planets/background.png").expect("Cannot open background.png."));
pub static TRAVELLING_BACKGROUND: Lazy<RgbaImage> = Lazy::new(|| {
    open_image("planets/travelling_background.png").expect("Cannot open travelling_background.png.")
});

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
    fn apply_color_map_with_shadow_mask(
        &mut self,
        color_map: ColorMap,
        mask: &RgbaImage,
    ) -> &RgbaImage;
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
}

pub fn open_image(path: &str) -> AppResult<RgbaImage> {
    let file = ASSETS_DIR.get_file(path);
    if file.is_none() {
        return Err(anyhow!("File {path} not found"));
    }
    let img = ImageReader::new(Cursor::new(file.unwrap().contents()))
        .with_guessed_format()?
        .decode()?
        .into_rgba8();
    Ok(img)
}
