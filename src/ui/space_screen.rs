use super::constants::BARS_LENGTH;
use super::traits::Screen;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::utils::{big_text, img_to_lines};
use super::widgets::{get_charge_spans, get_durability_spans, get_fuel_spans, get_storage_spans};
use crate::core::world::World;
use crate::space_adventure::ControllableSpaceship;
use crate::types::AppResult;
use crate::ui::ui_key;
use core::fmt::Debug;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::{prelude::Rect, widgets::Paragraph};

#[derive(Debug, Default)]
pub struct SpaceScreen {
    tick: usize,
    entity_count: usize,
    controls: Paragraph<'static>,
}

impl SpaceScreen {
    pub fn new() -> Self {
        //       ╔═════╗         ╔═════╗            ╔═════╗                  ╔═════╗
        //       ║  ↑  ║         ║  x  ║ autofire   ║  x  ║ toggle shield    ║  x  ║ release scraps
        // ╔═════╬═════╬═════╗   ╚═════╝╔═════╗     ╚═════╝╔═════╗           ╚═════╝
        // ║  ←  ║  ↓  ║  →  ║          ║  x  ║ shoot      ║  x  ║ return home
        // ╚═════╩═════╩═════╝          ╚═════╝            ╚═════╝
        let controls = [
            "      ╔═════╗         ╔═════╗            ╔═════╗                  ╔═════╗".to_string(),
            format!(
                "      ║  ↑  ║         ║  {}  ║ autofire   ║  {}  ║ toggle shield    ║  {}  ║ release scraps",
                ui_key::space::AUTOFIRE,
                ui_key::space::TOGGLE_SHIELD,
                ui_key::space::RELEASE_SCRAPS
            ),
            "╔═════╬═════╬═════╗   ╚═════╝╔═════╗     ╚═════╝╔═════╗           ╚═════╝".to_string(),
            format!(
                "║  ←  ║  ↓  ║  →  ║          ║  {}  ║ shoot      ║  {}  ║ return home  ",
                ui_key::space::SHOOT,
                ui_key::space::BACK_TO_BASE
            ),
            "╚═════╩═════╩═════╝          ╚═════╝            ╚═════╝              ".to_string(),
        ];
        Self {
            controls: big_text(&controls).left_aligned(),
            ..Default::default()
        }
    }
}

impl Screen for SpaceScreen {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;

        if let Some(space_adventure) = &world.space_adventure {
            self.entity_count = space_adventure.entity_count();
        }

        Ok(())
    }

    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        debug_view: bool,
    ) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Min(10), Constraint::Length(1)]).split(area);
        let space_adventure = if let Some(space_adventure) = &world.space_adventure {
            space_adventure
        } else {
            return Ok(());
        };

        match space_adventure.image(
            split[0].width as u32,
            split[0].height as u32 * 2,
            debug_view,
        ) {
            Ok(img) => {
                let mut space_img_lines = img_to_lines(&img);
                space_img_lines.truncate(split[0].height as usize);
                frame.render_widget(Paragraph::new(space_img_lines), split[0]);
            }

            Err(e) => {
                frame.render_widget(Paragraph::new(e.to_string()).centered(), split[0]);
            }
        }

        let info_split = Layout::horizontal([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(split[1]);

        if let Some(player) = space_adventure.get_player() {
            let bars_length = (area.width as usize / 4 - 20).min(BARS_LENGTH);

            let mut shield_current_durability = 0;
            let mut shield_max_durability = 0;
            if let Some(id) = player.shield_id() {
                if let Some(entity) = space_adventure.get_entity(&id) {
                    let shield = entity.as_shield()?;
                    shield_current_durability = shield.current_durability();
                    shield_max_durability = shield.max_durability();
                }
            }

            frame.render_widget(
                Line::from(get_durability_spans(
                    player.current_durability(),
                    player.max_durability(),
                    shield_current_durability,
                    shield_max_durability,
                    bars_length,
                )),
                info_split[0],
            );

            let is_recharging = player.is_recharging();

            frame.render_widget(
                Line::from(get_charge_spans(
                    player.current_charge(),
                    player.max_charge(),
                    is_recharging,
                    bars_length,
                )),
                info_split[1],
            );

            frame.render_widget(
                Line::from(get_fuel_spans(
                    player.fuel(),
                    player.fuel_capacity(),
                    bars_length,
                )),
                info_split[2],
            );

            frame.render_widget(
                Line::from(get_storage_spans(
                    player.resources(),
                    player.storage_capacity(),
                    bars_length,
                )),
                info_split[3],
            );
        }
        if space_adventure.is_starting() || debug_view {
            let v_split =
                Layout::vertical([Constraint::Min(0), Constraint::Length(5)]).split(split[0]);
            frame.render_widget(Clear, v_split[1]);
            frame.render_widget(&self.controls, v_split[1]);
        }

        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<super::ui_callback::UiCallback> {
        if ui_key::space::ALL.contains(&key_event.code) {
            return Some(UiCallback::SpaceAdventurePlayerInput {
                key_code: key_event.code,
            });
        }

        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![format!(" Entity count {:<4} ", self.entity_count)]
    }
}
