use super::traits::SplitPanel;
use crate::network::trade::offered_phrase;
use crate::types::{HashMapWithResult, PlayerId, SystemTimeTick, TeamId, Tick};
use crate::ui::button::Button;
use crate::ui::clickable_list::ClickableListState;
use crate::ui::gif_map::GifMap;
use crate::ui::panels::traits::{normalize_index, HelpContent, HelpPanel, IndexBound, Screen};
use crate::ui::renders::{
    default_block, render_player_description, selectable_list, teleport_button, PlayerWidgetView,
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
use std::cmp::Reverse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DockRow {
    FreePirate(PlayerId),
    Listing(PlayerId),
}

impl DockRow {
    const fn player_id(&self) -> PlayerId {
        match self {
            Self::FreePirate(id) | Self::Listing(id) => *id,
        }
    }
}

#[derive(Debug, Default)]
pub struct DockPanel {
    tick: usize,
    rows: Vec<DockRow>,
    index: Option<usize>,
    list_state: ClickableListState,
    gif_map: GifMap,
}

impl DockPanel {
    pub fn new() -> Self {
        Self::default()
    }

    fn selected_row(&self) -> Option<DockRow> {
        self.index.and_then(|index| self.rows.get(index)).copied()
    }

    fn team_name(world: &World, team_id: Option<TeamId>) -> String {
        team_id
            .and_then(|id| world.teams.get(&id))
            .map_or_else(|| "a crew".to_string(), |team| team.name.clone())
    }

    fn header_line(label: &'static str, value: String) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{label:<10}"), UiStyle::HEADER),
            Span::raw(value),
        ])
    }

    fn render_header(frame: &mut UiFrame, world: &World, area: Rect) -> AppResult<()> {
        let dock_name = world
            .planets
            .get(&*GALAXY_ROOT_ID)
            .map_or("the black hole", |planet| planet.name.as_str());
        let split = Layout::horizontal([Constraint::Fill(1), Constraint::Length(32)]).split(area);
        frame.render_widget(
            Paragraph::new(format!("The Dock, at {dock_name}"))
                .style(UiStyle::HEADER)
                .block(default_block()),
            split[0],
        );

        let own_team = world.get_own_team()?;
        if own_team.is_at_dock() {
            frame.render_widget(
                Paragraph::new("You are at the dock")
                    .centered()
                    .block(default_block().border_style(UiStyle::OK)),
                split[1],
            );
        } else if own_team.is_on_planet().is_some() {
            frame.render_interactive_widget(teleport_button(world, *GALAXY_ROOT_ID)?, split[1]);
        } else {
            frame.render_widget(
                Paragraph::new("Not on a planet")
                    .centered()
                    .block(default_block()),
                split[1],
            );
        }
        Ok(())
    }

    fn offers_on(world: &World, player_id: PlayerId) -> usize {
        world.get_own_team().map_or(0, |team| {
            team.received_trades
                .values()
                .filter(|trade| trade.target_player.id == player_id)
                .count()
        })
    }

    fn render_list(&mut self, frame: &mut UiFrame, world: &World, area: Rect) {
        if self.rows.is_empty() {
            frame.render_widget(
                Paragraph::new("Nobody is looking for a crew.")
                    .centered()
                    .block(default_block().title("Looking for a crew")),
                area,
            );
            return;
        }

        let options = self
            .rows
            .iter()
            .filter_map(|row| {
                let player = world.players.get(&row.player_id())?;
                let (detail, style) = match row {
                    DockRow::FreePirate(_) => {
                        (format_satoshi(player.hire_cost()), UiStyle::DEFAULT)
                    }
                    DockRow::Listing(_) if player.team == Some(world.own_team_id) => (
                        format!("yours, {} offers", Self::offers_on(world, player.id)),
                        UiStyle::OWN_TEAM,
                    ),
                    DockRow::Listing(_) => (Self::team_name(world, player.team), UiStyle::DEFAULT),
                };
                Some((
                    format!(
                        "{:<width$} {} {}",
                        player.info.short_name(),
                        player.stars(),
                        detail,
                        width = MAX_NAME_LENGTH
                    ),
                    style,
                ))
            })
            .collect();

        self.list_state.select(self.index);
        frame.render_stateful_interactive_widget(
            selectable_list(options).block(default_block().title("Looking for a crew ↓/↑")),
            area,
            &mut self.list_state,
        );
    }

    fn render_detail_pane(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let split = Layout::horizontal([
            Constraint::Length(PLAYER_DESCRIPTION_WIDTH),
            Constraint::Fill(1),
        ])
        .split(area);

        let Some(row) = self.selected_row() else {
            frame.render_widget(default_block().title("Pirate"), split[0]);
            frame.render_widget(default_block(), split[1]);
            return Ok(());
        };
        let player = world.players.get_or_err(&row.player_id())?;
        let left = Layout::vertical([
            Constraint::Length(PLAYER_DESCRIPTION_HEIGHT),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(split[0]);

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

        match row {
            DockRow::FreePirate(_) => {
                Self::render_free_pirate(frame, world, player, left[1], split[1])
            }
            DockRow::Listing(_) if player.team == Some(world.own_team_id) => {
                Self::render_own_listing(frame, world, player, left[1], split[1])
            }
            DockRow::Listing(_) => Self::render_listing(frame, world, player, left[1], split[1]),
        }
    }

    fn render_free_pirate(
        frame: &mut UiFrame,
        world: &World,
        player: &Player,
        button_area: Rect,
        detail_area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let hire_cost = player.hire_cost();
        let mut button = Button::new(
            format!("Hire (-{})", format_satoshi(hire_cost)),
            UiCallback::HirePlayer {
                player_id: player.id,
            },
        )
        .hover_text(format!(
            "Hire {} for {}",
            player.info.short_name(),
            format_satoshi(hire_cost)
        ))
        .hotkey(ui_key::player::HIRE);
        if !own_team.is_at_dock() {
            button.disable(Some("Teleport to the dock to hire them"));
        } else if let Err(err) =
            own_team.can_hire_player(player, world.player_is_in_space_cove_on(player))
        {
            button.disable(Some(err.to_string()));
        }
        frame.render_interactive_widget(button, button_area);

        frame.render_widget(
            Paragraph::new(vec![
                Self::header_line("Free", "looking for a crew".to_string()),
                Self::header_line("Hire cost", format_satoshi(hire_cost)),
            ])
            .block(default_block().title("At the dock")),
            detail_area,
        );
        Ok(())
    }

    fn render_listing(
        frame: &mut UiFrame,
        world: &World,
        player: &Player,
        button_area: Rect,
        detail_area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let waiting = world
            .listing_for(&player.id)
            .map_or_else(String::new, |listing| {
                Tick::now().saturating_sub(listing.listed_on).formatted()
            });
        let mut lines = vec![
            Self::header_line("Left by", Self::team_name(world, player.team)),
            Self::header_line("Waiting", waiting),
        ];

        match own_team.offer_on(&player.id) {
            Some(offer) => {
                let pirate = offer.pirate.and_then(|id| world.players.get(&id));
                lines.push(Line::default());
                lines.push(Line::styled(
                    format!("Your offer: {}", offered_phrase(pirate, offer.satoshis)),
                    UiStyle::OK,
                ));
                let button = Button::new(
                    "Retire offer",
                    UiCallback::RetireOffer {
                        trade_id: offer.trade_id,
                    },
                )
                .hover_text("Retire your offer and get back what it holds")
                .block(default_block().border_style(UiStyle::ERROR));
                frame.render_interactive_widget(button, button_area);
            }
            None => {
                let mut button = Button::new(
                    "Make offer",
                    UiCallback::OpenOfferOverlay {
                        target_player_id: player.id,
                        own_offer: None,
                    },
                )
                .hover_text(format!("Make an offer for {}", player.info.short_name()))
                .hotkey(ui_key::CREATE_TRADE);
                if !own_team.is_at_dock() {
                    button.disable(Some("Teleport to the dock to make an offer"));
                }
                frame.render_interactive_widget(button, button_area);
            }
        }

        frame.render_widget(
            Paragraph::new(lines).block(default_block().title("At the dock")),
            detail_area,
        );
        Ok(())
    }

    fn render_own_listing(
        frame: &mut UiFrame,
        world: &World,
        player: &Player,
        button_area: Rect,
        detail_area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let now = Tick::now();

        let mut recall = Button::new(
            "Recall",
            UiCallback::RecallPlayerFromDock {
                player_id: player.id,
            },
        )
        .hover_text(format!("Take {} back aboard", player.info.short_name()))
        .hotkey(ui_key::player::MARKET_LISTING);
        if let Err(err) = own_team.can_recall_player_from_dock(&player.id) {
            recall.disable(Some(err.to_string()));
        }
        frame.render_interactive_widget(recall, button_area);

        frame.render_widget(default_block().title("Offers"), detail_area);
        let inner = detail_area.inner(Margin::new(1, 1));
        let offers = own_team
            .received_trades
            .values()
            .filter(|trade| trade.target_player.id == player.id)
            .sorted_by_key(|trade| (trade.created_at, trade.id))
            .collect_vec();
        if offers.is_empty() {
            frame.render_widget(Paragraph::new("No offers yet."), inner);
            return Ok(());
        }

        let mut constraints = [Constraint::Length(3)].repeat(offers.len());
        constraints.push(Constraint::Fill(1));
        let rows = Layout::vertical(constraints).split(inner);
        for (idx, trade) in offers.iter().enumerate() {
            let split = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(10),
                Constraint::Length(10),
            ])
            .split(rows[idx]);
            let crew = Self::team_name(world, Some(trade.proposer_team_id));
            let presence = if world.is_team_present(&trade.proposer_team_id, now) {
                Span::styled("● ", UiStyle::OK)
            } else {
                Span::styled("○ offline  ", UiStyle::UNSELECTABLE)
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    presence,
                    Span::raw(format!("{crew}: {}", trade.offered())),
                ]))
                .block(default_block()),
                split[0],
            );

            let mut accept =
                Button::new(UiText::YES, UiCallback::AcceptTrade { trade_id: trade.id })
                    .block(default_block().border_style(UiStyle::OK))
                    .hover_text(format!("Accept {crew}'s offer"));
            if let Err(err) = world.can_accept_offer(&trade.id, now) {
                accept.disable(Some(err.to_string()));
            }
            frame.render_interactive_widget(accept, split[1]);

            let decline = Button::new(UiText::NO, UiCallback::DeclineTrade { trade_id: trade.id })
                .block(default_block().border_style(UiStyle::ERROR))
                .hover_text(format!("Decline {crew}'s offer"));
            frame.render_interactive_widget(decline, split[2]);
        }
        Ok(())
    }
}

