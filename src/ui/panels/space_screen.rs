use super::traits::{HelpContent, HelpPanel, Screen};
use crate::core::world::World;
use crate::space_adventure::LocalPlayerView;
use crate::types::AppResult;
use crate::ui::constants::BARS_LENGTH;
use crate::ui::renders::{
    get_charge_spans, get_durability_spans, get_fuel_spans, get_storage_spans,
};
use crate::ui::ui_callback::UiCallback;
use crate::ui::ui_frame::UiFrame;
use crate::ui::ui_key;
use crate::ui::utils::{big_text, img_to_lines};
use core::fmt::Debug;
use ratatui::crossterm;
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
    fn tick(&mut self) {
        self.tick += 1;
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
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

        if let Some(view) = space_adventure
            .host_id()
            .and_then(|id| space_adventure.local_view(id))
        {
            render_hud(frame, &view, split[1]);
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
    ) -> Option<UiCallback> {
        if ui_key::space::ALL.contains(&key_event.code) {
            return Some(UiCallback::SpaceAdventurePlayerInput {
                key_code: key_event.code,
            });
        }

        None
    }

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" Autofire {} ", ui_key::space::AUTOFIRE),
            format!(" Shoot {} ", ui_key::space::SHOOT),
            format!(" Toggle shield  {} ", ui_key::space::TOGGLE_SHIELD),
            format!(" Release scraps {} ", ui_key::space::RELEASE_SCRAPS),
            format!(" Return home  {} ", ui_key::space::BACK_TO_BASE),
            format!(" Entity count {:<4} ", self.entity_count),
        ]
    }
}

fn render_hud(frame: &mut UiFrame, view: &LocalPlayerView, area: Rect) {
    let info_split = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .split(area);
    let bars_length = (area.width as usize / 4 - 20).min(BARS_LENGTH);

    frame.render_widget(
        Line::from(get_durability_spans(
            view.durability,
            view.max_durability,
            view.shield_durability,
            view.shield_max_durability,
            bars_length,
        )),
        info_split[0],
    );

    frame.render_widget(
        Line::from(get_charge_spans(
            view.charge,
            view.max_charge,
            view.is_recharging,
            bars_length,
        )),
        info_split[1],
    );

    frame.render_widget(
        Line::from(get_fuel_spans(view.fuel, view.fuel_capacity, bars_length)),
        info_split[2],
    );

    frame.render_widget(
        Line::from(get_storage_spans(
            &view.resources,
            view.storage_capacity,
            bars_length,
        )),
        info_split[3],
    );
}

impl HelpPanel for SpaceScreen {
    fn help_content(&self) -> HelpContent {
        HelpContent {
            description: [
                "Pilot your spaceship through asteroids and hostile ships.",
                "Survive long enough to discover new asteroids.",
            ]
            .join("\n"),
            links: vec![],
            controls: vec![
                Line::from("Controls:"),
                Line::from("  ↑/↓/←/→     Thrust your spaceship"),
                Line::from(format!(
                    "  {}           Toggle autofire",
                    ui_key::space::AUTOFIRE
                )),
                Line::from(format!("  {}           Shoot", ui_key::space::SHOOT)),
                Line::from(format!(
                    "  {}           Toggle shield (drains charge)",
                    ui_key::space::TOGGLE_SHIELD
                )),
                Line::from(format!(
                    "  {}           Release scraps as decoys",
                    ui_key::space::RELEASE_SCRAPS
                )),
                Line::from(format!(
                    "  {}           Return home, ending the adventure",
                    ui_key::space::BACK_TO_BASE
                )),
            ],
        }
    }
}
