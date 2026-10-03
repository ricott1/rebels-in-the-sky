use crate::types::{PlayerId, SystemTimeTick, TeamId, Tick};
use crate::ui::button::Button;
use crate::ui::clickable_list::ClickableListState;
use crate::ui::gif_map::GifMap;
use crate::ui::panels::traits::{normalize_index, HelpContent, HelpPanel, IndexBound, Screen};
use crate::ui::renders::{
    default_block, render_player_description, selectable_list, sign_now_button, PlayerWidgetView,
};
use crate::ui::ui_callback::UiCallback;
use crate::ui::ui_frame::UiFrame;
use crate::ui::ui_screen::{tab_link, UiTab};
use crate::ui::utils::format_satoshi;
use crate::ui::{constants::*, ui_key};
use crate::{core::*, types::AppResult};
use itertools::Itertools;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::prelude::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::traits::SplitPanel;

/// Every pirate in the galaxy waiting for a new crew, on one board. The dock is
/// not a place, so there is nothing to draw: just the list, the pirate, the auction.
#[derive(Debug, Default)]
pub struct DockPanel {
    tick: usize,
    /// Closing soonest first.
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

    fn time_left(listing: &DockListing, now: Tick) -> String {
        if listing.has_expired(now) {
            "ended".to_string()
        } else {
            format!(
                "{} left",
                listing.expires_at.saturating_sub(now).formatted()
            )
        }
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
                let asking = world
                    .listing_for(&player.id)
                    .map(|listing| {
                        listing
                            .highest_bid
                            .as_ref()
                            .map_or(listing.min_bid, |bid| bid.amount)
                    })
                    .unwrap_or_default();
                (
                    format!(
                        "{:<width$} {} {:>9}",
                        player.info.short_name(),
                        player.stars(),
                        format_satoshi(asking),
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
            frame.render_widget(default_block().title("Auction"), split[1]);
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
            frame.render_widget(default_block().title("Auction"), split[1]);
            return;
        };

        let now = Tick::now();
        Self::render_auction_buttons(frame, world, player, listing, now, left[1], left[2]);
        Self::render_auction_detail(frame, world, player, listing, now, split[1]);
    }

    fn render_auction_buttons(
        frame: &mut UiFrame,
        world: &World,
        player: &Player,
        listing: &DockListing,
        now: Tick,
        bid_area: Rect,
        sign_area: Rect,
    ) {
        // Our own listing: we watch it run, we do not bid on it.
        if player.team == Some(world.own_team_id) {
            return;
        }

        let player_id = player.id;
        let mut bid_button = Button::new("Bid", UiCallback::OpenDockBidOverlay { player_id })
            .hover_text(format!(
                "Bid for {} - {}",
                player.info.short_name(),
                Self::time_left(listing, now)
            ));
        if let Err(err) = world.can_bid_on(&player_id, listing.next_valid_bid()) {
            bid_button.disable(Some(err.to_string()));
        }
        frame.render_interactive_widget(bid_button, bid_area);

        frame.render_interactive_widget(
            sign_now_button(world, player_id, listing.release_fee),
            sign_area,
        );
    }

    fn render_auction_detail(
        frame: &mut UiFrame,
        world: &World,
        player: &Player,
        listing: &DockListing,
        now: Tick,
        area: Rect,
    ) {
        frame.render_widget(default_block().title("Auction"), area);

        let standing = match listing.highest_bid.as_ref() {
            Some(bid) => format!(
                "{} by {}",
                format_satoshi(bid.amount),
                Self::team_name(world, Some(bid.team_id))
            ),
            None => "no bids yet".to_string(),
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Released by ", UiStyle::HEADER),
                Span::raw(Self::team_name(world, player.team)),
            ]),
            Line::default(),
            Line::from(vec![
                Span::styled("Sign now    ", UiStyle::HEADER),
                Span::raw(format_satoshi(listing.release_fee)),
            ]),
            Line::from(vec![
                Span::styled("Highest bid ", UiStyle::HEADER),
                Span::raw(standing),
            ]),
            Line::from(vec![
                Span::styled("Next bid    ", UiStyle::HEADER),
                Span::raw(format_satoshi(listing.next_valid_bid())),
            ]),
            Line::default(),
            Line::from(vec![
                Span::styled("Closes in   ", UiStyle::HEADER),
                Span::raw(Self::time_left(listing, now)),
            ]),
        ];
        if world.has_outstanding_bid_on(&player.id) {
            lines.push(Line::default());
            lines.push(Line::styled("Your bid stands.", UiStyle::OK));
        }

        let inner = area.inner(Margin::new(2, 1));
        let split = Layout::vertical([Constraint::Length(lines.len() as u16), Constraint::Fill(1)])
            .split(inner);

        frame.render_widget(Paragraph::new(lines), split[0]);
        Self::render_bid_log(frame, world, listing, now, split[1]);
    }

    fn render_bid_log(
        frame: &mut UiFrame,
        world: &World,
        listing: &DockListing,
        now: Tick,
        area: Rect,
    ) {
        if listing.bid_log.is_empty() {
            return;
        }

        let mut lines = vec![Line::default(), Line::styled("Outbid", UiStyle::HEADER)];
        let rows = (area.height as usize).saturating_sub(lines.len());
        for record in listing.bid_log.iter().take(rows) {
            let age = format!(
                "{} ago",
                now.saturating_sub(record.placed_on).formatted_up_to_hours()
            );
            lines.push(Line::from(vec![
                Span::raw(format!("{:>12}  ", format_satoshi(record.amount))),
                Span::raw(format!(
                    "{:<width$}",
                    Self::team_name(world, Some(record.team_id)),
                    width = MAX_NAME_LENGTH
                )),
                Span::styled(format!("{age:>12}"), UiStyle::UNSELECTABLE),
            ]));
        }

        frame.render_widget(Paragraph::new(lines), area);
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
            .sorted_by_key(|listing| (listing.expires_at, listing.player_id))
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

    fn footer_spans(&self) -> Vec<String> {
        vec![
            format!(" {} ", ui_key::dock::SIGN_NOW),
            " Sign now ".to_string(),
        ]
    }
}

impl HelpPanel for DockPanel {
    fn help_content(&self) -> HelpContent {
        HelpContent {
            description: [
                "Every pirate in the galaxy looking for a new crew, in one place.",
                "Crews leave a pirate at the dock from My Team; other crews bid, and the pirate signs with the winner.",
                "The releasing crew gets the fee.",
            ]
            .join("\n"),
            links: vec![
                tab_link("My Team", UiTab::MyTeam),
                tab_link("Pirates", UiTab::Pirates),
            ],
            controls: vec![
                Line::from("  Bid...      Offer for a pirate another crew left at the dock"),
                Line::from("  Sign now    Pay the release fee and they sign on the spot. Binding."),
                Line::from("  Bids hold your satoshi until you win, are outbid, or the auction lapses."),
                Line::from("  A bid in the last minute pushes the deadline out, so nothing is sniped."),
                Line::from(format!(
                    "  A raise must beat the standing bid by {DOCK_MIN_BID_RAISE_PERCENT}%."
                )),
                Line::from("  A pirate left at the dock is shown off: every crew sees more of their skills."),
                Line::from("  A won pirate comes aboard the moment the auction closes, wherever you are."),
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
