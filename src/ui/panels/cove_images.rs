use crate::core::*;
use crate::image::player::PLAYER_IMAGE_WIDTH;
use crate::image::utils::{open_image, ExtraImageUtils};
use crate::types::{AppResult, PlayerId};
use crate::ui::constants::*;
use crate::ui::renders::default_block;
use crate::ui::ui_frame::UiFrame;
use image::RgbaImage;
use itertools::Itertools;
use ratatui::prelude::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph};

pub(crate) const COVE_IMAGE_WIDTH: u32 = 122;
pub(crate) const PIRATE_X_STEP: u32 = 20;
pub(crate) const DRAWN_PIRATES: usize = 3;
const _: () = assert!(
    MAX_TAVERN_POPULATION as usize <= DRAWN_PIRATES,
    "the tavern picture would quietly drop a pirate"
);
pub(crate) const TAVERN_PIRATE_BASELINES_Y: [u32; DRAWN_PIRATES] = [66, 70, 67];

pub(crate) fn pirate_frames(player_ids: &[PlayerId], world: &World) -> Vec<RgbaImage> {
    player_ids
        .iter()
        .filter_map(|id| world.players.get(id))
        .filter_map(|player| player.compose_image().ok())
        .filter_map(|gif| gif.into_iter().next())
        .collect()
}

pub(crate) fn strongest_pirates(player_ids: &[PlayerId], world: &World) -> Vec<PlayerId> {
    player_ids
        .iter()
        .filter_map(|id| world.players.get(id))
        .collect_vec()
        .sort_by_rating()
        .into_iter()
        .take(DRAWN_PIRATES)
        .map(|player| player.id)
        .collect()
}

fn pirate_group_origin_x(count: usize) -> u32 {
    let group_width = PIRATE_X_STEP * count.saturating_sub(1) as u32 + PLAYER_IMAGE_WIDTH;
    COVE_IMAGE_WIDTH.saturating_sub(group_width) / 2 + 4
}

pub(crate) fn blit_pirate_group(
    base: &mut RgbaImage,
    pirate_frames: &[RgbaImage],
    baselines_y: &[u32],
) -> AppResult<()> {
    let pirates = &pirate_frames[..pirate_frames.len().min(baselines_y.len())];
    let mut x = pirate_group_origin_x(pirates.len());
    for (index, frame) in pirates.iter().enumerate() {
        let y = baselines_y[index].saturating_sub(frame.height());
        base.copy_non_trasparent_from(frame, x, y)?;
        x += PIRATE_X_STEP;
    }
    Ok(())
}

pub(crate) fn render_pirate_summaries(
    frame: &mut UiFrame,
    world: &World,
    player_ids: &[PlayerId],
    selected: Option<PlayerId>,
    baselines_y: &[u32],
    area: Rect,
) {
    let pirates = player_ids
        .iter()
        .filter_map(|id| world.players.get(id))
        .take(baselines_y.len())
        .collect_vec();

    let mut x = pirate_group_origin_x(pirates.len());
    for (index, player) in pirates.iter().enumerate() {
        let best_position = player.best_position();
        let lines = vec![
            Line::from(player.info.short_name()).centered(),
            Line::from(format!(
                "{} {}",
                best_position.as_role(),
                player.position_rating(best_position).stars()
            ))
            .centered(),
        ];
        // Rounded up so the caption clears the feet of a pirate
        // whose baseline falls mid-cell.
        let rect = Rect::new(
            area.x + x as u16,
            area.y + baselines_y[index].div_ceil(2) as u16,
            PLAYER_IMAGE_WIDTH as u16,
            lines.len() as u16 + 2,
        )
        .intersection(area);
        let style = if selected == Some(player.id) {
            UiStyle::DEFAULT
        } else {
            UiStyle::UNSELECTABLE
        };
        frame.render_widget(Clear, rect);
        frame.render_widget(
            Paragraph::new(lines).style(style).block(default_block()),
            rect,
        );
        x += PIRATE_X_STEP;
    }
}

pub(crate) fn get_market_image() -> AppResult<RgbaImage> {
    let mut base = open_image("cove/market.png")?;
    let outer = open_image("cove/base_outer.png")?;
    base.copy_non_trasparent_from(&outer, 0, 0)?;
    Ok(base)
}
