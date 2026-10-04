use super::{centered_rect, Overlay};
use crate::core::world::World;
use crate::core::{skill::Rated, OfferKind, MIN_PLAYERS_PER_GAME};
use crate::types::{AppResult, HashMapWithResult, PlayerId, TeamId};
use crate::ui::button::Button;
use crate::ui::constants::{UiStyle, UiText, MAX_NAME_LENGTH};
use crate::ui::panels::{HelpContent, IndexBound, SplitPanel};
use crate::ui::renders::default_block;
use crate::ui::ui_callback::UiCallback;
use crate::ui::ui_frame::UiFrame;
use crate::ui::ui_key;
use crate::ui::utils::format_satoshi;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::Styled;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

/// Quick-adjust steps for the satoshi fields, mirroring the resource market.
const SATOSHI_STEPS: [(i64, &str); 4] = [
    (-10_000, "-10k"),
    (-1_000, "-1k"),
    (1_000, "+1k"),
    (10_000, "+10k"),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    #[default]
    Own,
    Other,
}

#[derive(Debug)]
pub struct TradeOverlay {
    other_team_id: TeamId,
    own_roster: Vec<PlayerId>,
    other_roster: Vec<PlayerId>,
    own_offer: Option<PlayerId>,
    other_offer: Option<PlayerId>,
    own_satoshis: u32,
    other_satoshis: u32,
    focus: TradeSide,
    own_index: usize,
    other_index: usize,
    /// Filled in by `update`, so `render` never has to compute it.
    blocker: Option<String>,
}

impl TradeOverlay {
    pub fn crew_swap(other_team_id: TeamId, seed_other: Option<PlayerId>) -> Self {
        Self {
            other_team_id,
            own_roster: vec![],
            other_roster: vec![],
            own_offer: None,
            other_offer: seed_other,
            own_satoshis: 0,
            other_satoshis: 0,
            focus: TradeSide::Own,
            own_index: 0,
            other_index: 0,
            blocker: None,
        }
    }

    pub fn set_offer_player(&mut self, side: TradeSide, player_id: PlayerId) {
        let slot = match side {
            TradeSide::Own => &mut self.own_offer,
            TradeSide::Other => &mut self.other_offer,
        };
        // Clicking the pirate already on the table takes them back off it.
        *slot = if *slot == Some(player_id) {
            None
        } else {
            Some(player_id)
        };
    }

    pub fn adjust_satoshis(&mut self, side: TradeSide, delta: i64, balance: u32) {
        let current = match side {
            TradeSide::Own => self.own_satoshis,
            TradeSide::Other => self.other_satoshis,
        };
        let next = (current as i64 + delta).clamp(0, balance as i64) as u32;
        match side {
            TradeSide::Own => self.own_satoshis = next,
            TradeSide::Other => self.other_satoshis = next,
        }
    }

    /// The offer as the network layer wants it.
    pub fn offer(&self) -> (OfferKind, Option<PlayerId>, Option<PlayerId>, i64) {
        (
            OfferKind::Direct,
            self.own_offer,
            self.other_offer,
            self.own_satoshis as i64 - self.other_satoshis as i64,
        )
    }

    pub const fn other_team_id(&self) -> TeamId {
        self.other_team_id
    }

    pub fn blocker(&self) -> Option<&String> {
        self.blocker.as_ref()
    }

    fn roster(&self, side: TradeSide) -> &[PlayerId] {
        match side {
            TradeSide::Own => &self.own_roster,
            TradeSide::Other => &self.other_roster,
        }
    }

    fn index(&self, side: TradeSide) -> usize {
        match side {
            TradeSide::Own => self.own_index,
            TradeSide::Other => self.other_index,
        }
    }

    fn recompute_blockers(&mut self, world: &World) {
        self.blocker = None;
        let Ok(own_team) = world.get_own_team() else {
            return;
        };
        let Ok(other_team) = world.teams.get_or_err(&self.other_team_id()) else {
            self.blocker = Some("That crew is gone".to_string());
            return;
        };

        let (route, own_player_id, other_player_id, satoshis) = self.offer();

        let Some(target_player_id) = other_player_id else {
            self.blocker = Some("Pick a pirate to trade for".to_string());
            return;
        };
        let Ok(target_player) = world.players.get_or_err(&target_player_id) else {
            self.blocker = Some("That pirate is gone".to_string());
            return;
        };

        if route == OfferKind::Direct && own_player_id.is_none() {
            self.blocker = Some("Pick one of your pirates".to_string());
            return;
        }

        let own_player = own_player_id.and_then(|id| world.players.get(&id));
        if let Err(err) =
            own_team.can_make_offer(other_team, route, own_player, target_player, satoshis)
        {
            self.blocker = Some(err.to_string());
            return;
        }

        // Advisory only: a legal trade that leaves you unable to field a game.
        if route == OfferKind::Direct
            && own_team.active_players_count() <= MIN_PLAYERS_PER_GAME
            && own_player_id.is_some()
        {
            self.blocker = Some("Your crew would be too small for a game".to_string());
        }
    }

