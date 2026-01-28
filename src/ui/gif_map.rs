use crate::{
    core::{
        planet::{Planet, PlanetType},
        player::Player,
        spaceship::Spaceship,
        utils::ellipse_coords,
        world::World,
    },
    image::{
        color_map::AsteroidColorMap,
        spaceship::SpaceshipImageId,
        utils::{
            open_gif, open_image, ExtraImageUtils, Gif, LightMaskStyle, STAR_LAYERS,
            UNIVERSE_BACKGROUND,
        },
    },
    types::{AppResult, HashMapWithResult, PlanetId, PlayerId},
    ui::traits::{GifLines, ImageLines, PrintableGif},
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
    open_gif("game/spinning_ball.gif".to_string())
        .expect("Left shot gif should open")
        .iter()
        .map(|img: &RgbaImage| {
            let base = &mut UNIVERSE_BACKGROUND.clone();
            // FIXME: Hardcoded from assets file, should be taken from Sol planet.
            base.copy_non_trasparent_from(img, X_BLIT, Y_BLIT)
                .unwrap_or_else(|_| {
                    panic!("Could not copy_non_trasparent_from at {X_BLIT} {Y_BLIT}")
                });

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
    open_gif("game/left_shot.gif".to_string())
        .expect("Left shot gif should open")
        .to_lines()
});

pub static RIGHT_SHOT_GIF: Lazy<GifLines> = Lazy::new(|| {
    open_gif("game/right_shot.gif".to_string())
        .expect("Right shot gif should open")
        .to_lines()
});

pub static PORTAL_GIFS: Lazy<Vec<GifLines>> = Lazy::new(|| {
    vec![
        open_gif("portal/portal_blue.gif".into())
            .expect("Cannot open portal_blue.gif.")
            .to_lines(),
        open_gif("portal/portal_pink.gif".into())
            .expect("Cannot open portal_pink.gif.")
            .to_lines(),
        open_gif("portal/portal_red.gif".into())
            .expect("Cannot open portal_red.gif.")
            .to_lines(),
    ]
});

pub static TREASURE_GIF: Lazy<GifLines> = Lazy::new(|| {
    open_gif("treasure/treasure.gif".into())
        .expect("Cannot open treasure.gif.")
        .to_lines()
});

pub static _FLAG_GIF: Lazy<GifLines> = Lazy::new(|| {
    open_gif("cove/flag.gif".into())
        .expect("Cannot open flag.gif.")
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
    with_shield_spaceship_lines: HashMap<SpaceshipImageId, GifLines>,
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
            open_gif(format!("planets/{}_full.gif", planet.filename))?
                .iter()
                .map(|img: &RgbaImage| {
                    let base = &mut UNIVERSE_BACKGROUND.clone();

                    // Blit img on base
                    base.copy_non_trasparent_from(img, x_blit, y_blit).unwrap();

                    let center = (x_blit + img.width() / 2, y_blit + img.height() / 2);
                    let img = base
                        .view(
                            center.0 - MAX_GIF_WIDTH / 2,
                            center.1 - MAX_GIF_HEIGHT / 2,
                            MAX_GIF_WIDTH,
                            MAX_GIF_HEIGHT,
                        )
                        .to_image();

                    img
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
        if let Some(lines) = self.planets_zoom_in_lines.get(planet_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let planet = world
            .planets
            .get(planet_id)
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
            let galaxy_gif = open_gif("planets/galaxy.gif".to_string())?;
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

        let mut center_images = open_gif(format!("planets/{}_zoomout.gif", planet.filename))?;

        let satellites_images = planet
            .satellites
            .iter()
            .map(|satellite_id| {
                let satellite = world.planets.get_or_err(satellite_id)?;

                let gif = if satellite.planet_type == PlanetType::Asteroid {
                    let mut img =
                        open_image(format!("asteroids/{}.png", satellite.filename).as_str())?;
                    let color_map = AsteroidColorMap::Base.color_map();
                    img.apply_color_map(color_map);
                    vec![img] as Gif
                } else {
                    open_gif(format!("planets/{}_zoomout.gif", satellite.filename))?
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
            for (idx, satellite_id) in planet.satellites.iter().enumerate() {
                let satellite = world.planets.get_or_err(satellite_id)?;
                // Satellite img moves along an ellipse
                // Can divide safely because if we enter the loop => planet.satellites.len() > 0.
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
            let mut img = base.view(x, y, MAX_GIF_WIDTH, MAX_GIF_HEIGHT).to_image();

            // Stars emit light!
            if planet.planet_type == PlanetType::Sol {
                img.apply_light_mask(&LightMaskStyle::star_zoom_out());
            }

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
            Self::planet_zoom_out(planet, world)?
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

        let gif = spaceship.compose_image(Some(LightMaskStyle::radial()))?;
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

    pub fn with_shield_spaceship_lines(
        &mut self,
        spaceship: &Spaceship,
        tick: usize,
    ) -> AppResult<ImageLines> {
        let spacehip_image_id = spaceship.image_id();

        if let Some(lines) = self.with_shield_spaceship_lines.get(&spacehip_image_id) {
            return Ok(lines[tick % lines.len()].clone());
        }

        let gif = spaceship.compose_image_with_shield()?;
        let lines = gif.to_lines();
        let frame = lines[tick % lines.len()].clone();
        self.with_shield_spaceship_lines
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

        let ship_gif = spaceship
            .compose_image(Some(LightMaskStyle::vertical()))?
            .iter()
            .map(|img| {
                rotate_about_center(
                    img,
                    std::f32::consts::PI / 2.0,
                    Interpolation::Nearest,
                    Rgba([255, 0, 0, 0]),
                )
            })
            .collect::<Gif>();

        let star_layer_width = STAR_LAYERS[0].width();
        let star_layer_height = STAR_LAYERS[0].height();
        let mut gif: Vec<RgbaImage> = vec![];
        // 160 frames
        let mut idx = 0;
        loop {
            let img = &ship_gif[idx % ship_gif.len()];

            let mut base = RgbaImage::new(star_layer_width * 2, star_layer_height * 2);

            if idx >= base.width() as usize / 2 {
                break;
            }

            base.copy_non_trasparent_from(&STAR_LAYERS[1], 0, 0)?;
            base.copy_non_trasparent_from(&STAR_LAYERS[1], star_layer_width, 0)?;

            // STAR_LAYERS[0] do not move with respect to spaceship (far field stars)
            base.copy_non_trasparent_from(&STAR_LAYERS[0], idx as u32, 20)?;

            let y = ((star_layer_height - img.height()) as f32 / 2.0
                + (std::f32::consts::PI * idx as f32 / 17.0).cos()
                + (std::f32::consts::PI * idx as f32 / 333.0).sin()) as u32;
            base.copy_non_trasparent_from(img, idx as u32, y)?;

            let view = base.view(idx as u32, 0, base.width() - idx as u32, star_layer_height);
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

        let ship_gif = spaceship
            .compose_image(Some(LightMaskStyle::vertical()))?
            .iter()
            .map(|img| {
                rotate_about_center(
                    img,
                    std::f32::consts::PI / 2.0,
                    Interpolation::Nearest,
                    Rgba([255, 0, 0, 0]),
                )
            })
            .collect::<Gif>();

        let mut base = RgbaImage::new(STAR_LAYERS[1].width(), STAR_LAYERS[1].height());
        base.copy_non_trasparent_from(&STAR_LAYERS[0], 0, 0)?;
        base.copy_non_trasparent_from(&STAR_LAYERS[1], 0, 0)?;
        let mut gif: Vec<RgbaImage> = vec![];
        // 160 frames
        let mut idx = 0;
        loop {
            let img = &ship_gif[idx % ship_gif.len()];
            let mut base = base.clone();

            if idx >= (base.width() - img.width()) as usize {
                break;
            }

            let y = (base.height() - img.height()) / 2;
            base.copy_non_trasparent_from(img, idx as u32, y)?;
            let view = base.view(img.width(), 0, base.width() - img.width(), base.height());
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
