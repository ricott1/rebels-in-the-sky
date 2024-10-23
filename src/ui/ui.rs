use super::button::Button;
use super::constants::{UiKey, UiStyle};
use super::galaxy_panel::GalaxyPanel;
use super::gif_map::GifMap;
use super::popup_message::PopupMessage;
use super::space_screen::SpaceScreen;
use super::splash_screen::{AudioPlayerState, SplashScreen};
use super::traits::SplitPanel;
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::utils::SwarmPanelEvent;
use super::widgets::default_block;
use super::{
    game_panel::GamePanel, my_team_panel::MyTeamPanel, new_team_screen::NewTeamScreen,
    player_panel::PlayerListPanel, swarm_panel::SwarmPanel, team_panel::TeamListPanel,
    traits::Screen,
};
use crate::audio::music_player::MusicPlayer;
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::world::world::World;
use core::fmt::Debug;
use itertools::Itertools;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Styled};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::vec;
use strum_macros::Display;
use tui_textarea::{CursorMove, TextArea};

const MAX_POPUP_MESSAGES: usize = 8;

#[derive(Debug, Default, Display, PartialEq)]
pub enum UiState {
    #[default]
    Splash,
    NewTeam,
    Main,
    SpaceAdventure,
}

#[derive(Debug, Clone, Copy, Hash, Display, PartialEq)]
pub enum UiTab {
    MyTeam,
    Teams,
    Players,
    Galaxy,
    Games,
    Swarm,
}

#[derive(Debug)]
pub struct Ui {
    state: UiState,
    ui_tabs: Vec<UiTab>,
    pub tab_index: usize,
    debug_view: bool,
    last_update: Instant,
    pub splash_screen: SplashScreen,
    pub new_team_screen: NewTeamScreen,
    pub space_screen: SpaceScreen,
    pub player_panel: PlayerListPanel,
    pub team_panel: TeamListPanel,
    pub game_panel: GamePanel,
    pub swarm_panel: SwarmPanel,
    pub my_team_panel: MyTeamPanel,
    pub galaxy_panel: GalaxyPanel,
    popup_messages: Vec<PopupMessage>,
    popup_input: TextArea<'static>,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
}

impl Ui {
    pub fn new(store_prefix: &str, disable_network: bool) -> Self {
        let gif_map = Arc::new(Mutex::new(GifMap::new()));
        let callback_registry = Arc::new(Mutex::new(CallbackRegistry::new()));

        let splash_screen = SplashScreen::new(
            store_prefix,
            Arc::clone(&callback_registry),
            Arc::clone(&gif_map),
        );
        let player_panel =
            PlayerListPanel::new(Arc::clone(&callback_registry), Arc::clone(&gif_map));
        let team_panel = TeamListPanel::new(Arc::clone(&callback_registry), Arc::clone(&gif_map));
        let game_panel = GamePanel::new(Arc::clone(&callback_registry), Arc::clone(&gif_map));
        let swarm_panel = SwarmPanel::new(Arc::clone(&callback_registry));
        let my_team_panel = MyTeamPanel::new(Arc::clone(&callback_registry), Arc::clone(&gif_map));
        let new_team_screen =
            NewTeamScreen::new(Arc::clone(&callback_registry), Arc::clone(&gif_map));
        let galaxy_panel = GalaxyPanel::new(Arc::clone(&callback_registry), Arc::clone(&gif_map));

        let mut ui_tabs = vec![];

        ui_tabs.push(UiTab::MyTeam);
        ui_tabs.push(UiTab::Teams);
        ui_tabs.push(UiTab::Players);
        ui_tabs.push(UiTab::Galaxy);
        ui_tabs.push(UiTab::Games);

        if !disable_network {
            ui_tabs.push(UiTab::Swarm);
        }

        let space_screen = SpaceScreen::new(Arc::clone(&callback_registry));

        Self {
            state: UiState::default(),
            ui_tabs,
            tab_index: 0,
            debug_view: false,
            last_update: Instant::now(),
            splash_screen,
            new_team_screen,
            space_screen,
            player_panel,
            team_panel,
            game_panel,
            swarm_panel,
            my_team_panel,
            galaxy_panel,
            popup_input: TextArea::default(),
            popup_messages: vec![],
            callback_registry,
        }
    }

