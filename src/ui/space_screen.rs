use super::constants::BARS_LENGTH;
use super::ui_callback::UiCallback;
use super::utils::img_to_lines;
use super::widgets::get_fuel_spans;
use super::{traits::Screen, ui_callback::CallbackRegistry};
use crate::space_adventure::Body;
use crate::types::AppResult;
use crate::ui::constants::UiKey;
use crate::world::world::World;
use core::fmt::Debug;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::{prelude::Rect, widgets::Paragraph, Frame};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct SpaceScreen {
    tick: usize,
    _callback_registry: Arc<Mutex<CallbackRegistry>>,
}

impl SpaceScreen {
    pub fn new(_callback_registry: Arc<Mutex<CallbackRegistry>>) -> Self {
        Self {
            _callback_registry,
            ..Default::default()
        }
    }
}

impl Screen for SpaceScreen {
    fn update(&mut self, _world: &World) -> AppResult<()> {
        self.tick += 1;

        Ok(())
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        debug_view: bool,
    ) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Min(10), Constraint::Length(1)]).split(area);
        let space = if let Some(space) = &world.space {
            space
        } else {
            return Ok(());
        };

        let mut space_img_lines = img_to_lines(&space.image(debug_view)?);
        space_img_lines.truncate(split[0].height as usize);

        frame.render_widget(Paragraph::new(space_img_lines).centered(), split[0]);

        let info_split = Layout::horizontal([
            Constraint::Length(40),
            Constraint::Length(40),
            Constraint::Min(0),
        ])
        .split(split[1]);

        if let Some(player) = space.get_player() {
            frame.render_widget(
                Paragraph::new(format!(
                    "pos:{:.02},{:.02}  vel:{:.02},{:.02} ",
                    player.position()[0],
                    player.position()[1],
                    player.velocity()[0],
                    player.velocity()[1],
                )),
                info_split[0],
            );

            frame.render_widget(
                Line::from(get_fuel_spans(
                    player.fuel(),
                    player.fuel_capacity(),
                    BARS_LENGTH,
                )),
                info_split[1],
            );
        }

        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<super::ui_callback::UiCallback> {
        return match key_event.code {
            UiKey::MOVE_LEFT => Some(UiCallback::MovePlayerLeft),
            UiKey::MOVE_RIGHT => Some(UiCallback::MovePlayerRight),
            UiKey::MOVE_DOWN => Some(UiCallback::MovePlayerDown),
            UiKey::MOVE_UP => Some(UiCallback::MovePlayerUp),
            _ => None,
        };
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" {} ", UiKey::START_SPACE_ADVENTURE.to_string()),
            " Start/stop secret space adventure ".to_string(),
        ]
    }
}
