use super::constants::BARS_LENGTH;
use super::traits::Screen;
use super::ui_callback::UiCallback;
use super::ui_frame::UiFrame;
use super::utils::{big_text, img_to_lines};
use super::widgets::{get_charge_spans, get_durability_spans, get_fuel_spans, get_storage_spans};
use crate::space_adventure::{ControllableSpaceship, ShooterState};
use crate::types::AppResult;
use crate::ui::constants::UiKey;
use crate::world::world::World;
use core::fmt::Debug;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::{prelude::Rect, widgets::Paragraph};

const CONTROLS: [&'static str; 5] = [
    "      ╔═════╗         ╔═════╗            ╔═════╗                     ",
    "      ║  ↑  ║         ║  a  ║ autofire   ║  s  ║ release scraps      ",
    "╔═════╬═════╬═════╗   ╚═════╝╔═════╗     ╚═════╝╔═════╗              ",
    "║  ←  ║  ↓  ║  →  ║          ║  z  ║ shoot      ║  x  ║ return home  ",
    "╚═════╩═════╩═════╝          ╚═════╝            ╚═════╝              ",
];

#[derive(Debug, Default)]
pub struct SpaceScreen {
    tick: usize,
    entity_count: usize,
    controls: Paragraph<'static>,
}

impl SpaceScreen {
    pub fn new() -> Self {
        Self {
            controls: big_text(&CONTROLS).left_aligned(),
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

            let description: &dyn ControllableSpaceship = player
                .as_trait_ref()
                .expect("Player should implement ControllableSpaceship.");

            frame.render_widget(
                Line::from(get_durability_spans(
                    description.current_durability(),
                    description.durability(),
                    bars_length,
                )),
                info_split[0],
            );

            let is_recharing = match description.shooter_state() {
                ShooterState::Recharging { .. } => true,
                _ => false,
            };

            frame.render_widget(
                Line::from(get_charge_spans(
                    description.charge(),
                    description.max_charge(),
                    is_recharing,
                    bars_length,
                )),
                info_split[1],
            );

            frame.render_widget(
                Line::from(get_fuel_spans(
                    description.fuel(),
                    description.fuel_capacity(),
                    bars_length,
                )),
                info_split[2],
            );

            frame.render_widget(
                Line::from(get_storage_spans(
                    description.resources(),
                    description.storage_capacity(),
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
        return match key_event.code {
            UiKey::SPACE_MOVE_LEFT => Some(UiCallback::SpaceMovePlayerLeft),
            UiKey::SPACE_MOVE_RIGHT => Some(UiCallback::SpaceMovePlayerRight),
            UiKey::SPACE_MOVE_DOWN => Some(UiCallback::SpaceMovePlayerDown),
            UiKey::SPACE_MOVE_UP => Some(UiCallback::SpaceMovePlayerUp),
            UiKey::SPACE_AUTOFIRE => Some(UiCallback::SpaceToggleAutofire),
            UiKey::SPACE_SHOOT => Some(UiCallback::SpaceShoot),
            UiKey::SPACE_BACK_TO_BASE => Some(UiCallback::StopSpaceAdventure),
            UiKey::SPACE_RELEASE_SCRAPS => Some(UiCallback::SpaceReleaseScraps),
            _ => None,
        };
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![format!(" Entity count {:<4} ", self.entity_count)]
    }
}