    fn render_roster(
        &self,
        frame: &mut UiFrame,
        world: &World,
        side: TradeSide,
        title: &str,
        area: Rect,
        layer: usize,
    ) {
        frame.render_widget(default_block().title(title.to_string()), area);
        let inner = area.inner(Margin::new(1, 1));
        let roster = self.roster(side);

        if roster.is_empty() {
            frame.render_widget(Paragraph::new("No pirate available.").centered(), inner);
            return;
        }

        let visible = inner.height as usize;
        let cursor = self.index(side);
        // Keep the cursor inside the window without storing a scroll offset.
        let offset = cursor.saturating_sub(visible.saturating_sub(1));
        let chosen = match side {
            TradeSide::Own => self.own_offer,
            TradeSide::Other => self.other_offer,
        };

        let rows = Layout::vertical([Constraint::Length(1)].repeat(visible)).split(inner);
        for (row, player_id) in roster.iter().skip(offset).take(visible).enumerate() {
            let Some(player) = world.players.get(player_id) else {
                continue;
            };
            let mark = if chosen == Some(*player_id) {
                "✔"
            } else {
                " "
            };
            let line = Line::from(format!(
                "{mark} {:<width$} {}",
                player.info.short_name(),
                player.stars(),
                width = MAX_NAME_LENGTH,
            ));

            let mut button = Button::no_box(
                line,
                UiCallback::SetTradeOfferPlayer {
                    side,
                    player_id: *player_id,
                },
            )
            .set_text_alignment(Alignment::Left)
            .hover_text(format!("Put {} on the table", player.info.full_name()));
            if offset + row == cursor && self.focus == side {
                button = button.set_style(UiStyle::SELECTED);
            }
            frame.render_interactive_widget_on_layer(button, rows[row], layer);
        }
    }

    fn render_satoshi_row(
        &self,
        frame: &mut UiFrame,
        side: TradeSide,
        amount: u32,
        title: &str,
        area: Rect,
        layer: usize,
    ) {
        frame.render_widget(default_block().title(title.to_string()), area);
        let inner = area.inner(Margin::new(1, 1));

        let split = Layout::horizontal([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Length(6),
        ])
        .split(inner);

        for (slot, (delta, label)) in SATOSHI_STEPS.iter().enumerate() {
            let target = if slot < 2 { slot } else { slot + 1 };
            let button = Button::new(
                *label,
                UiCallback::AdjustTradeOfferSatoshis {
                    side,
                    delta: *delta,
                },
            )
            .hover_text(format!("Adjust by {delta} satoshi"));
            frame.render_interactive_widget_on_layer(button, split[target], layer);
        }

        frame.render_widget(Paragraph::new(format_satoshi(amount)).centered(), split[2]);
    }
}

impl SplitPanel for TradeOverlay {
    fn index(&self) -> Option<usize> {
        Some(self.index(self.focus))
    }

    fn max_index(&self) -> usize {
        self.roster(self.focus).len()
    }

    fn index_bound(&self) -> IndexBound {
        IndexBound::Clamp
    }

    fn set_index(&mut self, index: usize) {
        match self.focus {
            TradeSide::Own => self.own_index = index,
            TradeSide::Other => self.other_index = index,
        }
    }
}

impl Overlay for TradeOverlay {
    fn title(&self, world: &World) -> String {
        let other = world
            .teams
            .get(&self.other_team_id())
            .map(|team| team.name.clone())
            .unwrap_or_else(|| "unknown crew".to_string());
        format!("Trade with {other}")
    }

    fn rect(&self, screen_area: Rect) -> Rect {
        centered_rect(screen_area, 80, 70, (76, 20))
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        self.own_roster = own_team.active_player_ids();

        self.other_roster = world
            .teams
            .get(&self.other_team_id)
            .map(|team| team.active_player_ids())
            .unwrap_or_default();

        self.own_index = self.own_index.min(self.own_roster.len().saturating_sub(1));
        self.other_index = self
            .other_index
            .min(self.other_roster.len().saturating_sub(1));

        self.recompute_blockers(world);
        Ok(())
    }