    pub fn push_popup(&mut self, popup_message: PopupMessage) {
        self.popup_messages.push(popup_message);
        if self.popup_messages.len() >= MAX_POPUP_MESSAGES {
            for index in 0..self.popup_messages.len() {
                if self.popup_messages[index].is_skippable() {
                    self.popup_messages.remove(index);
                    break;
                }
            }
        }
    }

    pub fn close_popup(&mut self) {
        self.popup_messages.remove(0);
    }

    pub fn set_state(&mut self, state: UiState) {
        self.state = state;
    }

    pub fn toggle_data_view(&mut self) {
        self.debug_view = !self.debug_view;
    }

    fn get_active_screen(&self) -> &dyn Screen {
        match self.state {
            UiState::Splash => &self.splash_screen,
            UiState::NewTeam => &self.new_team_screen,
            UiState::Main => match self.ui_tabs[self.tab_index] {
                UiTab::MyTeam => &self.my_team_panel,
                UiTab::Teams => &self.team_panel,
                UiTab::Players => &self.player_panel,
                UiTab::Galaxy => &self.galaxy_panel,
                UiTab::Games => &self.game_panel,
                UiTab::Swarm => &self.swarm_panel,
            },
            UiState::SpaceAdventure => &self.space_screen,
        }
    }

    pub fn get_active_panel(&mut self) -> Option<&mut dyn SplitPanel> {
        match self.state {
            UiState::Splash => None,
            UiState::NewTeam => Some(&mut self.new_team_screen),
            _ => match self.ui_tabs[self.tab_index] {
                UiTab::MyTeam => Some(&mut self.my_team_panel),
                UiTab::Teams => Some(&mut self.team_panel),
                UiTab::Players => Some(&mut self.player_panel),
                UiTab::Galaxy => Some(&mut self.galaxy_panel),
                UiTab::Games => Some(&mut self.game_panel),
                UiTab::Swarm => Some(&mut self.swarm_panel),
            },
        }
    }

    fn get_active_screen_mut(&mut self) -> &mut dyn Screen {
        match self.state {
            UiState::Splash => &mut self.splash_screen,
            UiState::NewTeam => &mut self.new_team_screen,
            UiState::Main => match self.ui_tabs[self.tab_index] {
                UiTab::MyTeam => &mut self.my_team_panel,
                UiTab::Teams => &mut self.team_panel,
                UiTab::Players => &mut self.player_panel,
                UiTab::Galaxy => &mut self.galaxy_panel,
                UiTab::Games => &mut self.game_panel,
                UiTab::Swarm => &mut self.swarm_panel,
            },
            UiState::SpaceAdventure => &mut self.space_screen,
        }
    }

