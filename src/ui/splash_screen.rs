use super::button::RadioButton;
use super::gif_map::GifMap;
use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::utils::big_text;
use super::{
    traits::{Screen, SplitPanel},
    widgets::default_block,
};
use crate::store::world_file_data;
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::world::constants::{DEBUG_TIME_MULTIPLIER, SOL_ID};
use crate::{store::world_exists, world::world::World};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use rand::seq::SliceRandom;
use ratatui::layout::Margin;
use ratatui::{
    prelude::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Paragraph, Wrap},
    Frame,
};
use std::sync::{Arc, Mutex};

use std::vec;
const TITLE_WIDTH: u16 = 71;
const BUTTON_WIDTH: u16 = 36;

#[derive(Debug, PartialEq)]
pub enum AudioPlayerState {
    Playing,
    Paused,
    Disabled,
}

#[derive(Debug)]
pub struct SplashScreen {
    index: usize,
    title: Paragraph<'static>,
    quote: &'static str,
    selection_text: Vec<String>,
    tick: usize,
    can_load_world: bool,
    audio_player_state: AudioPlayerState,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    gif_map: Arc<Mutex<GifMap>>,
}

const QUOTES: [&'static str;14] = [
    " “What cannot be destroyed can, nonetheless, be diverted, frozen, transformed, and gradually deprived of its substance - which in the case of states, is ultimately their capacity to inspire terror.” - D. Graeber",
    " “Aber der Staat lügt in allen Zungen des Guten und Bösen; und was er auch redet, er lügt—und was er auch hat, gestohlen hat er's.” - F. Nietzsche",
    " “That is what I have always understood to be the essence of anarchism: the conviction that the burden of proof has to be placed on authority, and that it should be dismantled if that burden cannot be met.” - N. Chomsky",
    " “To make a thief, make an owner; to create crime, create laws.” - U. K. Le Guin",
    " “There, did you think to kill me? There's no flesh or blood within this cloak to kill. There's only an idea. Ideas are bulletproof.” - A. Moore",
    " “The state calls its own violence law, but that of the individual, crime.” - M. Stirner",
    " “Certo bisogna farne di strada Da una ginnastica d’obbedienza Fino ad un gesto molto più umano Che ti dia il senso della violenza
    Però bisogna farne altrettanta Per diventare così coglioni Da non riuscire più a capire Che non ci sono poteri buoni.” - F. De Andre'",
    " “Erano dei re… e i re si decapitano.” - A. Barbero",
    " “The state is a condition, a certain relationship between human beings, a mode of behaviour; we destroy it by contracting other relationships, by behaving differently toward one another…” - G. Orwell",
    " “Underneath the gaze of Orion's belt, where the Sea of Tranquility meets the edge of twilight, lies a hidden trove of wisdom, forgotten by many, coveted by those in the know. It holds the keys to untold power.” - Anonymous",
    " “Dilige, et quod vis fac.” - Aurelius Augustinus Hipponensis",
    " “The only way to deal with an unfree world is to become so absolutely free that your very existence is an act of rebellion.” - A. Camus",
    " “He who can destroy a thing, controls a thing.” - F. Herbert",
    " “What's law? Control? Laws filter chaos and what drips through? Serenity? [..] Don't look too closely at the law. Do, and you'll find the rationalised interpretations, the legal casuistry, the precedents of convenience. You'll find the serenity, which is just another word for death.” - F. Herbert"

    ];

const TITLE: [&'static str; 13] = [
    "            ██████╗ ███████╗██████╗ ███████╗██╗     ███████╗           ",
    "            ██╔══██╗██╔════╝██╔══██╗██╔════╝██║     ██╔════╝           ",
    "            ██████╔╝█████╗  ██████╔╝█████╗  ██║     ███████╗           ",
    "            ██╔══██╗██╔══╝  ██╔══██╗██╔══╝  ██║     ╚════██║           ",
    "            ██║  ██║███████╗██████╔╝███████╗███████╗███████║           ",
    "            ╚═╝  ╚═╝╚══════╝╚═════╝ ╚══════╝╚══════╝╚══════╝           ",
    "                                                                       ",
    "██╗███╗   ██╗    ████████╗██╗  ██╗███████╗    ███████╗██╗  ██╗██╗   ██╗",
    "██║████╗  ██║    ╚══██╔══╝██║  ██║██╔════╝    ██╔════╝██║ ██╔╝╚██╗ ██╔╝",
    "██║██╔██╗ ██║       ██║   ███████║█████╗      ███████╗█████╔╝  ╚████╔╝ ",
    "██║██║╚██╗██║       ██║   ██╔══██║██╔══╝      ╚════██║██╔═██╗   ╚██╔╝  ",
    "██║██║ ╚████║       ██║   ██║  ██║███████╗    ███████║██║  ██╗   ██║   ",
    "╚═╝╚═╝  ╚═══╝       ╚═╝   ╚═╝  ╚═╝╚══════╝    ╚══════╝╚═╝  ╚═╝   ╚═╝   ",
];
const VERSION: &str = env!("CARGO_PKG_VERSION");

impl SplashScreen {
    pub fn new(
        store_prefix: &str,
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        gif_map: Arc<Mutex<GifMap>>,
    ) -> Self {
        let mut selection_text = vec![];
        let can_load_world: bool;
        let mut continue_text = "Continue".to_string();

        if world_exists(store_prefix) {
            if let Ok(continue_data) = world_file_data(store_prefix) {
                if let Ok(last_modified) = continue_data.modified() {
                    continue_text = format!(
                        "Continue: {}",
                        Tick::from_system_time(last_modified).formatted_as_date()
                    );
                }
            }
            can_load_world = true;
        } else {
            can_load_world = false;
        }
        selection_text.push(continue_text);
        selection_text.push("New Game".to_string());
        selection_text.push("Music: On ".to_string());
        selection_text.push("Quit".to_string());

        let quote = QUOTES
            .choose(&mut rand::thread_rng())
            .expect("There should be a quote");
        let index = if can_load_world { 0 } else { 1 };
        let title = big_text(&TITLE);

        Self {
            index,
            title,
            quote,
            selection_text,
            tick: 0,
            can_load_world,
            audio_player_state: AudioPlayerState::Disabled,
            callback_registry,
            gif_map,
        }
    }

