use crate::{
    image::{
        types::Gif,
        utils::{read_image, ExtraImageUtils},
    },
    store::ASSETS_DIR,
    types::{AppResult, PlanetId, PlayerId, TeamId},
    ui::utils::img_to_lines,
    world::{
        planet::{Planet, PlanetType},
        player::Player,
        utils::ellipse_coords,
        world::World,
    },
};
use image::{imageops::resize, GenericImageView, ImageBuffer, Rgba, RgbaImage};
use once_cell::sync::Lazy;
use ratatui::text::Line;
use std::{collections::HashMap, error::Error};

pub type FrameLines = Vec<Line<'static>>;
pub type GifLines = Vec<FrameLines>;

const MAX_GIF_WIDTH: u32 = 160;
const MAX_GIF_HEIGHT: u32 = 140;
// pub const TICKS_PER_REVOLUTION: usize = 3;
pub const FRAMES_PER_REVOLUTION: usize = 360;
pub static UNIVERSE_BACKGROUND: Lazy<RgbaImage> =
    Lazy::new(|| read_image("planets/background.png").unwrap());

pub enum ImageResizeInGalaxyGif {
    ZoomOutCentral { planet_type: PlanetType },
    ZoomOutSatellite { planet_type: PlanetType },
}

impl ImageResizeInGalaxyGif {
    pub fn size(&self) -> u32 {
        match self {
            ImageResizeInGalaxyGif::ZoomOutCentral { planet_type } => match planet_type {
                PlanetType::BlackHole => 16,
                PlanetType::Sol => 28,
                PlanetType::Earth => 24,
                PlanetType::Ring => 32,
                _ => 24,
            },
            ImageResizeInGalaxyGif::ZoomOutSatellite { planet_type } => match planet_type {
                PlanetType::Gas => 8,
                PlanetType::Sol => 8,
                PlanetType::Rocky => 4,
                PlanetType::Ring => 16,
                _ => 6,
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct GifMap {
    players_lines: HashMap<PlayerId, (u64, GifLines)>,
    spaceship_lines: HashMap<TeamId, GifLines>,
    planets_zoom_in_lines: HashMap<PlanetId, GifLines>,
    planets_zoom_out_lines: HashMap<PlanetId, GifLines>,
}

impl GifMap {
    pub fn new() -> Self {
        Self::default()
    }

    fn open_gif(filename: String) -> Gif {
        let mut decoder = gif::DecodeOptions::new();
        // Configure the decoder such that it will expand the image to RGBA.
        decoder.set_color_output(gif::ColorOutput::RGBA);
        let file = ASSETS_DIR.get_file(filename).unwrap().contents();
        let mut decoder = decoder.read_info(file).unwrap();
        let mut gif: Gif = vec![];
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            let img = ImageBuffer::from_raw(
                frame.width as u32,
                frame.height as u32,
                frame.buffer.to_vec(),
            )
            .unwrap();
            gif.push(img);
        }
        gif
    }

    fn gif_to_lines(gif: &Gif) -> GifLines {
        gif.iter().map(|img| img_to_lines(img)).collect()
    }

    fn player(&mut self, player: &Player) -> AppResult<Gif> {
        let gif = player
            .compose_image()
            .map_err(|err: Box<dyn Error>| err.to_string())?;

        Ok(gif)
    }

    pub fn player_frame_lines(
        &mut self,
        player_id: PlayerId,
        tick: usize,
        world: &World,
    ) -> AppResult<FrameLines> {
        let player = world
            .get_player(player_id)
            .ok_or("World: Player not found.")?;

        if let Some((version, lines)) = self.players_lines.get(&player.id) {
            if player.version == *version {
                return Ok(lines[(tick / 8) % lines.len()].clone());
            }
        }

        let gif = self.player(player)?;
        let lines = Self::gif_to_lines(&gif);
        self.players_lines
            .insert(player.id, (player.version, lines.clone()));
        Ok(lines[(tick / 8) % lines.len()].clone())
    }

    fn planet_zoom_in(&mut self, planet: &Planet) -> AppResult<Gif> {
        // just picked those randomly, we could do better by using some deterministic position
        let x_blit = MAX_GIF_WIDTH / 2 + planet.axis.0 as u32;
        let y_blit = MAX_GIF_HEIGHT / 2 + planet.axis.1 as u32;
        let gif = Self::open_gif(format!("planets/{}_full.gif", planet.filename.clone()))
            .iter()
            .map(|img: &ImageBuffer<Rgba<u8>, Vec<u8>>| {
                let base = &mut UNIVERSE_BACKGROUND.clone();

                // Blit img on base
                base.copy_non_trasparent_from(&mut img.clone(), x_blit, y_blit)
                    .unwrap();

                let center = (x_blit + img.width() / 2, y_blit + img.height() / 2);
                base.view(
                    center.0 - MAX_GIF_WIDTH / 2,
                    center.1 - MAX_GIF_HEIGHT / 2,
                    MAX_GIF_WIDTH,
                    MAX_GIF_HEIGHT,
                )
                .to_image()
            })
            .collect::<Gif>();
        Ok(gif)
    }

    pub fn planet_zoom_in_frame_lines(
        &mut self,
        planet_id: PlanetId,
        tick: usize,
        world: &World,
    ) -> AppResult<FrameLines> {
        if let Some(lines) = self.planets_zoom_in_lines.get(&planet_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let planet = world
            .get_planet(planet_id)
            .ok_or("World: Planet not found.")?;

        let gif = self.planet_zoom_in(planet)?;
        let lines = Self::gif_to_lines(&gif);
        self.planets_zoom_in_lines.insert(planet.id, lines.clone());
        Ok(lines[tick % lines.len()].clone())
    }

    fn planet_zoom_out(&mut self, planet_id: PlanetId, world: &World) -> AppResult<Gif> {
        let planet = world.planets.get(&planet_id).unwrap();
        let base_images = if planet.satellite_of.is_none() {
            let galaxy_gif = Self::open_gif("planets/galaxy.gif".to_string());
            let mut base_gif = vec![];
            let base = RgbaImage::new(MAX_GIF_WIDTH, MAX_GIF_HEIGHT);
            for frame in galaxy_gif.iter() {
                let mut frame_base = base.clone();
                frame_base.copy_non_trasparent_from(
                    frame,
                    (MAX_GIF_WIDTH - frame.width()) / 2,
                    (MAX_GIF_HEIGHT - frame.height()) / 2,
                )?;
                base_gif.push(frame_base);
            }
            base_gif
        } else {
            vec![UNIVERSE_BACKGROUND.clone()]
        };

        let center_images: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> =
            Self::open_gif(format!("planets/{}_zoomout.gif", planet.filename.clone()));

        let satellites_images: Vec<Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>> = planet
            .satellites
            .iter()
            .map(|satellite_id| {
                let satellite = world.planets.get(satellite_id).unwrap();
                let size = ImageResizeInGalaxyGif::ZoomOutSatellite {
                    planet_type: satellite.planet_type.clone(),
                }
                .size();
                Self::open_gif(format!(
                    "planets/{}_zoomout.gif",
                    satellite.filename.clone()
                ))
                .iter()
                .map(|img| {
                    //We resize twice to try to get nicer looking results
                    resize(
                        img,
                        2 * size,
                        2 * size,
                        image::imageops::FilterType::Triangle,
                    )
                })
                .map(|img| resize(&img, size, size, image::imageops::FilterType::Nearest))
                .collect()
            })
            .collect();

        let mut frames = vec![];

        for tick in 0..FRAMES_PER_REVOLUTION {
            let mut base = base_images[tick % base_images.len()].clone();
            let mut center_img =
                center_images[(tick / planet.rotation_period) % center_images.len()].clone();

            let x_origin = if planet.satellite_of.is_none() {
                base.width() / 2
            } else {
                4 * (MAX_GIF_WIDTH / 2 + planet.axis.0 as u32) / 4
            };
            let y_origin = if planet.satellite_of.is_none() {
                base.height() / 2
            } else {
                4 * (MAX_GIF_HEIGHT / 2 + planet.axis.1 as u32) / 4
            };

            // Blit star on base
            let x = x_origin - center_img.width() / 2;
            let y = y_origin - center_img.height() / 2;
            base.copy_non_trasparent_from(&mut center_img, x, y)
                .unwrap();

            //blit planet imgs on base
            for idx in 0..planet.satellites.len() {
                let satellite_id = planet.satellites[idx];
                if let Some(satellite) = world.planets.get(&satellite_id) {
                    // Satellite img moves along an ellipse
                    let theta_0 = idx.clone() as f32 * 2.0 * std::f32::consts::PI
                        / planet.satellites.len() as f32;
                    let theta = theta_0
                        + (tick as f32 * 2.0 * std::f32::consts::PI)
                            / satellite.revolution_period as f32;

                    let (mut x_planet, mut y_planet) = ellipse_coords(satellite.axis, theta);
                    x_planet += x_origin as f32;
                    y_planet += y_origin as f32;

                    let mut satellite_img = satellites_images[idx]
                        [(tick / satellite.rotation_period) % satellites_images.len()]
                    .clone();

                    base.copy_non_trasparent_from(
                        &mut satellite_img,
                        x_planet.round() as u32,
                        y_planet.round() as u32,
                    )
                    .unwrap();
                }
            }
            // take subimage around center of base
            let x = if x_origin > MAX_GIF_WIDTH / 2 {
                x_origin - MAX_GIF_WIDTH / 2
            } else {
                0
            };
            let y = if y_origin > MAX_GIF_HEIGHT / 2 {
                y_origin - MAX_GIF_HEIGHT / 2
            } else {
                0
            };
            let mut width = if base.width() > MAX_GIF_WIDTH {
                MAX_GIF_WIDTH
            } else {
                base.width()
            };
            if x + width > base.width() {
                width = base.width() - x;
            }

            let mut height = if base.height() > MAX_GIF_HEIGHT {
                MAX_GIF_HEIGHT
            } else {
                base.height()
            };
            if y + height > base.height() {
                height = base.height() - y;
            }

            let img = base.view(x, y, width, height).to_image();
            frames.push(img);
        }

        Ok(frames)
    }

    pub fn planet_zoom_out_frame_lines(
        &mut self,
        planet_id: PlanetId,
        tick: usize,
        world: &World,
    ) -> AppResult<FrameLines> {
        if let Some(lines) = self.planets_zoom_out_lines.get(&planet_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let gif = self.planet_zoom_out(planet_id, world)?;
        let lines = Self::gif_to_lines(&gif);
        self.planets_zoom_out_lines.insert(planet_id, lines.clone());
        Ok(lines[tick % lines.len()].clone())
    }

    pub fn spaceship_lines(
        &mut self,
        team_id: TeamId,
        tick: usize,
        world: &World,
    ) -> AppResult<FrameLines> {
        if let Some(lines) = self.spaceship_lines.get(&team_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let team = world.get_team_or_err(team_id)?;
        let gif = team.spaceship.compose_image()?;
        let lines = Self::gif_to_lines(&gif);
        self.planets_zoom_out_lines.insert(team_id, lines.clone());
        Ok(lines[tick % lines.len()].clone())
    }
}
