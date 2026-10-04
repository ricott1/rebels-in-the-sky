use crate::types::{PlayerId, SystemTimeTick, TeamId, Tick};
use crate::ui::clickable_list::ClickableListState;
use crate::ui::constants::*;
use crate::ui::gif_map::GifMap;
use crate::ui::panels::traits::{normalize_index, HelpContent, HelpPanel, IndexBound, Screen};
use crate::ui::renders::{
    default_block, render_player_description, selectable_list, PlayerWidgetView,
};
use crate::ui::ui_callback::UiCallback;
use crate::ui::ui_frame::UiFrame;
use crate::ui::ui_screen::{tab_link, UiTab};
use crate::{core::*, types::AppResult};
use itertools::Itertools;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::prelude::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::traits::SplitPanel;

#[derive(Debug, Default)]
pub struct DockPanel {
    tick: usize,
    /// Oldest listing first.
    listing_ids: Vec<PlayerId>,
    index: Option<usize>,
    list_state: ClickableListState,
    gif_map: GifMap,
}

impl DockPanel {
    pub fn new() -> Self {
        Self::default()
    }

    fn selected_player_id(&self) -> Option<PlayerId> {
        self.index
            .and_then(|index| self.listing_ids.get(index))
            .copied()
    }

    fn team_name(world: &World, team_id: Option<TeamId>) -> &str {
        team_id
            .and_then(|id| world.teams.get(&id))
            .map_or("someone", |team| team.name.as_str())
    }

    fn render_list(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        if self.listing_ids.is_empty() {
            frame.render_widget(
                Paragraph::new("Nobody is looking for a crew.")
                    .centered()
                    .block(default_block().title("The dock")),
                area,
            );
            return;
        }

        let options = self
            .listing_ids
            .iter()
            .filter_map(|id| world.players.get(id))
            .map(|player| {
                (
                    format!(
                        "{:<width$} {} {}",
                        player.info.short_name(),
                        player.stars(),
                        Self::team_name(world, player.team),
                        width = MAX_NAME_LENGTH
                    ),
                    UiStyle::DEFAULT,
                )
            })
            .collect();

        let list = selectable_list(options);
        self.list_state.select(self.index);
        frame.render_stateful_interactive_widget(
            list.block(default_block().title("Looking for a crew ↓/↑")),
            area,
            &mut self.list_state,
        );
    }

    fn render_detail_pane(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        let split = Layout::horizontal([
            Constraint::Length(PLAYER_DESCRIPTION_WIDTH),
            Constraint::Fill(1),
        ])
        .split(area);

        let left = Layout::vertical([
            Constraint::Length(PLAYER_DESCRIPTION_HEIGHT),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(split[0]);

        let Some(player) = self
            .selected_player_id()
            .and_then(|id| world.players.get(&id))
        else {
            frame.render_widget(default_block().title("Pirate"), left[0]);
            frame.render_widget(default_block().title("At the dock"), split[1]);
            return;
        };

        render_player_description(
            player,
            &world.players_scouting,
            PlayerWidgetView::Skills,
            &mut self.gif_map,
            self.tick,
            world,
            frame,
            left[0],
        );

        let Some(listing) = world.listing_for(&player.id) else {
            frame.render_widget(default_block().title("At the dock"), split[1]);
            return;
        };

        let now = Tick::now();
        Self::render_listing_detail(frame, world, player, listing, now, split[1]);
    }

    fn render_listing_detail(
        frame: &mut UiFrame,
        world: &World,
        player: &Player,
        listing: &DockListing,
        now: Tick,
        area: Rect,
    ) {
        frame.render_widget(default_block().title("At the dock"), area);
        let lines = vec![
            Line::from(vec![
                Span::styled("Left by  ", UiStyle::HEADER),
                Span::raw(Self::team_name(world, player.team)),
            ]),
            Line::from(vec![
                Span::styled("Waiting  ", UiStyle::HEADER),
                Span::raw(now.saturating_sub(listing.listed_on).formatted()),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines), area.inner(Margin::new(2, 1)));
    }
}

impl Screen for DockPanel {
    fn tick(&mut self) {
        self.tick += 1;
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        self.listing_ids = world
            .teams
            .values()
            .flat_map(|team| team.dock_listings.iter())
            .filter(|listing| world.players.contains_key(&listing.player_id))
            .sorted_by_key(|listing| (listing.listed_on, listing.player_id))
            .map(|listing| listing.player_id)
            .collect();
        self.index = normalize_index(
            self.index.unwrap_or(0),
            self.listing_ids.len(),
            IndexBound::Clamp,
        );
        Ok(())
    }

    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        let split = Layout::horizontal([Constraint::Length(LEFT_PANEL_WIDTH), Constraint::Fill(1)])
            .split(area);

        self.render_list(frame, world, split[0]);

        frame.render_widget(default_block(), split[1]);
        self.render_detail_pane(frame, world, split[1].inner(Margin::new(1, 1)));

        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, _world: &World) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            KeyCode::Enter => {
                let player_id = self.selected_player_id()?;
                return Some(UiCallback::GoToPlayer { player_id });
            }
            _ => {}
        }
        None
    }
}

impl HelpPanel for DockPanel {
    fn help_content(&self) -> HelpContent {
        HelpContent {
            description: "Every pirate in the galaxy left at the dock by their crew.\nCrews leave a pirate at the dock from My Team."
                .to_string(),
            links: vec![
                tab_link("My Team", UiTab::MyTeam),
                tab_link("Pirates", UiTab::Pirates),
            ],
            controls: vec![
                Line::default(),
                Line::from("Controls:"),
                Line::from("   ↑/↓        Move highlight in the list"),
                Line::from("   Enter      Open the highlighted pirate"),
            ],
        }
    }
}

impl SplitPanel for DockPanel {
    fn index(&self) -> Option<usize> {
        self.index
    }

    fn max_index(&self) -> usize {
        self.listing_ids.len()
    }

    fn set_index(&mut self, index: usize) {
        let len = self.max_index();
        if len == 0 {
            return;
        }
        self.index = Some(index % len);
    }

    fn index_bound(&self) -> IndexBound {
        IndexBound::Clamp
    }
}
