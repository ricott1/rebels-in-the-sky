use crate::{
    image::{
        color_map::AsteroidColorMap,
        spaceship::SpaceshipImageId,
        types::*,
        utils::{open_image, ExtraImageUtils, TRAVELLING_BACKGROUND, UNIVERSE_BACKGROUND},
    },
    types::{AppResult, PlanetId, PlayerId},
    world::{
        planet::{Planet, PlanetType},
        player::Player,
        spaceship::Spaceship,
        utils::ellipse_coords,
        world::World,
    },
};
use anyhow::anyhow;
use image::{imageops::resize, GenericImageView, Rgba, RgbaImage};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::collections::HashMap;

const MAX_GIF_WIDTH: u32 = 160;
const MAX_GIF_HEIGHT: u32 = 140;
pub const FRAMES_PER_REVOLUTION: usize = 360;

pub static SPINNING_BALL_GIF: Lazy<GifLines> = Lazy::new(|| {
    const X_BLIT: u32 = MAX_GIF_WIDTH / 2 + 30;
    const Y_BLIT: u32 = MAX_GIF_HEIGHT / 2 + 20;
    Gif::open("game/spinning_ball.gif".to_string())
        .expect("Left shot gif should open")
        .iter()
        .map(|img: &RgbaImage| {
            let base = &mut UNIVERSE_BACKGROUND.clone();
            // FIXME: Hardcoded from assets file, should be taken from Sol planet.
            base.copy_non_trasparent_from(&mut img.clone(), X_BLIT, Y_BLIT)
                .expect(
                    format!(
                        "Could not copy_non_trasparent_from at {} {}",
                        X_BLIT, Y_BLIT,
                    )
                    .as_str(),
                );

            let center = (X_BLIT + img.width() / 2, Y_BLIT + img.height() / 2);
            base.view(
                center.0 - MAX_GIF_WIDTH / 2,
                center.1 - MAX_GIF_HEIGHT / 2,
                MAX_GIF_WIDTH,
                MAX_GIF_HEIGHT,
            )
            .to_image()
        })
        .collect::<Gif>()
        .to_lines()
});

pub static LEFT_SHOT_GIF: Lazy<GifLines> = Lazy::new(|| {
    Gif::open("game/left_shot.gif".to_string())
        .expect("Left shot gif should open")
        .to_lines()
});

pub static RIGHT_SHOT_GIF: Lazy<GifLines> = Lazy::new(|| {
    Gif::open("game/right_shot.gif".to_string())
        .expect("Right shot gif should open")
        .to_lines()
});

pub static PORTAL_GIFS: Lazy<Vec<GifLines>> = Lazy::new(|| {
    vec![
        Gif::open("portal/portal_blue.gif".into())
            .expect("Cannot open portal_blue.gif.")
            .to_lines(),
        Gif::open("portal/portal_pink.gif".into())
            .expect("Cannot open portal_pink.gif.")
            .to_lines(),
        Gif::open("portal/portal_red.gif".into())
            .expect("Cannot open portal_red.gif.")
            .to_lines(),
    ]
});

pub static TREASURE_GIF: Lazy<GifLines> = Lazy::new(|| {
    Gif::open("treasure/treasure.gif".into())
        .expect("Cannot open treasure.gif.")
        .to_lines()
});

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
                PlanetType::Rocky => 16,
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
    on_planet_spaceship_lines: HashMap<SpaceshipImageId, GifLines>,
    in_shipyard_spaceship_lines: HashMap<SpaceshipImageId, GifLines>,
    shooting_spaceship_lines: HashMap<SpaceshipImageId, GifLines>,
    travelling_spaceship_lines: HashMap<SpaceshipImageId, GifLines>,
    exploring_spaceship_lines: HashMap<SpaceshipImageId, GifLines>,
    planets_zoom_in_lines: HashMap<PlanetId, GifLines>,
    planets_zoom_out_lines: HashMap<PlanetId, (u64, GifLines)>,
}

impl GifMap {
    pub fn new() -> Self {
        Self::default()
    }

    fn player(&mut self, player: &Player) -> AppResult<Gif> {
        player.compose_image()
    }