    pub fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        world: &World,
    ) -> Option<UiCallback> {
        match key_event.code {
            UiKey::ESC => {
                return Some(UiCallback::PromptQuit);
            }

            UiKey::UI_DEBUG_MODE => {
                return Some(UiCallback::ToggleUiDebugMode);
            }

            UiKey::NEXT_TAB if self.state == UiState::Main => {
                self.next_tab();
                None
            }

            UiKey::PREVIOUS_TAB if self.state == UiState::Main => {
                self.previous_tab();
                None
            }
            _ => {
                if self.popup_messages.len() > 0 {
                    return self.popup_messages[0].consumes_input(&mut self.popup_input, key_event);
                }
                self.popup_input.move_cursor(CursorMove::End);
                self.popup_input.delete_line_by_head();

                if let Some(callback) = self
                    .get_active_screen_mut()
                    .handle_key_events(key_event, world)
                {
                    return Some(callback);
                }

                self.callback_registry
                    .lock()
                    .unwrap()
                    .handle_keyboard_event(&key_event.code)
            }
        }
    }

    pub fn handle_mouse_events(
        &mut self,
        mouse_event: crossterm::event::MouseEvent,
    ) -> Option<UiCallback> {
        self.callback_registry
            .lock()
            .unwrap()
            .set_hovering(mouse_event);
        self.callback_registry
            .lock()
            .unwrap()
            .handle_mouse_event(&mouse_event)
    }

    pub(super) fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % self.ui_tabs.len();
    }

    pub(super) fn previous_tab(&mut self) {
        self.tab_index = (self.tab_index + self.ui_tabs.len() - 1) % self.ui_tabs.len();
    }

    pub fn update(&mut self, world: &World, audio_player: Option<&MusicPlayer>) -> AppResult<()> {
        self.callback_registry.lock().unwrap().clear();
        match self.state {
            UiState::Splash => {
                // This is only to get a nice view in the splash screen
                let audio_state =
                    if audio_player.is_some() && audio_player.as_ref().unwrap().is_playing() {
                        AudioPlayerState::Playing
                    } else if audio_player.is_some() {
                        AudioPlayerState::Paused
                    } else {
                        AudioPlayerState::Disabled
                    };
                self.splash_screen.set_audio_player_state(audio_state);
                self.splash_screen.update(world)?
            }
            UiState::NewTeam => self.new_team_screen.update(world)?,
            UiState::Main => {
                // Update panels. Can we get away updating only the active one?
                // self.get_active_screen_mut().update(world)?;
                self.my_team_panel.update(world)?;
                self.team_panel.update(world)?;
                self.player_panel.update(world)?;
                self.game_panel.update(world)?;
                self.galaxy_panel.update(world)?;
            }
            UiState::SpaceAdventure => self.space_screen.update(world)?,
        }

        Ok(())
    }

    /// Renders the user interface widgets.
    pub fn render(&mut self, frame: &mut Frame, world: &World, audio_player: Option<&MusicPlayer>) {
        self.callback_registry.lock().unwrap().clear();
        if self.popup_messages.len() > 0 {
            self.callback_registry.lock().unwrap().set_max_layer(1);
        }
        let area = frame.area();
        let split = Layout::vertical([
            Constraint::Min(6),    // body
            Constraint::Length(1), // footer
            Constraint::Length(1), // hover text
        ])
        .split(area);

        // Render footer
        // We render the footer first because hover text is displayed in the footer (and thus must overwrite it)
        self.render_footer(frame, world, audio_player, split[1]);

        // render selected tab
        let render_result = match self.state {
            UiState::Splash => self
                .splash_screen
                .render(frame, world, split[0], self.debug_view),
            UiState::NewTeam => {
                self.new_team_screen
                    .render(frame, world, split[0], self.debug_view)
            }
            UiState::Main => {
                // Render tabs at top
                let tab_main_split = Layout::vertical([
                    Constraint::Length(3), // tabs
                    Constraint::Min(3),    // panel
                ])
                .split(split[0]);

                let debug_view = self.debug_view;
                let active_render = self.get_active_screen_mut().render(
                    frame,
                    world,
                    tab_main_split[1],
                    debug_view,
                );

                let mut constraints = [Constraint::Length(16)].repeat(self.ui_tabs.len());
                constraints.push(Constraint::Min(0));
                let tab_split = Layout::horizontal(constraints).split(tab_main_split[0]);

                for (idx, &tab) in self.ui_tabs.iter().enumerate() {
                    let tab_name = if tab == UiTab::MyTeam {
                        world
                            .get_own_team()
                            .expect("Own team should be set if rendering main page")
                            .name
                            .clone()
                    } else {
                        tab.to_string()
                    };
                    let mut button = Button::no_box(
                        tab_name.into(),
                        UiCallback::SetUiTab {
                            ui_tab: self.ui_tabs[idx],
                        },
                        Arc::clone(&self.callback_registry),
                    )
                    .set_hover_style(UiStyle::HIGHLIGHT);

                    if idx == self.tab_index {
                        button = button
                            .set_style(UiStyle::SELECTED)
                            .set_hover_style(UiStyle::SELECTED);
                    }

                    frame.render_widget(button, tab_split[idx]);
                }

                frame.render_widget(default_block(), tab_main_split[0]);

                active_render
            }
            UiState::SpaceAdventure => {
                self.space_screen
                    .render(frame, world, split[0], self.debug_view)
            }
        };

        if let Err(err) = render_result {
            let event = SwarmPanelEvent {
                timestamp: Tick::now(),
                peer_id: None,
                text: format!("Render error\n{}", err.to_string()),
            };
            self.swarm_panel.push_log_event(event);
        }

        if let Err(err) = self.render_popup_messages(frame, area) {
            let event = SwarmPanelEvent {
                timestamp: Tick::now(),
                peer_id: None,
                text: format!("Popup render error\n{}", err.to_string()),
            };
            self.swarm_panel.push_log_event(event);
        }
        self.last_update = Instant::now();
    }

    fn render_popup_messages(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        // Render popup message
        if self.popup_messages.len() > 0 {
            self.popup_messages[0].render(
                frame,
                area,
                &mut self.popup_input,
                &self.callback_registry,
            )?;
        }
        Ok(())
    }

    pub fn switch_to(&mut self, tab: UiTab) {
        for i in 0..self.ui_tabs.len() {
            if self.ui_tabs[i] == tab {
                self.tab_index = i;
                return;
            }
        }
    }

    fn render_footer(
        &self,
        frame: &mut Frame,
        world: &World,
        audio_player: Option<&MusicPlayer>,
        area: Rect,
    ) {
        let split = Layout::horizontal([
            Constraint::Min(50),
            Constraint::Length(20),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(26),
        ])
        .split(area);

        let mut spans = vec![" Esc ".to_string(), " Quit ".to_string()];

        if !self.debug_view && self.state == UiState::Main {
            spans.extend(vec![
                format!(" {} ", UiKey::PREVIOUS_TAB.to_string()),
                " Previous tab ".to_string(),
                format!(" {} ", UiKey::NEXT_TAB.to_string()),
                " Next tab ".to_string(),
            ]);
        }

        let extra_spans = if self.debug_view {
            let fps = (1.0 / self.last_update.elapsed().as_secs_f64()).round() as u32;
            let world_size = world.serialized_size / 1024;

            let mut spans = vec![
                format!("FPS {:>4} ", fps),
                format!(" World size {:06} kb ", world_size),
                format!(" Seed {} ", world.seed),
                format!(
                    " Screen size {}x{} ",
                    frame.area().width,
                    frame.area().height
                ),
            ];
            if world.has_own_team() {
                spans.push(format!(
                    " New FA in {} ",
                    world.next_free_pirates_refresh().formatted()
                ));
            }

            spans
        } else {
            self.get_active_screen().footer_spans()
        };
        spans.extend(extra_spans);

        let styles = [
            Style::default().bg(Color::Gray).fg(Color::DarkGray),
            Style::default().fg(Color::DarkGray),
        ];

        frame.render_widget(
            Line::from(
                spans
                    .iter()
                    .enumerate()
                    .map(|(idx, content)| Span::styled(content, styles[idx % 2]))
                    .collect_vec(),
            )
            .left_aligned(),
            split[0],
        );

        if let Some(audio_player) = &audio_player {
            frame.render_widget(
                Button::no_box(
                    format!(
                        " {}: Turn radio {} ",
                        UiKey::TOGGLE_AUDIO.to_string(),
                        if audio_player.is_playing() {
                            "off"
                        } else {
                            "on "
                        }
                    )
                    .into(),
                    UiCallback::ToggleAudio,
                    Arc::clone(&self.callback_registry),
                )
                .set_hotkey(UiKey::TOGGLE_AUDIO),
                split[1],
            );

            frame.render_widget(
                Button::no_box(
                    format!(" {} ", UiKey::PREVIOUS_RADIO.to_string(),).into(),
                    UiCallback::PreviousRadio,
                    Arc::clone(&self.callback_registry),
                )
                .set_hotkey(UiKey::PREVIOUS_RADIO),
                split[2],
            );

            frame.render_widget(
                Button::no_box(
                    format!(" {} ", UiKey::NEXT_RADIO.to_string(),).into(),
                    UiCallback::NextRadio,
                    Arc::clone(&self.callback_registry),
                )
                .set_hotkey(UiKey::NEXT_RADIO),
                split[3],
            );
            if audio_player.is_playing() {
                if let Some(currently_playing) = audio_player.currently_playing() {
                    frame.render_widget(Paragraph::new(format!(" {currently_playing} ")), split[4]);
                }
            }
        }
    }
}
