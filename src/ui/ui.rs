use super::button::Button;
use super::constants::{PrintableKeyCode, UiKey, UiStyle, UiText};
use super::galaxy_panel::GalaxyPanel;
use super::gif_map::GifMap;
use super::splash_screen::{AudioPlayerState, SplashScreen};
use super::traits::SplitPanel;
use super::ui_callback::{CallbackRegistry, UiCallbackPreset};
use super::widgets::{default_block, popup_rect};
use super::{
    game_panel::GamePanel, my_team_panel::MyTeamPanel, new_team_screen::NewTeamScreen,
    player_panel::PlayerListPanel, swarm_panel::SwarmPanel, team_panel::TeamListPanel,
    traits::Screen,
};
use crate::audio::{self};
use crate::types::{AppResult, SystemTimeTick, Tick};
use crate::world::world::World;
use core::fmt::Debug;
use crossterm::event::KeyCode;
use ratatui::layout::{Margin, Rect};
use ratatui::prelude::Alignment;
use ratatui::style::{Color, Style, Styled};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::vec;
use strum_macros::Display;

const MAX_POPUP_MESSAGES: usize = 8;

#[derive(Debug, Default, Display, PartialEq)]
pub enum UiState {
    #[default]
    Splash,
    NewTeam,
    Main,
}

#[derive(Debug, Clone, Copy, Eq, Hash, Display, PartialEq)]
pub enum UiTab {
    MyTeam,
    Team,
    Player,
    Galaxy,
    Game,
    Swarm,
}

#[derive(Debug, Display, Clone)]
pub enum PopupMessage {
    Error(String, Tick),
    Ok(String, Tick),
}

#[derive()]
pub struct Ui {
    state: UiState,
    ui_tabs: Vec<UiTab>,
    pub tab_index: usize,
    data_view: bool,
    last_update: Instant,
    pub splash_screen: SplashScreen,
    pub new_team_screen: NewTeamScreen,
    pub audio_player: Option<audio::MusicPlayer>,
    pub player_panel: PlayerListPanel,
    pub team_panel: TeamListPanel,
    pub game_panel: GamePanel,
    pub swarm_panel: SwarmPanel,
    pub my_team_panel: MyTeamPanel,
    pub galaxy_panel: GalaxyPanel,
    popup_messages: Vec<PopupMessage>,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new(false, false)
    }
}
unsafe impl Send for audio::MusicPlayer {}

impl Ui {
    pub fn new(disable_network: bool, disable_audio: bool) -> Self {
        let gif_map = Arc::new(Mutex::new(GifMap::new()));
        let callback_registry = Arc::new(Mutex::new(CallbackRegistry::new()));

        let splash_screen = SplashScreen::new(Arc::clone(&callback_registry), Arc::clone(&gif_map));
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
        ui_tabs.push(UiTab::Team);
        ui_tabs.push(UiTab::Player);
        ui_tabs.push(UiTab::Galaxy);
        ui_tabs.push(UiTab::Game);

        if !disable_network {
            ui_tabs.push(UiTab::Swarm);
        }

        let audio_player = if disable_audio {
            None
        } else {
            let audio_player = audio::MusicPlayer::new();
            audio_player.ok()
        };

        Self {
            state: UiState::default(),
            ui_tabs,
            tab_index: 0,
            data_view: false,
            last_update: Instant::now(),
            splash_screen,
            new_team_screen,
            audio_player,
            player_panel,
            team_panel,
            game_panel,
            swarm_panel,
            my_team_panel,
            galaxy_panel,
            popup_messages: vec![],
            callback_registry,
        }
    }

    pub fn set_popup(&mut self, message: PopupMessage) {
        self.popup_messages.push(message);
        if self.popup_messages.len() == MAX_POPUP_MESSAGES {
            self.popup_messages.remove(0);
        }
    }

    pub fn close_popup(&mut self) {
        self.popup_messages.remove(0);
    }

    pub fn set_state(&mut self, state: UiState) {
        self.state = state;
    }

    pub fn toggle_audio_player(&mut self) {
        if self.audio_player.is_some() {
            self.audio_player.as_mut().unwrap().toggle();
        }
    }

    pub fn next_audio_sample(&mut self) {
        if self.audio_player.is_some() {
            self.audio_player.as_mut().unwrap().next();
        }
    }

    pub fn previous_audio_sample(&mut self) {
        if self.audio_player.is_some() {
            self.audio_player.as_mut().unwrap().previous();
        }
    }

    fn get_active_screen(&self) -> &dyn Screen {
        match self.state {
            UiState::Splash => &self.splash_screen,
            UiState::NewTeam => &self.new_team_screen,
            _ => match self.ui_tabs[self.tab_index] {
                UiTab::MyTeam => &self.my_team_panel,
                UiTab::Team => &self.team_panel,
                UiTab::Player => &self.player_panel,
                UiTab::Galaxy => &self.galaxy_panel,
                UiTab::Game => &self.game_panel,
                UiTab::Swarm => &self.swarm_panel,
            },
        }
    }

