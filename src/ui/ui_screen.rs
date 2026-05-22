use super::button::Button;
use super::constants::UiStyle;
use super::galaxy_panel::GalaxyPanel;
use super::popup_message::PopupMessage;
use super::space_screen::SpaceScreen;
use super::splash_screen::SplashScreen;
use super::swarm_panel::SwarmPanel;
use super::traits::SplitPanel;
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::ui_frame::UiFrame;
use super::ui_key;
use super::widgets::{default_block, thick_block};
use super::{
    game_panel::GamePanel, my_team_panel::MyTeamPanel, new_team_screen::NewTeamScreen,
    player_panel::PlayerListPanel, team_panel::TeamListPanel, tournament_panel::TournamentPanel,
    traits::Screen,
};
#[cfg(feature = "audio")]
use crate::audio::music_player::MusicPlayer;
use crate::core::world::World;
use crate::network::types::ChatHistoryEntry;
use crate::types::Tick;
use crate::types::{AppResult, SystemTimeTick};
use crate::ui::space_cove_panel::SpaceCovePanel;
#[cfg(feature = "audio")]
use crate::AudioPlayerState;
use anyhow::Error;
use core::fmt::Debug;
use itertools::Itertools;
use libp2p::PeerId;
use ratatui::crossterm;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Color, Style, Styled, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::{
    layout::{Constraint, Layout},
    Frame,
};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Instant;
use std::vec;
use strum_macros::Display;
use ratatui_textarea::{CursorMove, TextArea};

const MAX_POPUP_MESSAGES: usize = 8;

static CONSTANT_TAB_BUTTONS: LazyLock<HashMap<UiTab, [Button<'static>; 2]>> = LazyLock::new(|| {
    let constant_tabs = [
        UiTab::Crews,
        UiTab::Pirates,
        UiTab::Galaxy,
        UiTab::Games,
        UiTab::Tournaments,
        UiTab::SpaceCoves,
    ];
    constant_tabs
        .into_iter()
        .map(|tab| {
            let callback = UiCallback::SetUiTab { ui_tab: tab };
            let unselected = Button::no_box(tab.to_string(), callback.clone());
            let selected = Button::new(tab.to_string(), callback).selected();
            (tab, [unselected, selected])
        })
        .collect()
});