    pub fn player_frame_lines(&mut self, player: &Player, tick: usize) -> AppResult<ImageLines> {
        const TICKS_PER_FRAME: usize = 5;

        if let Some((version, lines)) = self.players_lines.get(&player.id) {
            if player.version == *version {
                return Ok(lines[(tick / TICKS_PER_FRAME) % lines.len()].clone());
            }
        }

        let gif = self.player(player)?;
        let lines = gif.to_lines();
        let frame = lines[(tick / TICKS_PER_FRAME) % lines.len()].clone();
        self.players_lines
            .insert(player.id, (player.version, lines));
        Ok(frame)
    }

    fn planet_zoom_in(planet: &Planet) -> AppResult<Gif> {
        // just picked those randomly, we could do better by using some deterministic position
        let x_blit = MAX_GIF_WIDTH / 2 + planet.axis.0 as u32;
        let y_blit = MAX_GIF_HEIGHT / 2 + planet.axis.1 as u32;
        let gif = if planet.planet_type == PlanetType::Asteroid {
            let mut img = open_image(format!("asteroids/{}.png", planet.filename).as_str())?;

            let color_map = AsteroidColorMap::Base.color_map();
            img.apply_color_map(color_map);
            vec![img]
        } else {
            Gif::open(format!("planets/{}_full.gif", planet.filename))?
                .iter()
                .map(|img: &RgbaImage| {
                    let base = &mut UNIVERSE_BACKGROUND.clone();

                    // Blit img on base
                    base.copy_non_trasparent_from(&img, x_blit, y_blit).unwrap();

                    let center = (x_blit + img.width() / 2, y_blit + img.height() / 2);
                    base.view(
                        center.0 - MAX_GIF_WIDTH / 2,
                        center.1 - MAX_GIF_HEIGHT / 2,
                        MAX_GIF_WIDTH,
                        MAX_GIF_HEIGHT,
                    )
                    .to_image()
                })
                .collect::<Gif>()
        };
        Ok(gif)
    }