    pub fn get_active_panel(&mut self) -> Option<&mut dyn SplitPanel> {
        match self.state {
            UiState::Splash => None,
            UiState::NewTeam => Some(&mut self.new_team_screen),
            _ => match self.ui_tabs[self.tab_index] {
                UiTab::MyTeam => Some(&mut self.my_team_panel),
                UiTab::Team => Some(&mut self.team_panel),
                UiTab::Player => Some(&mut self.player_panel),
                UiTab::Galaxy => Some(&mut self.galaxy_panel),
                UiTab::Game => Some(&mut self.game_panel),
                UiTab::Swarm => Some(&mut self.swarm_panel),
            },
        }
    }

    fn get_active_screen_mut(&mut self) -> &mut dyn Screen {
        match self.state {
            UiState::Splash => &mut self.splash_screen,
            UiState::NewTeam => &mut self.new_team_screen,
            _ => match self.ui_tabs[self.tab_index] {
                UiTab::MyTeam => &mut self.my_team_panel,
                UiTab::Team => &mut self.team_panel,
                UiTab::Player => &mut self.player_panel,
                UiTab::Galaxy => &mut self.galaxy_panel,
                UiTab::Game => &mut self.game_panel,
                UiTab::Swarm => &mut self.swarm_panel,
            },
        }
    }