    fn get_ui_preset_at_index(&self, index: usize) -> UiCallbackPreset {
        match index {
            0 => UiCallbackPreset::ContinueGame,
            1 => UiCallbackPreset::NewGame,
            2 => UiCallbackPreset::ToggleAudio,
            _ => UiCallbackPreset::QuitGame,
        }
    }

    pub fn set_audio_player_state(&mut self, state: AudioPlayerState) {
        self.audio_player_state = state;
    }
}

impl Screen for SplashScreen {
    fn update(&mut self, _world: &World) -> AppResult<()> {
        self.tick += 1;
        self.selection_text[2] = if self.audio_player_state == AudioPlayerState::Playing {
            "Music: On ".to_string()
        } else {
            "Music: Off".to_string()
        };
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let split = Layout::vertical([
            Constraint::Length(2),                  //margin
            Constraint::Length(TITLE.len() as u16), //title
            Constraint::Length(3),                  //version
            Constraint::Min(5),                     //body
            Constraint::Length(4),                  // quote
        ])
        .split(area);

        let side_length = if area.width > TITLE_WIDTH {
            (area.width - TITLE_WIDTH) / 2
        } else {
            0
        };
        let title = Layout::horizontal([
            Constraint::Length(side_length),
            Constraint::Length(TITLE_WIDTH),
            Constraint::Length(side_length),
        ])
        .split(split[1]);

        frame.render_widget(self.title.clone(), title[1]);
        frame.render_widget(
            Paragraph::new(format!(
                "Version {} {}",
                VERSION,
                if DEBUG_TIME_MULTIPLIER == 1 {
                    ""
                } else {
                    "DEBUG MODE"
                }
            ))
            .centered(),
            split[2].inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
        );

        let side_width = if area.width > BUTTON_WIDTH {
            (area.width - BUTTON_WIDTH) / 2
        } else {
            0
        };

        let body = Layout::horizontal([
            Constraint::Length(side_width),
            Constraint::Min(12),
            Constraint::Length(side_width),
        ])
        .split(split[3]);

        if let Ok(mut lines) = self.gif_map.lock().unwrap().planet_zoom_in_frame_lines(
            SOL_ID.clone(),
            self.tick / 3,
            world,
        ) {
            let offset = if lines.len() > split[3].height as usize {
                (lines.len() - split[3].height as usize) / 4
            } else {
                0
            };
            lines = lines[offset..offset + split[3].height as usize].to_vec();

            // Apply x-centering
            if lines[0].spans.len() > split[3].width as usize {
                let min_offset = (lines[0].spans.len() - split[3].width as usize) / 2;
                let max_offset = (min_offset + split[3].width as usize).min(lines[0].spans.len());
                for line in lines.iter_mut() {
                    line.spans = line.spans[min_offset..max_offset].to_vec();
                }
            }
            frame.render_widget(Paragraph::new(lines).centered(), split[3]);
        }

        let selection_split = Layout::vertical::<Vec<Constraint>>(
            (0..=self.max_index())
                .map(|_| Constraint::Length(3))
                .collect::<Vec<Constraint>>(),
        )
        .split(body[1]);

        for i in 0..selection_split.len() - 1 {
            let mut button = RadioButton::box_on_hover(
                self.selection_text[i].clone(),
                self.get_ui_preset_at_index(i),
                Arc::clone(&self.callback_registry),
                &mut self.index,
                i,
            );

            // Disable continue button if no world exists
            if i == 0 && self.can_load_world == false {
                button.disable();
            }
            // Disable music button if audio is not supported
            if i == 2 && self.audio_player_state == AudioPlayerState::Disabled {
                button.disable();
            }
            frame.render_widget(button, selection_split[i]);
        }

        frame.render_widget(
            Paragraph::new(self.quote)
                .wrap(Wrap { trim: true })
                .block(default_block()),
            split[4],
        );
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallbackPreset> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            KeyCode::Enter => match self.index {
                // continue
                0 => {
                    return Some(UiCallbackPreset::ContinueGame);
                }
                // new
                1 => {
                    return Some(UiCallbackPreset::NewGame);
                }
                //options
                2 => {
                    return Some(UiCallbackPreset::ToggleAudio);
                }
                //quit
                3 => {
                    return Some(UiCallbackPreset::QuitGame);
                }
                _ => {}
            },
            KeyCode::Char('r') => {
                self.quote = QUOTES
                    .choose(&mut rand::thread_rng())
                    .expect("There should be a quote");
            }

            _ => {}
        }
        None
    }

    fn footer_spans(&self) -> Vec<Span> {
        vec![
            Span::styled(
                " ↑/↓ ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Select option ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " Enter ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Confirm ", Style::default().fg(Color::DarkGray)),
        ]
    }
}

impl SplitPanel for SplashScreen {
    fn index(&self) -> usize {
        self.index
    }

    fn previous_index(&mut self) {
        if self.index < self.max_index() - 1 {
            self.set_index(self.index + 1);
        }
    }

    fn next_index(&mut self) {
        let min_index = if self.can_load_world { 0 } else { 1 };
        if self.index > min_index {
            self.set_index(self.index - 1);
        }
    }

    fn max_index(&self) -> usize {
        self.selection_text.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}
