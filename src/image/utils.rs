use super::color_map::ColorMap;
use crate::store::ASSETS_DIR;
use crate::types::AppResult;
use image::error::{ParameterError, ParameterErrorKind};
use image::io::Reader as ImageReader;
use image::{ImageBuffer, ImageError, ImageResult, Rgba, RgbaImage};
use std::io::Cursor;

pub trait ExtraImageUtils {
    fn copy_non_trasparent_from(
        &mut self,
        other: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
    ) -> ImageResult<()>;
    fn apply_color_map(&mut self, color_map: ColorMap) -> &ImageBuffer<Rgba<u8>, Vec<u8>>;
    fn apply_color_map_with_shadow_mask(
        &mut self,
        color_map: ColorMap,
        mask: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> &ImageBuffer<Rgba<u8>, Vec<u8>>;
}

impl ExtraImageUtils for ImageBuffer<Rgba<u8>, Vec<u8>> {
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
    fn copy_non_trasparent_from(
        &mut self,
        other: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: u32,
        y: u32,
    ) -> ImageResult<()> {
        // Do bounds checking here so we can use the non-bounds-checking
        // functions to copy pixels.
        if self.width() < other.width() + x || self.height() < other.height() + y {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )));
        }

        for k in 0..other.height() {
            for i in 0..other.width() {
                let p = other.get_pixel(i, k);
                if p[3] > 0 {
                    self.put_pixel(i + x, k + y, *p);
                }
            }
        }
        Ok(())
    }
    fn apply_color_map(&mut self, color_map: ColorMap) -> &ImageBuffer<Rgba<u8>, Vec<u8>> {
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
        mask: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> &ImageBuffer<Rgba<u8>, Vec<u8>> {
        for k in 0..self.height() {
            for i in 0..self.width() {
                let p = self.get_pixel(i, k);
                if p[3] > 0 {
                    let mask_p = mask.get_pixel_checked(i, k).unwrap_or_else(|| {
                        log::error!("Failed to get pixel from mask: {:?}", color_map);
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

pub fn read_image(path: &str) -> AppResult<RgbaImage> {
    let file = ASSETS_DIR.get_file(path);
    if file.is_none() {
        return Err(format!("File {} not found", path).into());
    }
    let img = ImageReader::new(Cursor::new(file.unwrap().contents()))
        .with_guessed_format()?
        .decode()?
        .into_rgba8();
    Ok(img)
}
