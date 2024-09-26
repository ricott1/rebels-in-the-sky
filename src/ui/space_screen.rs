use super::ui_callback::UiCallbackPreset;
use super::utils::img_to_lines;
use super::{traits::Screen, ui_callback::CallbackRegistry};
use crate::types::AppResult;
use crate::ui::constants::UiKey;
use crate::world::world::World;
use core::fmt::Debug;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
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

    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::vertical([Constraint::Min(10), Constraint::Length(1)]).split(area);
        let space = if let Some(space) = &world.space {
            space
        } else {
            return Ok(());
        };

        let space_img_lines = img_to_lines(&space.gif_frame(split[0])?);
        frame.render_widget(Paragraph::new(space_img_lines).centered(), split[0]);

        let player = space.player();
        frame.render_widget(
            Paragraph::new(format!(
                "pos:{:.02},{:.02}  vel:{:.02},{:.02}  acc:{:.02},{:.02}",
                player.position()[0],
                player.position()[1],
                player.velocity()[0],
                player.velocity()[1],
                player.accelleration()[0],
                player.accelleration()[1]
            )),
            split[1],
        );
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<super::ui_callback::UiCallbackPreset> {
        return match key_event.code {
            UiKey::MOVE_LEFT => Some(UiCallbackPreset::MovePlayerLeft),
            UiKey::MOVE_RIGHT => Some(UiCallbackPreset::MovePlayerRight),
            UiKey::MOVE_DOWN => Some(UiCallbackPreset::MovePlayerDown),
            UiKey::MOVE_UP => Some(UiCallbackPreset::MovePlayerUp),
            _ => None,
        };
    }

    fn footer_spans(&self) -> Vec<ratatui::prelude::Span> {
        vec![
            Span::styled(
                format!(" {} ", UiKey::START_SPACE_ADVENTURE.to_string(),),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(
                " Start/stop secret space adventure ",
                Style::default().fg(Color::DarkGray),
            ),
        ]
    }
}