impl Screen for DockPanel {
    fn tick(&mut self) {
        self.tick += 1;
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        let free_pirates = world
            .players
            .values()
            .filter(|player| {
                player.team.is_none() && player.is_on_planet() == Some(*GALAXY_ROOT_ID)
            })
            .sorted_by_key(|player| (Reverse(player.hire_cost()), player.id))
            .map(|player| DockRow::FreePirate(player.id));
        let listings = world
            .teams
            .values()
            .flat_map(|team| team.dock_listings.iter())
            .filter(|listing| world.players.contains_key(&listing.player_id))
            .sorted_by_key(|listing| (listing.listed_on, listing.player_id))
            .map(|listing| DockRow::Listing(listing.player_id));
        self.rows = free_pirates.chain(listings).collect();
        self.index = normalize_index(self.index.unwrap_or(0), self.rows.len(), IndexBound::Clamp);
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

        let right = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).split(split[1]);
        Self::render_header(frame, world, right[0])?;
        frame.render_widget(default_block(), right[1]);
        self.render_detail_pane(frame, world, right[1].inner(Margin::new(1, 1)))?;
        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, _world: &World) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            KeyCode::Enter => {
                let row = self.selected_row()?;
                return Some(UiCallback::GoToPlayer {
                    player_id: row.player_id(),
                });
            }
            _ => {}
        }
        None
    }
}