    pub fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
    ) -> Option<UiCallbackPreset> {
        match key_event.code {
            UiKey::DATA_VIEW => {
                self.data_view = !self.data_view;
                None
            }
            UiKey::MUSIC_TOGGLE => {
                self.toggle_audio_player();
                None
            }
            UiKey::MUSIC_NEXT => {
                self.next_audio_sample();
                None
            }
            UiKey::MUSIC_PREVIOUS => {
                self.previous_audio_sample();
                None
            }

            UiKey::NEXT_TAB => {
                self.next_tab();
                None
            }
            UiKey::PREVIOUS_TAB => {
                self.previous_tab();
                None
            }
            _ => {
                if self.popup_messages.len() > 0 {
                    if key_event.code == KeyCode::Enter {
                        self.close_popup();
                    }
                    return None;
                }
                self.get_active_screen_mut().handle_key_events(key_event)
            }
        }
    }

    pub fn handle_mouse_events(
        &mut self,
        mouse_event: crossterm::event::MouseEvent,
    ) -> Option<UiCallbackPreset> {
        self.callback_registry
            .lock()
            .unwrap()
            .set_hovering(mouse_event);
        self.callback_registry
            .lock()
            .unwrap()
            .handle_event(mouse_event)
    }

    pub(super) fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % self.ui_tabs.len();
    }

    pub(super) fn previous_tab(&mut self) {
        self.tab_index = (self.tab_index + self.ui_tabs.len() - 1) % self.ui_tabs.len();
    }

    pub fn update(&mut self, world: &World) -> AppResult<()> {
        self.callback_registry.lock().unwrap().clear();
        match self.state {
            UiState::Splash => {
                // This is only to get a nice view in the splash screen
                let audio_state = if self.audio_player.is_some()
                    && self.audio_player.as_ref().unwrap().is_playing
                {
                    AudioPlayerState::Playing
                } else if self.audio_player.is_some() {
                    AudioPlayerState::Paused
                } else {
                    AudioPlayerState::Disabled
                };
                self.splash_screen.set_audio_player_state(audio_state);
                self.splash_screen.update(world)?
            }
            UiState::NewTeam => self.new_team_screen.update(world)?,
            _ => {
                // Update panels
                self.my_team_panel.update(world)?;
                self.team_panel.update(world)?;
                self.player_panel.update(world)?;
                self.game_panel.update(world)?;
                self.galaxy_panel.update(world)?;
            }
        }

        if self.audio_player.is_some() {
            self.audio_player.as_mut().unwrap().check_if_next();
        }
        Ok(())
    }

    /// Renders the user interface widgets.
    pub fn render(&mut self, frame: &mut Frame, world: &World) {
        self.callback_registry.lock().unwrap().clear();
        let area = frame.size();
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),    // body
                Constraint::Length(2), //footer
            ])
            .split(area);

        // render selected tab
        let render_result = match self.state {
            UiState::Splash => self.splash_screen.render(frame, world, split[0]),
            UiState::NewTeam => self.new_team_screen.render(frame, world, split[0]),
            _ => {
                // Render tabs at top
                let tab_main_split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // tabs
                        Constraint::Min(3),    // panel
                    ])
                    .split(split[0]);

                let active_render =
                    self.get_active_screen_mut()
                        .render(frame, world, tab_main_split[1]);

                let mut constraints = [Constraint::Length(12)].repeat(self.ui_tabs.len());
                constraints.push(Constraint::Min(1));
                let tab_split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(constraints)
                    .split(tab_main_split[0]);

                for idx in 0..self.ui_tabs.len() {
                    let mut button = Button::no_box(
                        format!("{:^}", self.ui_tabs[idx].to_string()),
                        UiCallbackPreset::SetUiTab {
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
        };
        if render_result.is_err() {
            self.set_popup(PopupMessage::Error(
                format!("Render error\n{}", render_result.err().unwrap().to_string()),
                Tick::now(),
            ));
        }

        // Render footer
        frame.render_widget(self.footer(world), split[1]);
        self.render_popup(frame, area);
        self.last_update = Instant::now();
    }

    fn render_popup(&mut self, frame: &mut Frame, area: Rect) {
        // Render popup message
        if self.popup_messages.len() > 0 {
            let popup_rect = popup_rect(area);
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), //header
                    Constraint::Min(3),    //message
                    Constraint::Length(3), //button
                ])
                .split(popup_rect.inner(&Margin {
                    vertical: 1,
                    horizontal: 1,
                }));

            frame.render_widget(Clear, popup_rect);
            frame.render_widget(default_block(), popup_rect);

            let button = Button::new(
                UiText::YES.into(),
                UiCallbackPreset::CloseUiPopup,
                Arc::clone(&self.callback_registry),
            );

            frame.render_widget(
                button,
                split[2].inner(&Margin {
                    vertical: 0,
                    horizontal: 8,
                }),
            );
            let popup_message = self.popup_messages[0].clone();
            match popup_message {
                PopupMessage::Ok(message, tick) => {
                    frame.render_widget(
                        Paragraph::new(format!("Message: {}", tick.formatted_as_date()))
                            .block(default_block().border_style(UiStyle::OK))
                            .alignment(Alignment::Center),
                        split[0],
                    );
                    frame.render_widget(
                        Paragraph::new(message)
                            .alignment(Alignment::Center)
                            .wrap(Wrap { trim: true }),
                        split[1].inner(&Margin {
                            horizontal: 1,
                            vertical: 1,
                        }),
                    );
                }
                PopupMessage::Error(message, tick) => {
                    frame.render_widget(
                        Paragraph::new(format!("Error: {}", tick.formatted_as_date()))
                            .block(default_block().border_style(UiStyle::ERROR))
                            .alignment(Alignment::Center),
                        split[0],
                    );
                    frame.render_widget(
                        Paragraph::new(message)
                            .alignment(Alignment::Center)
                            .wrap(Wrap { trim: true }),
                        split[1].inner(&Margin {
                            horizontal: 1,
                            vertical: 1,
                        }),
                    );
                }
            }
        }
    }

    pub fn switch_to(&mut self, tab: UiTab) {
        for i in 0..self.ui_tabs.len() {
            if self.ui_tabs[i] == tab {
                self.tab_index = i;
                return;
            }
        }
    }

    fn footer(&mut self, world: &World) -> Paragraph {
        let mut spans = vec![
            Span::styled(
                " Esc ",
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Quit ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {} ", UiKey::PREVIOUS_TAB.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Previous tab ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {} ", UiKey::NEXT_TAB.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ),
            Span::styled(" Next tab ", Style::default().fg(Color::DarkGray)),
        ];

        if self.audio_player.is_some() {
            spans.push(Span::styled(
                format!(" {} ", UiKey::MUSIC_TOGGLE.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                format!(
                    " Toggle music: {} ",
                    if self.audio_player.is_some() && self.audio_player.as_ref().unwrap().is_playing
                    {
                        "ON "
                    } else {
                        "OFF"
                    },
                ),
                Style::default().fg(Color::DarkGray),
            ));

            spans.push(Span::styled(
                format!(" {} ", UiKey::MUSIC_PREVIOUS.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                format!(" Previous "),
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                format!(" {} ", UiKey::MUSIC_NEXT.to_string()),
                Style::default().bg(Color::Gray).fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                format!(" Next "),
                Style::default().fg(Color::DarkGray),
            ));
        }

        let extra_spans = if self.data_view {
            let fps = (1.0 / self.last_update.elapsed().as_secs_f64()).round() as u32;
            let world_size = world.serialized_size / 1024;

            let mut spans = vec![
                Span::styled(
                    format!("FPS {:03} ", fps),
                    Style::default().bg(Color::Gray).fg(Color::DarkGray),
                ),
                Span::styled(
                    format!(" World size {:06} kb ", world_size),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!(" Seed {} ", world.seed),
                    Style::default().bg(Color::Gray).fg(Color::DarkGray),
                ),
            ];
            if world.has_own_team() {
                spans.push(Span::styled(
                    format!(
                        " FA refresh in {} ",
                        world.next_free_agents_refresh().formatted()
                    ),
                    Style::default().fg(Color::DarkGray),
                ))
            }
            if let Some(audio_player) = &self.audio_player {
                if audio_player.is_playing {
                    if let Some(currently_playing) = audio_player.currently_playing() {
                        spans.push(Span::styled(
                            format!(" Playing: {} ", currently_playing.title),
                            Style::default().bg(Color::Gray).fg(Color::DarkGray),
                        ));
                    }
                }
            }
            spans
        } else {
            self.get_active_screen().footer_spans()
        };
        spans.extend(extra_spans);

        Paragraph::new(Line::from(spans)).alignment(Alignment::Center)
    }
}
