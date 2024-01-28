use super::{planet::Planet, skill::GameSkill};
use crate::store::ASSETS_DIR;
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TeamData {
    pub names: Vec<(String, String)>,
}

#[derive(Deserialize)]
pub struct PlayerData {
    pub first_names_he: Vec<Vec<String>>,
    pub first_names_she: Vec<Vec<String>>,
    pub last_names: Vec<Vec<String>>,
}

pub fn linear_interpolation(x: f32, coords: [f32; 4]) -> f32 {
    // coords = (x1, y1, x2, y2)
    // y = y1 + (y2 - y1) / (x2 - x1) * (x - x1)
    coords[1] + (coords[3] - coords[1]) / (coords[2] - coords[0]) * (x - coords[0])
}

pub fn skill_linear_interpolation(base_skill: f32, mod_skill: f32, coords: [f32; 4]) -> f32 {
    let modifier: f32;
    if (mod_skill) < coords[0] {
        modifier = coords[1];
    } else if (mod_skill) > coords[2] {
        modifier = coords[3];
    } else {
        modifier = linear_interpolation(mod_skill, coords);
    }
    (base_skill * modifier).bound()
}

pub static PLAYER_DATA: Lazy<Option<PlayerData>> = Lazy::new(|| {
    let file = ASSETS_DIR.get_file("data/players_data.json")?;
    let data = file.contents_utf8()?;
    serde_json::from_str(&data).ok()
});

pub static TEAM_DATA: Lazy<Option<TeamData>> = Lazy::new(|| {
    let file = ASSETS_DIR.get_file("data/teams_data.json")?;
    let data = file.contents_utf8()?;
    serde_json::from_str(&data).ok()
});

pub static PLANET_DATA: Lazy<Option<Vec<Planet>>> = Lazy::new(|| {
    let file = ASSETS_DIR.get_file("data/planets_data.json")?;
    let data = file.contents_utf8()?;
    serde_json::from_str(&data).ok()
});

pub fn ellipse_coords(axis: (f32, f32), theta: f32) -> (f32, f32) {
    let a = axis.0;
    let b = axis.1;
    let radius = a * b / ((b * (theta).cos()).powi(2) + (a * (theta).sin()).powi(2)).sqrt();
    (
        (radius * (theta).cos()).round(),
        (radius * (theta).sin()).round(),
    )
}

#[cfg(test)]
mod tests {
    use super::skill_linear_interpolation;

    //test linear interopolation
    #[test]
    fn test_linear_interpolation() {
        let coords = [30.0, 1.0, 36.0, 0.5];
        let base = 10.0;

        assert_eq!(skill_linear_interpolation(base, 16.0, coords), 10.0);
        assert_eq!(skill_linear_interpolation(base, 29.0, coords), 10.0);
        assert_eq!(skill_linear_interpolation(base, 30.0, coords), 10.0);
        assert_eq!(skill_linear_interpolation(base, 31.0, coords), 9.166667);
        assert_eq!(skill_linear_interpolation(base, 36.0, coords), 5.0);
        assert_eq!(skill_linear_interpolation(base, 37.0, coords), 5.0);
    }

    #[test]
    fn test_ellipse_coords() {
        let axis = (100.0, 50.0);
        let theta = 0.0;
        assert_eq!(super::ellipse_coords(axis, theta), (100.0, 0.0));
        let theta = std::f32::consts::PI / 2.0;
        assert_eq!(super::ellipse_coords(axis, theta), (0.0, 50.0));
        let theta = std::f32::consts::PI;
        assert_eq!(super::ellipse_coords(axis, theta), (-100.0, 0.0));
        let theta = 3.0 * std::f32::consts::PI / 2.0;
        assert_eq!(super::ellipse_coords(axis, theta), (0.0, -50.0));
    }
}