impl HelpPanel for DockPanel {
    fn help_content(&self) -> HelpContent {
        HelpContent {
            description: [
                "Everyone looking for a crew: free pirates at the black hole, and pirates left at the dock by their crew.",
                "Teleport to the black hole to hire, to leave or recall a pirate, or to make an offer for one left here.",
                "The crew that left a pirate accepts whichever offer it likes, from anywhere.",
            ]
            .join("\n"),
            links: vec![
                tab_link("My Team", UiTab::MyTeam),
                tab_link("Pirates", UiTab::Pirates),
            ],
            controls: vec![
                Line::from("  Make offer  A pirate, satoshi, or both. Cash you pay is held until it ends."),
                Line::from("  Retire      Take your offer back, with everything it holds."),
                Line::from("  Accept      Both crews must be online. The pirates wait at the dock to be collected."),
                Line::from("  Offline     Offers from crews you have not heard from lately cannot be accepted."),
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
        self.rows.len()
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

#[cfg(test)]
mod tests {
    use super::{DockPanel, DockRow};
    use crate::app::App;
    use crate::core::DockListing;
    use crate::types::{AppResult, SystemTimeTick, Tick};
    use crate::ui::panels::traits::Screen;

    #[test]
    fn test_free_pirates_come_before_listings() -> AppResult<()> {
        let mut app = App::test_default()?;
        let crew_id = app
            .world
            .teams
            .values()
            .find(|team| team.id != app.world.own_team_id)
            .expect("another crew")
            .id;
        let team = app.world.teams.get_mut(&crew_id).expect("crew");
        let listed = team.player_ids[0];
        team.dock_listings
            .push(DockListing::new(listed, Tick::now()));

        let mut panel = DockPanel::new();
        panel.update(&app.world)?;

        assert!(matches!(panel.rows.first(), Some(DockRow::FreePirate(_))));
        assert_eq!(panel.rows.last(), Some(&DockRow::Listing(listed)));
        Ok(())
    }
}