/// Returns a centered rect ~60% wide / 80% tall, used for the help popup.
/// Falls back to the full screen if it would otherwise be smaller than the
/// preferred minimum (50x20). `clamp(min, max)` would panic when min > max.
fn help_popup_rect(screen_area: Rect) -> Rect {
    let width = (screen_area.width * 60 / 100)
        .max(50)
        .min(screen_area.width);
    let height = (screen_area.height * 80 / 100)
        .max(20)
        .min(screen_area.height);
    let x = screen_area.x + screen_area.width.saturating_sub(width) / 2;
    let y = screen_area.y + screen_area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

/// Renders the standard help body: a description paragraph, a stack of
/// inline-link rows pointing to other tabs, and a controls paragraph.
/// Used by the main-tab panels' `render_help_widget` to avoid layout boilerplate.
pub fn render_help_block(
    frame: &mut UiFrame,
    area: Rect,
    description: Vec<Line<'static>>,
    links: Vec<(&'static str, &'static str, UiTab, &'static str)>,
    controls: Vec<Line<'static>>,
) {
    let desc_h = description.len().max(1) as u16;
    let n_links = links.len() as u16;
    let split = Layout::vertical([
        Constraint::Length(desc_h),
        Constraint::Length(1),       // gap
        Constraint::Length(n_links), // link rows
        Constraint::Length(1),       // gap
        Constraint::Min(5),          // controls
    ])
    .split(area);

    frame.render_widget(Paragraph::new(description), split[0]);

    if n_links > 0 {
        let link_areas = Layout::vertical(vec![Constraint::Length(1); links.len()]).split(split[2]);
        for (i, (prefix, label, ui_tab, suffix)) in links.into_iter().enumerate() {
            render_help_link_line(frame, link_areas[i], prefix, label, ui_tab, suffix);
        }
    }

    frame.render_widget(Paragraph::new(controls), split[4]);
}

/// Renders one help line as `prefix [link] suffix` where `link` is a clickable
/// inline button that switches to the target tab. Designed for help overlays.
pub fn render_help_link_line<'a>(
    frame: &mut UiFrame,
    area: Rect,
    prefix: &'a str,
    label: &'a str,
    ui_tab: UiTab,
    suffix: &'a str,
) {
    let prefix_w = prefix.chars().count() as u16;
    let label_w = label.chars().count() as u16;
    let suffix_w = suffix.chars().count() as u16;
    let split = Layout::horizontal([
        Constraint::Length(prefix_w),
        Constraint::Length(label_w),
        Constraint::Length(suffix_w),
        Constraint::Min(0),
    ])
    .split(area);
    frame.render_widget(Paragraph::new(prefix), split[0]);
    let button = Button::no_box(label, UiCallback::SetUiTab { ui_tab })
        .set_style(UiStyle::HELP_LINK)
        .set_layer(1);
    frame.render_interactive_widget(button, split[1]);
    frame.render_widget(Paragraph::new(suffix), split[2]);
}

#[derive(Debug, Default, Display, PartialEq)]
pub enum UiState {
    #[default]
    Splash,
    #[strum(to_string = "New Team")]
    NewTeam,
    Main,
    #[strum(to_string = "Space Adventure")]
    SpaceAdventure,
}

#[derive(Debug, Clone, Copy, Hash, Display, PartialEq, Eq)]
pub enum UiTab {
    #[strum(to_string = "My Team")]
    MyTeam,
    Crews,
    Pirates,
    Galaxy,
    Games,
    Tournaments,
    #[strum(to_string = "Space Coves")]
    SpaceCoves,
    Swarm,
}

#[derive(Debug)]
pub struct UiScreen {
    pub state: UiState,
    ui_tabs: Vec<UiTab>,
    tab_index: usize,
    debug_view: bool,
    show_help: bool,
    last_render: Instant,
    pub splash_screen: SplashScreen,
    pub new_team_screen: NewTeamScreen,
    pub space_screen: SpaceScreen,
    pub player_panel: PlayerListPanel,
    pub team_panel: TeamListPanel,
    pub game_panel: GamePanel,
    pub tournament_panel: TournamentPanel,
    pub space_cove_panel: SpaceCovePanel,
    pub swarm_panel: SwarmPanel,
    pub my_team_panel: MyTeamPanel,
    pub galaxy_panel: GalaxyPanel,
    popup_messages: Vec<PopupMessage>,
    popup_input: TextArea<'static>,
    inner_registry: CallbackRegistry,
}

impl UiScreen {
    pub fn new(store_prefix: &str, disable_network: bool) -> Self {
        let splash_screen = SplashScreen::new(store_prefix);
        let player_panel = PlayerListPanel::new();
        let team_panel = TeamListPanel::new();
        let game_panel = GamePanel::new();
        let tournament_panel = TournamentPanel::new();
        let space_cove_panel = SpaceCovePanel::new();
        let swarm_panel = SwarmPanel::new();
        let my_team_panel = MyTeamPanel::new();
        let new_team_screen = NewTeamScreen::new();
        let galaxy_panel = GalaxyPanel::new();

        let mut ui_tabs = vec![
            UiTab::MyTeam,
            UiTab::Crews,
            UiTab::Pirates,
            UiTab::SpaceCoves,
            UiTab::Games,
            UiTab::Tournaments,
            UiTab::Galaxy,
        ];

        if !disable_network {
            ui_tabs.push(UiTab::Swarm);
        }

        let space_screen = SpaceScreen::new();

        Self {
            state: UiState::default(),
            ui_tabs,
            tab_index: 0,
            debug_view: false,
            show_help: false,
            last_render: Instant::now(),
            splash_screen,
            new_team_screen,
            space_screen,
            player_panel,
            team_panel,
            game_panel,
            tournament_panel,
            space_cove_panel,
            swarm_panel,
            my_team_panel,
            galaxy_panel,
            popup_input: TextArea::default(),
            popup_messages: vec![],
            inner_registry: CallbackRegistry::new(),
        }
    }

    pub fn push_chat_event(
        &mut self,
        timestamp: Tick,
        peer_id: PeerId,
        author: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.swarm_panel
            .push_chat_event(timestamp, peer_id, author.into(), message.into());
    }

    pub fn push_chat_error_event(&mut self, timestamp: Tick, error: Error) {
        self.swarm_panel.push_chat_error_event(timestamp, error);
    }

    pub fn push_chat_history(&mut self, chat_history: &[ChatHistoryEntry]) {
        self.swarm_panel.push_chat_history(chat_history);
    }

    pub fn push_popup(&mut self, popup_message: PopupMessage) {
        // Avoid pushing twice the same popup
        if let Some(last_popup) = self.popup_messages.last().as_ref() {
            match (&popup_message, last_popup) {
                (
                    PopupMessage::Error { message, .. },
                    PopupMessage::Error {
                        message: l_message, ..
                    },
                ) => {
                    if *message == *l_message {
                        return;
                    }
                }

                (
                    PopupMessage::Ok { message, .. },
                    PopupMessage::Ok {
                        message: l_message, ..
                    },
                ) => {
                    if *message == *l_message {
                        return;
                    }
                }

                (PopupMessage::PromptQuit { .. }, PopupMessage::PromptQuit { .. }) => return,

                _ => {}
            }
        }

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

    pub fn push_popup_to_top(&mut self, popup_message: PopupMessage) {
        self.popup_messages.insert(0, popup_message);
    }

    pub fn close_popup(&mut self) {
        if !self.popup_messages.is_empty() {
            self.popup_messages.remove(0);
        }
    }

    pub const fn close_help(&mut self) {
        self.show_help = false;
    }

    pub fn push_log_event(
        &mut self,
        timestamp: Tick,
        peer_id: Option<PeerId>,
        text: impl Into<String>,
        level: log::Level,
    ) {
        self.swarm_panel
            .push_log_event(timestamp, peer_id, text.into(), level);
    }

    pub const fn set_state(&mut self, state: UiState) {
        self.state = state;
    }

    pub const fn toggle_data_view(&mut self) {
        self.debug_view = !self.debug_view;
    }

    pub fn switch_to(&mut self, tab: UiTab) {
        for i in 0..self.ui_tabs.len() {
            if self.ui_tabs[i] == tab {
                if self.tab_index != i {
                    self.show_help = false;
                }
                self.tab_index = i;
                return;
            }
        }
    }

    fn get_active_screen(&self) -> &dyn Screen {
        match self.state {
            UiState::Splash => &self.splash_screen,
            UiState::NewTeam => &self.new_team_screen,
            UiState::Main => match self.ui_tabs[self.tab_index] {
                UiTab::MyTeam => &self.my_team_panel,
                UiTab::Crews => &self.team_panel,
                UiTab::Pirates => &self.player_panel,
                UiTab::Games => &self.game_panel,
                UiTab::Tournaments => &self.tournament_panel,
                UiTab::Galaxy => &self.galaxy_panel,
                UiTab::SpaceCoves => &self.space_cove_panel,
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
                UiTab::Crews => Some(&mut self.team_panel),
                UiTab::Pirates => Some(&mut self.player_panel),
                UiTab::Games => Some(&mut self.game_panel),
                UiTab::Tournaments => Some(&mut self.tournament_panel),
                UiTab::Galaxy => Some(&mut self.galaxy_panel),
                UiTab::SpaceCoves => Some(&mut self.space_cove_panel),
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
                UiTab::Crews => &mut self.team_panel,
                UiTab::Pirates => &mut self.player_panel,
                UiTab::Games => &mut self.game_panel,
                UiTab::Tournaments => &mut self.tournament_panel,
                UiTab::Galaxy => &mut self.galaxy_panel,
                UiTab::SpaceCoves => &mut self.space_cove_panel,
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
            ui_key::ESC if self.show_help => {
                self.show_help = false;
                None
            }
            ui_key::ESC => {
                if self.state == UiState::Splash || self.state == UiState::NewTeam {
                    return Some(UiCallback::QuitGame);
                }

                let during_space_adventure = world.space_adventure.is_some();

                Some(UiCallback::PushUiPopup {
                    popup_message: PopupMessage::PromptQuit {
                        during_space_adventure,
                        timestamp: Tick::now(),
                    },
                })
            }

            ui_key::UI_DEBUG_MODE if !self.get_active_screen().is_capturing_text() => {
                Some(UiCallback::ToggleUiDebugMode)
            }

            ui_key::HELP
                if self.popup_messages.is_empty()
                    && !self.get_active_screen().is_capturing_text() =>
            {
                self.show_help = !self.show_help;
                None
            }

            ui_key::YES_TO_DIALOG if self.show_help => {
                self.show_help = false;
                None
            }

            ui_key::NEXT_TAB if self.state == UiState::Main && self.popup_messages.is_empty() => {
                self.show_help = false;
                self.next_tab();
                None
            }

            ui_key::PREVIOUS_TAB
                if self.state == UiState::Main && self.popup_messages.is_empty() =>
            {
                self.show_help = false;
                self.previous_tab();
                None
            }
            _ => {
                // Special handling for space screen. It takes precedence over popups.
                if self.state == UiState::SpaceAdventure {
                    if let Some(callback) = self.space_screen.handle_key_events(key_event, world) {
                        return Some(callback);
                    }
                }

                if !self.popup_messages.is_empty() {
                    return self.popup_messages[0].consumes_input(&mut self.popup_input, key_event);
                }
                self.popup_input.move_cursor(CursorMove::End);
                self.popup_input.delete_line_by_head();

                // While help is shown, swallow keyboard events targeted at the panel:
                // closing requires '?' or navigating away, both handled above.
                if self.show_help {
                    return None;
                }

                if let Some(callback) = self
                    .get_active_screen_mut()
                    .handle_key_events(key_event, world)
                {
                    return Some(callback);
                }

                self.inner_registry.handle_keyboard_event(&key_event.code)
            }
        }
    }

    pub fn handle_mouse_events(
        &mut self,
        mouse_event: crossterm::event::MouseEvent,
    ) -> Option<UiCallback> {
        self.inner_registry
            .set_hovering((mouse_event.column, mouse_event.row));

        self.inner_registry.handle_mouse_event(&mouse_event)
    }

    pub(super) const fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % self.ui_tabs.len();
    }

    pub(super) const fn previous_tab(&mut self) {
        self.tab_index = (self.tab_index + self.ui_tabs.len() - 1) % self.ui_tabs.len();
    }

    pub fn update(
        &mut self,
        world: &World,
        #[cfg(feature = "audio")] audio_player: Option<&MusicPlayer>,
    ) -> AppResult<()> {
        self.inner_registry.clear();
        match self.state {
            UiState::Splash => {
                #[cfg(feature = "audio")]
                {
                    // This is only to get a nice view in the splash screen
                    let audio_state = if let Some(player) = audio_player {
                        if player.is_playing() {
                            AudioPlayerState::Playing
                        } else {
                            AudioPlayerState::Paused
                        }
                    } else {
                        AudioPlayerState::Disabled
                    };
                    self.splash_screen.set_audio_player_state(audio_state);
                }
                self.splash_screen.update(world)?
            }
            UiState::NewTeam => self.new_team_screen.update(world)?,
            UiState::Main => {
                // Update panels. Can we get away updating only the active one?
                // Links between panels means they need to be updated.
                // Example: going to a game from the crews panel.
                // We call update explicitly whenever one of these links is clicked.
                // self.get_active_screen_mut().update(world)?;
                // FIXME: further check this.
                self.my_team_panel.update(world)?;
                self.team_panel.update(world)?;
                self.player_panel.update(world)?;
                self.game_panel.update(world)?;
                self.tournament_panel.update(world)?;
                self.galaxy_panel.update(world)?;
                self.space_cove_panel.update(world)?;
                if self.ui_tabs.contains(&UiTab::Swarm) {
                    self.swarm_panel.update(world)?;
                }
            }
            UiState::SpaceAdventure => self.space_screen.update(world)?,
        }

        Ok(())
    }

    /// Renders the user interface widgets.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        world: &World,
        #[cfg(feature = "audio")] audio_player: Option<&MusicPlayer>,
    ) {
        let mut ui_frame = UiFrame::new(frame);
        ui_frame.set_hovering(self.inner_registry.hovering());
        if !self.popup_messages.is_empty() || self.show_help {
            ui_frame.set_active_layer(1);
        } else {
            ui_frame.set_active_layer(0);
        }

        let screen_area = ui_frame.screen_area();

        let split = Layout::vertical([
            Constraint::Min(6),    // body
            Constraint::Length(1), // footer
            Constraint::Length(1), // hover text
        ])
        .split(screen_area);

        // render selected tab
        let render_result = match self.state {
            UiState::Splash => {
                self.splash_screen
                    .render(&mut ui_frame, world, split[0], self.debug_view)
            }
            UiState::NewTeam => {
                self.new_team_screen
                    .render(&mut ui_frame, world, split[0], self.debug_view)
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
                    &mut ui_frame,
                    world,
                    tab_main_split[1],
                    debug_view,
                );

                // render tab header
                let mut constraints = [Constraint::Length(16)].repeat(self.ui_tabs.len());
                constraints.push(Constraint::Min(0));

                ui_frame.render_widget(default_block(), tab_main_split[0]);
                let tab_split = Layout::horizontal(constraints).split(tab_main_split[0]);

                for (idx, &tab) in self.ui_tabs.iter().enumerate() {
                    let selected = idx == self.tab_index;
                    let mut button = if let Some(variants) = CONSTANT_TAB_BUTTONS.get(&tab) {
                        variants[selected as usize].clone()
                    } else {
                        let tab_name = if tab == UiTab::MyTeam {
                            world
                                .get_own_team()
                                .expect("Own team should be set if rendering main page")
                                .name
                                .clone()
                        } else {
                            let unread = self.swarm_panel.unread_chat_messages();
                            let suffix = if unread > 99 {
                                " (99+)".to_string()
                            } else if unread > 0 {
                                format!(" ({unread})")
                            } else {
                                String::new()
                            };
                            format!("{tab}{suffix}")
                        };
                        let callback = UiCallback::SetUiTab { ui_tab: tab };
                        if selected {
                            Button::new(tab_name, callback).selected()
                        } else {
                            Button::no_box(tab_name, callback)
                        }
                    };

                    if self.show_help && self.popup_messages.is_empty() {
                        button = button.set_layer(1);
                    }

                    ui_frame.render_interactive_widget(button, tab_split[idx]);
                }

                active_render
            }
            UiState::SpaceAdventure => {
                self.space_screen
                    .render(&mut ui_frame, world, split[0], self.debug_view)
            }
        };

        if let Err(err) = render_result {
            self.push_log_event(
                Tick::now(),
                None,
                format!("Render error\n{err}"),
                log::Level::Error,
            );
        }

        if self.show_help {
            let popup_rect = help_popup_rect(screen_area);
            ui_frame.render_widget(Clear, popup_rect);
            ui_frame.render_widget(thick_block(), popup_rect);

            let popup_split = Layout::vertical([
                Constraint::Length(3), // header
                Constraint::Min(3),    // body
                Constraint::Length(3), // close button
            ])
            .split(popup_rect.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }));

            let title = match &self.state {
                UiState::Main => self.ui_tabs[self.tab_index].to_string(),
                state => state.to_string(),
            };
            ui_frame.render_widget(
                Paragraph::new(format!("Help - {title}"))
                    .bold()
                    .block(default_block().border_style(UiStyle::HEADER))
                    .centered(),
                popup_split[0],
            );

            let debug_view = self.debug_view;
            if let Err(err) = self.get_active_screen().render_help_widget(
                &mut ui_frame,
                world,
                popup_split[1],
                debug_view,
            ) {
                self.push_log_event(
                    Tick::now(),
                    None,
                    format!("Help render error\n{err}"),
                    log::Level::Error,
                );
            }

            let button_split = Layout::horizontal([
                Constraint::Min(0),
                Constraint::Length(20),
                Constraint::Min(0),
            ])
            .split(popup_split[2]);
            let close_button = Button::new(super::constants::UiText::YES, UiCallback::CloseHelp)
                .set_hover_text("Close help")
                .set_hotkey(ui_key::YES_TO_DIALOG)
                .block(default_block().border_style(UiStyle::OK))
                .set_layer(1);
            ui_frame.render_interactive_widget(close_button, button_split[1]);
        }

        // Render footer
        self.render_footer(
            &mut ui_frame,
            world,
            #[cfg(feature = "audio")]
            audio_player,
            split[1],
        );

        if let Err(err) = self.render_popup_messages(&mut ui_frame, screen_area) {
            self.push_log_event(
                Tick::now(),
                None,
                format!("Popup render error\n{err}"),
                log::Level::Error,
            );
            log::error!("Popup render error\n{err}");
        }
        self.last_render = Instant::now();

        self.inner_registry = ui_frame.callback_registry().clone();
    }

    fn render_popup_messages(&mut self, frame: &mut UiFrame, area: Rect) -> AppResult<()> {
        // Render popup message
        if !self.popup_messages.is_empty() {
            self.popup_messages[0].render(frame, area, &mut self.popup_input)?;
        }
        Ok(())
    }

    fn render_footer(
        &self,
        frame: &mut UiFrame,
        world: &World,
        #[cfg(feature = "audio")] audio_player: Option<&MusicPlayer>,
        area: Rect,
    ) {
        frame.render_widget(Clear, area);
        let split = Layout::horizontal([
            Constraint::Min(50),
            Constraint::Length(20),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(26),
        ])
        .split(area);

        let mut spans = vec![
            " Esc ".to_string(),
            " Quit ".to_string(),
            " ? ".to_string(),
            if self.show_help {
                " Close help ".to_string()
            } else {
                " Help ".to_string()
            },
        ];

        if !self.debug_view && self.state == UiState::Main {
            spans.extend(vec![
                format!(" {} ", ui_key::PREVIOUS_TAB.to_string()),
                " Previous panel ".to_string(),
                format!(" {} ", ui_key::NEXT_TAB.to_string()),
                " Next panel ".to_string(),
            ]);
        }

        let extra_spans = if self.debug_view {
            let fps = (1.0 / self.last_render.elapsed().as_secs_f64()).round() as u32;
            let world_size = world.serialized_size / 1024;

            let mut spans = vec![
                format!(" FPS {:>4} ", fps),
                format!(" World size {:04} kb ", world_size),
                format!(" Seed {} ", world.seed),
                format!(
                    " Frame size {}x{} ",
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

        #[cfg(feature = "audio")]
        if let Some(audio_player) = &audio_player {
            let mut audio_button = Button::no_box(
                format!(
                    " {}: {}",
                    ui_key::radio::TOGGLE_AUDIO,
                    if audio_player.is_buffering() {
                        "Buffering...   "
                    } else if !audio_player.is_playing() {
                        "Turn radio on  "
                    } else {
                        "Turn radio off "
                    }
                ),
                UiCallback::ToggleAudio,
            )
            .set_hotkey(ui_key::radio::TOGGLE_AUDIO);

            if audio_player.is_buffering() {
                audio_button.disable(Some("Buffering..."));
            }

            frame.render_interactive_widget(audio_button, split[1]);

            frame.render_interactive_widget(
                Button::no_box(
                    format!(" {} ", ui_key::radio::PREVIOUS_RADIO),
                    UiCallback::PreviousRadio,
                )
                .set_hotkey(ui_key::radio::PREVIOUS_RADIO),
                split[2],
            );

            frame.render_interactive_widget(
                Button::no_box(
                    format!(" {} ", ui_key::radio::NEXT_RADIO),
                    UiCallback::NextRadio,
                )
                .set_hotkey(ui_key::radio::NEXT_RADIO),
                split[3],
            );
            if let Some(currently_playing) = audio_player.currently_playing() {
                frame.render_widget(Paragraph::new(currently_playing).centered(), split[4]);
            }
        }
    }
}
