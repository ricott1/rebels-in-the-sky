use super::{centered_rect, Overlay};
use crate::core::world::World;
use crate::core::{skill::Rated, OfferKind, MIN_PLAYERS_PER_GAME};
use crate::network::trade::satoshi_amount;
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
use std::cmp::Ordering;

const SATOSHI_STEPS: [(i64, &str); 4] = [
    (-10_000, "-10k"),
    (-1_000, "-1k"),
    (1_000, "+1k"),
    (10_000, "+10k"),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OfferSide {
    #[default]
    Own,
    Other,
}

#[derive(Debug)]
pub struct OfferOverlay {
    other_team_id: TeamId,
    dock_target: Option<PlayerId>,
    own_roster: Vec<PlayerId>,
    other_roster: Vec<PlayerId>,
    own_offer: Option<PlayerId>,
    other_offer: Option<PlayerId>,
    // Positive means we pay them, negative means we ask them to pay.
    satoshis: i64,
    focus: OfferSide,
    own_index: usize,
    other_index: usize,
    blocker: Option<String>,
    warning: Option<String>,
}

impl OfferOverlay {
    pub fn new(
        other_team_id: TeamId,
        target_player_id: PlayerId,
        is_dock_offer: bool,
        own_offer: Option<PlayerId>,
    ) -> Self {
        Self {
            other_team_id,
            dock_target: is_dock_offer.then_some(target_player_id),
            own_roster: vec![],
            other_roster: vec![],
            own_offer,
            other_offer: Some(target_player_id),
            satoshis: 0,
            focus: OfferSide::Own,
            own_index: 0,
            other_index: 0,
            blocker: None,
            warning: None,
        }
    }

    pub fn set_offer_player(&mut self, side: OfferSide, player_id: PlayerId) {
        if side == OfferSide::Other && self.dock_target.is_some() {
            return;
        }
        let slot = match side {
            OfferSide::Own => &mut self.own_offer,
            OfferSide::Other => &mut self.other_offer,
        };
        *slot = if *slot == Some(player_id) {
            None
        } else {
            Some(player_id)
        };
    }

    pub fn adjust_satoshis(&mut self, delta: i64, world: &World) {
        let (own_balance, their_balance) = self.balances(world);
        self.shift_satoshis(delta, own_balance, their_balance);
    }

    fn balances(&self, world: &World) -> (u32, u32) {
        let own = world
            .get_own_team()
            .map(|team| team.balance())
            .unwrap_or_default();
        let their = world
            .teams
            .get(&self.other_team_id)
            .map(|team| team.balance())
            .unwrap_or_default();
        (own, their)
    }

    fn clamp(satoshis: i64, own_balance: u32, their_balance: u32) -> i64 {
        satoshis.clamp(-i64::from(their_balance), i64::from(own_balance))
    }

    fn shift_satoshis(&mut self, delta: i64, own_balance: u32, their_balance: u32) {
        self.satoshis = Self::clamp(
            self.satoshis.saturating_add(delta),
            own_balance,
            their_balance,
        );
    }

    fn type_digit(&mut self, digit: i64, own_balance: u32, their_balance: u32) {
        let sign = if self.satoshis < 0 { -1 } else { 1 };
        let magnitude = self
            .satoshis
            .saturating_abs()
            .saturating_mul(10)
            .saturating_add(digit);
        self.satoshis = Self::clamp(sign * magnitude, own_balance, their_balance);
    }

    fn delete_digit(&mut self) {
        self.satoshis /= 10;
    }

    fn flip_direction(&mut self, own_balance: u32, their_balance: u32) {
        self.satoshis = Self::clamp(self.satoshis.saturating_neg(), own_balance, their_balance);
    }

    pub fn balance_label(&self) -> String {
        match self.satoshis.cmp(&0) {
            Ordering::Greater => {
                format!("You pay {}", format_satoshi(satoshi_amount(self.satoshis)))
            }
            Ordering::Less => format!(
                "You ask {}",
                format_satoshi(satoshi_amount(self.satoshis.saturating_neg()))
            ),
            Ordering::Equal => "Even".to_string(),
        }
    }

    pub fn offer(&self) -> (Option<PlayerId>, Option<PlayerId>, i64) {
        (self.own_offer, self.other_offer, self.satoshis)
    }

    pub const fn other_team_id(&self) -> TeamId {
        self.other_team_id
    }

    fn roster(&self, side: OfferSide) -> &[PlayerId] {
        match side {
            OfferSide::Own => &self.own_roster,
            OfferSide::Other => &self.other_roster,
        }
    }

    fn side_index(&self, side: OfferSide) -> usize {
        match side {
            OfferSide::Own => self.own_index,
            OfferSide::Other => self.other_index,
        }
    }

    fn recompute_blockers(&mut self, world: &World) {
        self.blocker = None;
        self.warning = None;
        let Ok(own_team) = world.get_own_team() else {
            return;
        };
        let Ok(other_team) = world.teams.get_or_err(&self.other_team_id) else {
            self.blocker = Some("That crew is gone".to_string());
            return;
        };
        let Some(target_player_id) = self.other_offer else {
            self.blocker = Some("Pick a pirate to make an offer for".to_string());
            return;
        };
        let Ok(target_player) = world.players.get_or_err(&target_player_id) else {
            self.blocker = Some("That pirate is gone".to_string());
            return;
        };
        if own_team.offer_on(&target_player_id).is_some() {
            self.blocker = Some(format!(
                "You already made an offer for {}",
                target_player.info.short_name()
            ));
            return;
        }

        let route = if other_team.is_listed(&target_player_id) {
            OfferKind::Dock
        } else {
            OfferKind::Direct
        };
        let own_player = self.own_offer.and_then(|id| world.players.get(&id));
        if let Err(err) =
            own_team.can_make_offer(other_team, route, own_player, target_player, self.satoshis)
        {
            self.blocker = Some(err.to_string());
            return;
        }

        if self.own_offer.is_some() && own_team.active_players_count() <= MIN_PLAYERS_PER_GAME {
            self.warning = Some("Your crew would be too small for a game".to_string());
        }
    }

    fn render_roster(
        &self,
        frame: &mut UiFrame,
        world: &World,
        side: OfferSide,
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
        let cursor = self.side_index(side);
        let offset = cursor.saturating_sub(visible.saturating_sub(1));
        let chosen = match side {
            OfferSide::Own => self.own_offer,
            OfferSide::Other => self.other_offer,
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
                UiCallback::SetOfferPlayer {
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

    fn render_satoshi_row(&self, frame: &mut UiFrame, area: Rect, layer: usize) {
        frame.render_widget(default_block().title("Satoshi"), area);
        let split = Layout::horizontal([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Length(6),
        ])
        .split(area.inner(Margin::new(1, 1)));

        for (slot, (delta, label)) in SATOSHI_STEPS.iter().enumerate() {
            let target = if slot < 2 { slot } else { slot + 1 };
            let button = Button::new(*label, UiCallback::AdjustOfferSatoshis { delta: *delta })
                .hover_text("Positive pays them, negative asks them to pay");
            frame.render_interactive_widget_on_layer(button, split[target], layer);
        }
        frame.render_widget(Paragraph::new(self.balance_label()).centered(), split[2]);
    }

    fn summary_lines(&self, world: &World) -> AppResult<Vec<Line<'static>>> {
        let own_team = world.get_own_team()?;
        let seats = own_team.player_ids.len();
        let seats_after = seats - usize::from(self.own_offer.is_some()) + 1;
        let balance = own_team.balance();
        let balance_after = match self.satoshis.cmp(&0) {
            Ordering::Greater => format!(
                "{} (held until it ends)",
                format_satoshi(balance.saturating_sub(satoshi_amount(self.satoshis)))
            ),
            Ordering::Less => format!(
                "{} (on accept)",
                format_satoshi(
                    balance.saturating_add(satoshi_amount(self.satoshis.saturating_neg()))
                )
            ),
            Ordering::Equal => format_satoshi(balance),
        };
        let mut lines = vec![Line::from(format!(
            "Crew {seats} → {seats_after}    Balance {} → {balance_after}",
            format_satoshi(balance)
        ))];

        let pirate_note = self
            .dock_target
            .and(self.own_offer)
            .and_then(|id| world.players.get(&id))
            .map(|player| {
                format!(
                    "{} waits at the dock while the offer stands.",
                    player.info.short_name()
                )
            });
        if let Some(note) = pirate_note.or_else(|| self.warning.clone()) {
            lines.push(Line::from(note));
        }
        Ok(lines)
    }
}

impl SplitPanel for OfferOverlay {
    fn index(&self) -> Option<usize> {
        Some(self.side_index(self.focus))
    }

    fn max_index(&self) -> usize {
        self.roster(self.focus).len()
    }

    fn index_bound(&self) -> IndexBound {
        IndexBound::Clamp
    }

    fn set_index(&mut self, index: usize) {
        match self.focus {
            OfferSide::Own => self.own_index = index,
            OfferSide::Other => self.other_index = index,
        }
    }
}

impl Overlay for OfferOverlay {
    fn title(&self, world: &World) -> String {
        let crew = world
            .teams
            .get(&self.other_team_id)
            .map_or_else(|| "another crew".to_string(), |team| team.name.clone());
        match self.other_offer.and_then(|id| world.players.get(&id)) {
            Some(player) => format!("Offer to {crew} for {}", player.info.short_name()),
            None => format!("Offer to {crew}"),
        }
    }

    fn rect(&self, screen_area: Rect) -> Rect {
        centered_rect(screen_area, 80, 70, (76, 22))
    }

    fn update(&mut self, world: &World) -> AppResult<()> {
        let own_team = world.get_own_team()?;
        self.own_roster = own_team.active_player_ids();
        self.other_roster = match self.dock_target {
            Some(player_id) => vec![player_id],
            None => world
                .teams
                .get(&self.other_team_id)
                .map(|team| {
                    team.player_ids
                        .iter()
                        .copied()
                        .filter(|id| !team.is_parked(id))
                        .collect()
                })
                .unwrap_or_default(),
        };
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
        let body = Layout::vertical([
            Constraint::Min(6),
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(3),
        ])
        .split(area);

        let columns =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(body[0]);
        self.render_roster(
            frame,
            world,
            OfferSide::Own,
            "You give (optional)",
            columns[0],
            layer,
        );
        self.render_roster(frame, world, OfferSide::Other, "You get", columns[1], layer);
        self.render_satoshi_row(frame, body[1], layer);

        let summary = match self.blocker.as_ref() {
            Some(blocker) => Paragraph::new(blocker.clone())
                .centered()
                .block(default_block().border_style(UiStyle::WARNING)),
            None => Paragraph::new(self.summary_lines(world)?)
                .centered()
                .block(default_block().border_style(UiStyle::OK)),
        };
        frame.render_widget(summary, body[2]);

        let actions = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(24),
            Constraint::Length(12),
            Constraint::Fill(1),
        ])
        .split(body[3]);

        let mut send = Button::new("Send offer", UiCallback::SendOffer)
            .hover_text("Send this offer. Cash you pay is held until it ends.")
            .hotkey(ui_key::CREATE_TRADE)
            .block(default_block().border_style(UiStyle::OK));
        if let Some(blocker) = self.blocker.as_ref() {
            send.disable(Some(blocker.clone()));
        }
        frame.render_interactive_widget_on_layer(send, actions[1], layer);

        let cancel = Button::new(UiText::NO, UiCallback::CloseOverlay)
            .hover_text("Close without sending")
            .block(default_block().border_style(UiStyle::ERROR));
        frame.render_interactive_widget_on_layer(cancel, actions[2], layer);

        Ok(())
    }

    fn handle_key_events(&mut self, key_event: KeyEvent, world: &World) -> Option<UiCallback> {
        let (own_balance, their_balance) = self.balances(world);
        match key_event.code {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.type_digit(i64::from(c.to_digit(10)?), own_balance, their_balance);
            }
            KeyCode::Backspace => self.delete_digit(),
            KeyCode::Char('-') => self.flip_direction(own_balance, their_balance),
            KeyCode::Left | KeyCode::Right | ui_key::CYCLE_VIEW => {
                self.focus = match self.focus {
                    OfferSide::Own => OfferSide::Other,
                    OfferSide::Other => OfferSide::Own,
                };
            }
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            ui_key::YES_TO_DIALOG => {
                let side = self.focus;
                let player_id = *self.roster(side).get(self.side_index(side))?;
                self.set_offer_player(side, player_id);
            }
            _ => return None,
        }
        self.recompute_blockers(world);
        Some(UiCallback::None)
    }

    fn as_split_panel(&mut self) -> Option<&mut dyn SplitPanel> {
        Some(self)
    }

    fn help_content(&self) -> Option<(String, HelpContent)> {
        Some((
            "Offer".to_string(),
            HelpContent {
                description: "Offer a pirate, satoshi, or both for one of their pirates. \
                    One satoshi balance covers the whole offer: positive pays them, negative \
                    asks them to pay. Cash you pay is held until the offer is accepted, \
                    declined or retired."
                    .to_string(),
                links: vec![],
                controls: vec![
                    Line::from("  ←/→ or Tab  Switch between the two crews"),
                    Line::from("  ↑/↓         Move the highlight"),
                    Line::from("  Enter       Put the highlighted pirate on the table"),
                    Line::from("  0-9         Type the amount"),
                    Line::from("  -           Switch between paying and asking"),
                    Line::from("  P           Send the offer"),
                    Line::from("  Esc         Close without sending"),
                ],
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlay() -> OfferOverlay {
        OfferOverlay::new(TeamId::new_v4(), PlayerId::new_v4(), false, None)
    }

    #[test]
    fn test_paying_and_asking_the_same_amount_is_even() {
        let mut overlay = overlay();
        overlay.shift_satoshis(10_000, 50_000, 50_000);
        overlay.shift_satoshis(-10_000, 50_000, 50_000);
        assert_eq!(overlay.offer().2, 0);
        assert_eq!(overlay.balance_label(), "Even");

        overlay.shift_satoshis(-3_000, 50_000, 50_000);
        assert_eq!(
            overlay.balance_label(),
            format!("You ask {}", format_satoshi(3_000))
        );
    }

    #[test]
    fn test_the_balance_is_clamped_to_what_each_side_has() {
        let mut overlay = overlay();
        overlay.shift_satoshis(10_000, 4_000, 2_000);
        assert_eq!(overlay.offer().2, 4_000);
        overlay.shift_satoshis(-100_000, 4_000, 2_000);
        assert_eq!(overlay.offer().2, -2_000);
    }

    #[test]
    fn test_typing_keeps_the_direction_and_minus_flips_it() {
        let mut overlay = overlay();
        overlay.type_digit(1, 50_000, 50_000);
        overlay.type_digit(5, 50_000, 50_000);
        assert_eq!(overlay.offer().2, 15);
        overlay.flip_direction(50_000, 50_000);
        overlay.type_digit(0, 50_000, 50_000);
        assert_eq!(overlay.offer().2, -150);
        overlay.delete_digit();
        assert_eq!(overlay.offer().2, -15);
    }

    #[test]
    fn test_a_dock_offer_cannot_change_the_pirate_it_is_for() {
        let target = PlayerId::new_v4();
        let mut overlay = OfferOverlay::new(TeamId::new_v4(), target, true, None);
        overlay.set_offer_player(OfferSide::Other, PlayerId::new_v4());
        overlay.set_offer_player(OfferSide::Other, target);
        assert_eq!(overlay.offer().1, Some(target));
    }
}