    pub fn planet_zoom_in_frame_lines(
        &mut self,
        planet_id: &PlanetId,
        tick: usize,
        world: &World,
    ) -> AppResult<ImageLines> {
        if let Some(lines) = self.planets_zoom_in_lines.get(&planet_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let planet = world
            .get_planet(planet_id)
            .ok_or(anyhow!("World: Planet not found."))?;

        let gif = Self::planet_zoom_in(planet)?;
        let lines = gif.to_lines();
        let frame = lines[tick % lines.len()].clone();
        self.planets_zoom_in_lines.insert(planet.id, lines);
        Ok(frame)
    }

    // We handle the asteroid as a special case because we have no zoomout iamges for them.
    pub fn asteroid_zoom_out(filename: &str) -> AppResult<Gif> {
        let mut img = open_image(format!("asteroids/{filename}.png",).as_str())?;
        let color_map = AsteroidColorMap::Base.color_map();

        let size = ImageResizeInGalaxyGif::ZoomOutCentral {
            planet_type: PlanetType::Asteroid,
        }
        .size();
        img.apply_color_map(color_map);

        img = resize(
            &img,
            2 * size,
            2 * size,
            image::imageops::FilterType::Triangle,
        );
        img = resize(&img, size, size, image::imageops::FilterType::Nearest);

        Ok(vec![img] as Gif)
    }

    fn planet_zoom_out(planet: &Planet, world: &World) -> AppResult<Gif> {
        // Background images. The black hole has the galaxy gif as base image.
        let base_images = if planet.satellite_of.is_none() {
            let galaxy_gif = Gif::open("planets/galaxy.gif".to_string())?;
            let mut base_gif = vec![];
            for frame in galaxy_gif.iter() {
                let mut frame_base = RgbaImage::new(MAX_GIF_WIDTH, MAX_GIF_HEIGHT);
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

        let mut center_images: Gif = Gif::open(format!("planets/{}_zoomout.gif", planet.filename))?;

        let satellites_images: Vec<Gif> = planet
            .satellites
            .iter()
            .map(|satellite_id| {
                let satellite = world.get_planet_or_err(satellite_id)?;

                let gif = if satellite.planet_type == PlanetType::Asteroid {
                    let mut img =
                        open_image(format!("asteroids/{}.png", satellite.filename).as_str())?;
                    let color_map = AsteroidColorMap::Base.color_map();
                    img.apply_color_map(color_map);
                    vec![img] as Gif
                } else {
                    Gif::open(format!("planets/{}_zoomout.gif", satellite.filename))?
                };

                let size = ImageResizeInGalaxyGif::ZoomOutSatellite {
                    planet_type: satellite.planet_type,
                }
                .size();

                let g = gif
                    .iter()
                    .map(|img| {
                        //We resize twice to try to get nicer looking results
                        let r = resize(
                            img,
                            2 * size,
                            2 * size,
                            image::imageops::FilterType::Triangle,
                        );
                        resize(&r, size, size, image::imageops::FilterType::Nearest)
                    })
                    .collect_vec();
                Ok(g)
            })
            .collect::<AppResult<Vec<Gif>>>()?;

        let mut frames = vec![];

        let center_img_len = center_images.len();

        for tick in 0..FRAMES_PER_REVOLUTION {
            let mut base = base_images[tick % base_images.len()].clone();
            let center_img = &mut center_images[(tick / planet.rotation_period) % center_img_len];

            // Blit star on base
            let x = (base.width() - center_img.width()) / 2;
            let y = (base.height() - center_img.height()) / 2;
            base.copy_non_trasparent_from(center_img, x, y)?;

            // Blit satellite images on base
            for idx in 0..planet.satellites.len() {
                let satellite = world.get_planet_or_err(&planet.satellites[idx])?;
                // Satellite img moves along an ellipse
                let theta_0 =
                    idx as f32 * 2.0 * std::f32::consts::PI / planet.satellites.len() as f32;

                let theta = theta_0
                    + (tick as f32 * 2.0 * std::f32::consts::PI)
                        / satellite.revolution_period as f32;

                let (mut x_planet, mut y_planet) = ellipse_coords(satellite.axis, theta);
                x_planet += base.width() as f32 / 2.0;
                y_planet += base.height() as f32 / 2.0;

                let satellites_images_len = satellites_images[idx].len();
                let satellite_img = &satellites_images[idx]
                    [(tick / satellite.rotation_period) % satellites_images_len];

                base.copy_non_trasparent_from(
                    satellite_img,
                    x_planet.round() as u32,
                    y_planet.round() as u32,
                )?;
            }

            // take subimage around center of base
            let x = base.width().saturating_sub(MAX_GIF_WIDTH) / 2;
            let y = base.height().saturating_sub(MAX_GIF_HEIGHT) / 2;
            let img = base.view(x, y, MAX_GIF_WIDTH, MAX_GIF_HEIGHT).to_image();
            frames.push(img);
        }

        Ok(frames)
    }

    pub fn planet_zoom_out_frame_lines(
        &mut self,
        planet: &Planet,
        tick: usize,
        world: &World,
    ) -> AppResult<ImageLines> {
        if let Some((version, lines)) = self.planets_zoom_out_lines.get(&planet.id) {
            if planet.version == *version {
                return Ok(lines[tick % lines.len()].clone());
            }
        }

        let gif = if planet.planet_type == PlanetType::Asteroid {
            Self::asteroid_zoom_out(&planet.filename)?
        } else {
            Self::planet_zoom_out(&planet, world)?
        };
        let lines = gif.to_lines();
        let frame = lines[tick % lines.len()].clone();
        self.planets_zoom_out_lines
            .insert(planet.id, (planet.version, lines));
        Ok(frame)
    }

    pub fn on_planet_spaceship_lines(
        &mut self,
        spaceship: &Spaceship,
        tick: usize,
    ) -> AppResult<ImageLines> {
        let spacehip_image_id = spaceship.image_id();

        if let Some(lines) = self.on_planet_spaceship_lines.get(&spacehip_image_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let gif = spaceship.compose_image()?;
        let lines = gif.to_lines();
        let frame = lines[tick % lines.len()].clone();
        self.on_planet_spaceship_lines
            .insert(spacehip_image_id, lines);
        Ok(frame)
    }

    pub fn in_shipyard_spaceship_lines(
        &mut self,
        spaceship: &Spaceship,
        tick: usize,
    ) -> AppResult<ImageLines> {
        let spacehip_image_id = spaceship.image_id();

        if let Some(lines) = self.in_shipyard_spaceship_lines.get(&spacehip_image_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let gif = spaceship.compose_image_in_shipyard()?;
        let lines = gif.to_lines();
        let frame = lines[tick % lines.len()].clone();
        self.in_shipyard_spaceship_lines
            .insert(spacehip_image_id, lines);
        Ok(frame)
    }

    pub fn shooting_spaceship_lines(
        &mut self,
        spaceship: &Spaceship,
        tick: usize,
    ) -> AppResult<ImageLines> {
        let spacehip_image_id = spaceship.image_id();

        if let Some(lines) = self.shooting_spaceship_lines.get(&spacehip_image_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let gif = spaceship.compose_image_shooting()?;
        let lines = gif.to_lines();
        let frame = lines[tick % lines.len()].clone();
        self.shooting_spaceship_lines
            .insert(spacehip_image_id, lines);
        Ok(frame)
    }

    pub fn travelling_spaceship_lines(
        &mut self,
        spaceship: &Spaceship,
        tick: usize,
    ) -> AppResult<ImageLines> {
        let spacehip_image_id = spaceship.image_id();

        if let Some(lines) = self.travelling_spaceship_lines.get(&spacehip_image_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let ship_gif = spaceship.compose_image()?;
        let base = TRAVELLING_BACKGROUND.clone();
        let mut gif: Vec<RgbaImage> = vec![];
        // 160 frames
        let mut idx = 0;
        loop {
            let img = ship_gif[idx % ship_gif.len()].clone();
            let bg_left = base.clone();
            let bg_right = base.clone();
            let mut base = RgbaImage::new(bg_left.width() * 2, bg_left.height());

            if idx >= base.width() as usize / 2 {
                break;
            }

            base.copy_non_trasparent_from(&bg_left, 0, 0)?;
            base.copy_non_trasparent_from(&bg_right, base.width() / 2, 0)?;

            let rotated_img = rotate_about_center(
                &img,
                std::f32::consts::PI / 2.0,
                Interpolation::Nearest,
                Rgba([255, 0, 0, 0]),
            );
            let y = ((base.height() - rotated_img.height()) as f32 / 2.0
                + (std::f32::consts::PI * idx as f32 / 17.0).cos()
                + (std::f32::consts::PI * idx as f32 / 333.0).sin()) as u32;
            base.copy_non_trasparent_from(&rotated_img, idx as u32, y)?;
            let view = base.view(idx as u32, 0, base.width() - idx as u32, base.height());
            gif.push(view.to_image());
            idx += 1;
        }

        let lines = gif.to_lines();
        let frame = lines[tick % lines.len()].clone();
        self.travelling_spaceship_lines
            .insert(spacehip_image_id, lines);
        Ok(frame)
    }

    pub fn exploring_spaceship_lines(
        &mut self,
        spaceship: &Spaceship,
        tick: usize,
    ) -> AppResult<ImageLines> {
        let spacehip_image_id = spaceship.image_id();

        if let Some(lines) = self.exploring_spaceship_lines.get(&spacehip_image_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let ship_gif = spaceship.compose_image()?;
        let base = TRAVELLING_BACKGROUND.clone();
        let mut gif: Vec<RgbaImage> = vec![];
        // 160 frames
        let mut idx = 0;
        loop {
            let img = ship_gif[idx % ship_gif.len()].clone();
            let mut base = base.clone();
            let rotated_img = rotate_about_center(
                &img,
                std::f32::consts::PI / 2.0,
                Interpolation::Nearest,
                Rgba([255, 0, 0, 0]),
            );

            if idx >= (base.width() - rotated_img.width()) as usize {
                break;
            }

            let y = (base.height() - rotated_img.height()) / 2;
            base.copy_non_trasparent_from(&rotated_img, idx as u32, y)?;
            let view = base.view(
                rotated_img.width(),
                0,
                base.width() - rotated_img.width(),
                base.height(),
            );
            gif.push(view.to_image());
            idx += 1;
        }

        let lines = gif.to_lines();
        let frame = lines[tick % lines.len()].clone();
        self.exploring_spaceship_lines
            .insert(spacehip_image_id, lines);
        Ok(frame)
    }
}