    fn render(
        &mut self,
        frame: &mut UiFrame,
        world: &World,
        area: Rect,
        layer: usize,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        let body = Layout::vertical([
            Constraint::Min(6),    // rosters
            Constraint::Length(3), // satoshis
            Constraint::Length(3), // deal summary
            Constraint::Length(3), // actions
        ])
        .split(area);

        let columns =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(body[0]);

        self.render_roster(frame, world, TradeSide::Own, "You give", columns[0], layer);
        self.render_roster(frame, world, TradeSide::Other, "You get", columns[1], layer);

        let money =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(body[1]);
        self.render_satoshi_row(
            frame,
            TradeSide::Own,
            self.own_satoshis,
            "Satoshi you add",
            money[0],
            layer,
        );
        self.render_satoshi_row(
            frame,
            TradeSide::Other,
            self.other_satoshis,
            "Satoshi they add",
            money[1],
            layer,
        );

        let summary = match self.blocker() {
            Some(blocker) => Paragraph::new(blocker.clone())
                .centered()
                .block(default_block().border_style(UiStyle::WARNING)),
            None => {
                let crew_now = own_team.active_players_count();
                let crew_after = crew_now - self.own_offer.is_some() as usize
                    + self.other_offer.is_some() as usize;
                Paragraph::new(format!(
                    "Crew {crew_now} → {crew_after}    Balance {} → {}",
                    format_satoshi(own_team.balance()),
                    format_satoshi(own_team.balance().saturating_sub(self.own_satoshis)),
                ))
                .centered()
                .block(default_block().border_style(UiStyle::OK))
            }
        };
        frame.render_widget(summary, body[2]);

        let actions = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(24),
            Constraint::Length(12),
            Constraint::Fill(1),
        ])
        .split(body[3]);

        let mut send = Button::new("Send offer", UiCallback::SendTradeOffer)
            .hover_text("Send this offer. It cannot be taken back once accepted.")
            .hotkey(ui_key::CREATE_TRADE)
            .block(default_block().border_style(UiStyle::OK));
        if let Some(blocker) = self.blocker() {
            send.disable(Some(blocker.clone()));
        }
        frame.render_interactive_widget_on_layer(send, actions[1], layer);

        let cancel = Button::new(UiText::NO, UiCallback::CloseOverlay)
            .hover_text("Close without sending")
            .block(default_block().border_style(UiStyle::ERROR));
        frame.render_interactive_widget_on_layer(cancel, actions[2], layer);

        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, _world: &World) -> Option<UiCallback> {
        match key_event.code {
            // Left/Right hop columns rather than switching tab.
            KeyCode::Left | KeyCode::Right | ui_key::CYCLE_VIEW => {
                self.focus = match self.focus {
                    TradeSide::Own => TradeSide::Other,
                    TradeSide::Other => TradeSide::Own,
                };
                Some(UiCallback::None)
            }
            KeyCode::Up => {
                self.next_index();
                Some(UiCallback::None)
            }
            KeyCode::Down => {
                self.previous_index();
                Some(UiCallback::None)
            }
            // Enter puts a pirate on the table. Sending is 'P': in a builder Enter
            // is pressed constantly, and an offer cannot be unsent.
            ui_key::YES_TO_DIALOG => {
                let side = self.focus;
                let roster = self.roster(side);
                let player_id = *roster.get(self.index(side))?;
                self.set_offer_player(side, player_id);
                Some(UiCallback::None)
            }
            _ => None,
        }
    }

    fn as_split_panel(&mut self) -> Option<&mut dyn SplitPanel> {
        Some(self)
    }

    fn help_content(&self) -> Option<(String, HelpContent)> {
        Some((
            "Trade".to_string(),
            HelpContent {
                description: "Pick a pirate from each crew and add satoshi to either side."
                    .to_string(),
                // A link would switch tab and lose the offer being composed.
                links: vec![],
                controls: vec![
                    Line::from("  ←/→ or Tab  Switch between the two crews"),
                    Line::from("  ↑/↓         Move the highlight"),
                    Line::from("  Enter       Put the highlighted pirate on the table"),
                    Line::from("  P           Send the offer"),
                    Line::from("  Esc         Close without sending"),
                ],
            },
        ))
    }
}
