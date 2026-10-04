use super::types::NetworkRequestState;
use crate::app_version;
use crate::core::{player::Player, skill::Rated, OfferKind};
use crate::types::{SystemTimeTick, TeamId, Tick, TradeId};
use crate::ui::utils::format_satoshi;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trade {
    pub id: TradeId,
    pub state: NetworkRequestState,
    pub route: OfferKind,
    /// Mirrors `Challenge::app_version`: the wire shape has changed, so a peer on
    /// a different minor version gets a readable refusal instead of silence.
    pub app_version: [usize; 3],
    /// Gives offer lists a stable order. Without it the swarm panel sorts by
    /// `HashMap` iteration order, which reshuffles on insert.
    pub created_at: Tick,
    pub proposer_peer_id: PeerId,
    pub target_peer_id: PeerId,
    pub proposer_team_id: TeamId,
    pub target_team_id: TeamId,
    pub proposer_player: Option<Player>,
    pub target_player: Player,
    /// Positive: the proposer pays. Negative: the proposer asks the target to pay.
    pub satoshis: i64,
}

impl Trade {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        route: OfferKind,
        proposer_peer_id: PeerId,
        target_peer_id: PeerId,
        proposer_team_id: TeamId,
        target_team_id: TeamId,
        proposer_player: Option<Player>,
        target_player: Player,
        satoshis: i64,
    ) -> Self {
        Self {
            id: TradeId::new_v4(),
            state: NetworkRequestState::Syn,
            route,
            app_version: app_version(),
            created_at: Tick::now(),
            proposer_peer_id,
            target_peer_id,
            proposer_team_id,
            target_team_id,
            proposer_player,
            target_player,
            satoshis,
        }
    }

    pub fn proposer_pays(&self) -> u32 {
        satoshi_amount(self.satoshis)
    }

    pub fn target_pays(&self) -> u32 {
        satoshi_amount(self.satoshis.saturating_neg())
    }

    /// True when the peer is close enough to speak the same wire format.
    pub fn app_version_matches(&self) -> bool {
        let [major, minor, _] = app_version();
        let [their_major, their_minor, _] = self.app_version;
        major == their_major && minor == their_minor
    }

    pub fn offered(&self) -> String {
        offered_phrase(self.proposer_player.as_ref(), self.satoshis)
    }

    pub fn format(&self) -> String {
        format!(
            "Offer ({}): {} for {} {}",
            self.state,
            self.offered(),
            self.target_player.info.short_name(),
            self.target_player.stars(),
        )
    }
}

pub fn satoshi_amount(satoshis: i64) -> u32 {
    u32::try_from(satoshis.max(0)).unwrap_or(u32::MAX)
}

pub fn satoshi_suffix(satoshis: i64) -> String {
    match satoshis.cmp(&0) {
        Ordering::Greater => format!(" + {}", format_satoshi(satoshi_amount(satoshis))),
        Ordering::Less => format!(
            ", asking {}",
            format_satoshi(satoshi_amount(satoshis.saturating_neg()))
        ),
        Ordering::Equal => String::new(),
    }
}

pub fn offered_phrase(pirate: Option<&Player>, satoshis: i64) -> String {
    let pirate = pirate.map(|player| format!("{} {}", player.info.short_name(), player.stars()));
    match (pirate, satoshis.cmp(&0)) {
        (Some(pirate), _) => format!("{pirate}{}", satoshi_suffix(satoshis)),
        (None, Ordering::Greater) => format_satoshi(satoshi_amount(satoshis)),
        (None, _) => format!("nothing{}", satoshi_suffix(satoshis)),
    }
}

#[cfg(test)]
mod tests {
    use super::Trade;
    use crate::types::{PlayerId, SystemTimeTick, TeamId, Tick};
    use crate::{
        app::App,
        core::{skill::MAX_SKILL, OfferKind},
        types::{AppResult, HashMapWithResult},
        ui::UiCallback,
    };
    use libp2p::PeerId;
    use rand::{seq::IteratorRandom, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_local_trade_success() -> AppResult<()> {
        let mut app = App::test_default()?;

        let own_team = app.world.teams.get_or_err(&app.world.own_team_id)?.clone();
        let proposer_player_id = own_team.player_ids[0];
        let mut proposer_player = app.world.players.get(&proposer_player_id).unwrap().clone();
        assert!(proposer_player.team == Some(own_team.id));

        // Max all skills to guarantee bare_hiring_value exceeds any target player
        proposer_player.info.age = proposer_player.info.population.min_age();
        proposer_player.special_trait = Some(crate::core::player::Trait::Killer);
        proposer_player.athletics.quickness = MAX_SKILL;
        proposer_player.athletics.strength = MAX_SKILL;
        proposer_player.athletics.vertical = MAX_SKILL;
        proposer_player.athletics.stamina = MAX_SKILL;
        proposer_player.offense.brawl = MAX_SKILL;
        proposer_player.offense.close_range = MAX_SKILL;
        proposer_player.offense.medium_range = MAX_SKILL;
        proposer_player.offense.long_range = MAX_SKILL;
        proposer_player.defense.steal = MAX_SKILL;
        proposer_player.defense.block = MAX_SKILL;
        proposer_player.defense.perimeter_defense = MAX_SKILL;
        proposer_player.defense.interior_defense = MAX_SKILL;
        proposer_player.technical.passing = MAX_SKILL;
        proposer_player.technical.ball_handling = MAX_SKILL;
        proposer_player.technical.post_moves = MAX_SKILL;
        proposer_player.technical.rebounds = MAX_SKILL;
        proposer_player.mental.vision = MAX_SKILL;
        proposer_player.mental.aggression = MAX_SKILL;
        proposer_player.mental.intuition = MAX_SKILL;
        proposer_player.mental.charisma = MAX_SKILL;

        app.world
            .players
            .insert(proposer_player.id, proposer_player);

        let mut target_team = app
            .world
            .teams
            .values()
            .filter(|team| team.id != own_team.id)
            .choose(&mut ChaCha8Rng::from_rng(&mut rand::rng()))
            .expect("There should be one other team")
            .clone();
        target_team.current_location = own_team.current_location;
        let target_team_id = target_team.id;
        let target_player_id = target_team.player_ids[0];
        app.world.teams.insert(target_team.id, target_team);

        let target_player = app.world.players.get_or_err(&target_player_id)?;
        assert!(target_player.team == Some(target_team_id));

        let cb = UiCallback::MakeOffer {
            target_player_id,
            pirate: Some(proposer_player_id),
            satoshis: 0,
        };
        assert!(cb.call(&mut app).is_ok());

        let proposer_player = app.world.players.get_or_err(&proposer_player_id)?;
        assert!(proposer_player.team == Some(target_team_id));

        let target_player = app.world.players.get_or_err(&target_player_id)?;
        assert!(target_player.team == Some(app.world.own_team_id));

        Ok(())
    }

    #[test]
    fn test_local_trade_fail_not_same_planet() -> AppResult<()> {
        let mut app = App::test_default()?;

        let own_team = app.world.teams.get(&app.world.own_team_id).unwrap();
        let proposer_player_id = own_team.player_ids[0];
        let proposer_player = app.world.players.get(&proposer_player_id).unwrap();
        assert!(proposer_player.team == Some(own_team.id));

        let target_team = app
            .world
            .teams
            .values()
            .filter(|team| team.home_planet_id != own_team.home_planet_id)
            .choose(&mut rand::rng())
            .expect("There should be one team");

        let target_team_id = target_team.id;

        let target_player_id = target_team.player_ids[0];
        let target_player = app.world.players.get_or_err(&target_player_id)?;
        assert!(target_player.team == Some(target_team.id));

        let cb = UiCallback::MakeOffer {
            target_player_id,
            pirate: Some(proposer_player_id),
            satoshis: 0,
        };

        assert!(cb.call(&mut app).unwrap_err().to_string() == "Not on the same planet".to_string());

        let proposer_player = app.world.players.get_or_err(&proposer_player_id)?;
        assert!(proposer_player.team == Some(app.world.own_team_id));

        let target_player = app.world.players.get_or_err(&target_player_id)?;
        assert!(target_player.team == Some(target_team_id));

        Ok(())
    }

    #[test]
    fn test_local_trade_fail_trade_with_oneself() -> AppResult<()> {
        let mut app = App::test_default()?;

        let own_team = app.world.teams.get(&app.world.own_team_id).unwrap();
        let proposer_player_id = own_team.player_ids[0];
        let proposer_player = app.world.players.get(&proposer_player_id).unwrap();
        assert!(proposer_player.team == Some(own_team.id));

        let target_player_id = own_team.player_ids[1];
        let target_player = app.world.players.get(&target_player_id).unwrap();
        assert!(target_player.team == Some(own_team.id));

        let cb = UiCallback::MakeOffer {
            target_player_id,
            pirate: Some(proposer_player_id),
            satoshis: 0,
        };
        assert!(
            cb.call(&mut app).unwrap_err().to_string() == "Cannot trade with oneself".to_string()
        );

        let proposer_player = app.world.players.get_or_err(&proposer_player_id)?;
        assert!(proposer_player.team == Some(app.world.own_team_id));

        let target_player = app.world.players.get_or_err(&target_player_id)?;
        assert!(target_player.team == Some(app.world.own_team_id));

        Ok(())
    }

    /// Builds a local crew swap between the own team and another team parked on
    /// the same planet, without going near the network.
    fn crew_swap_setup(app: &mut App) -> AppResult<(Trade, PlayerId, PlayerId, TeamId, TeamId)> {
        let own_team = app.world.get_own_team()?.clone();
        let mut target_team = app
            .world
            .teams
            .values()
            .filter(|team| team.id != own_team.id && team.player_ids.len() >= 2)
            .choose(&mut ChaCha8Rng::from_rng(&mut rand::rng()))
            .expect("There should be one other team")
            .clone();
        target_team.current_location = own_team.current_location;
        let target_team_id = target_team.id;
        let target_player_id = target_team.player_ids[0];
        app.world.teams.insert(target_team_id, target_team);

        let proposer_player_id = own_team.player_ids[0];
        let proposer_player = app.world.players.get_or_err(&proposer_player_id)?.clone();
        let target_player = app.world.players.get_or_err(&target_player_id)?.clone();

        let peer = PeerId::random();
        let trade = Trade::new(
            OfferKind::Direct,
            peer,
            PeerId::random(),
            own_team.id,
            target_team_id,
            Some(proposer_player),
            target_player,
            0,
        );

        Ok((
            trade,
            proposer_player_id,
            target_player_id,
            own_team.id,
            target_team_id,
        ))
    }

    #[test]
    fn test_apply_trade_moves_both_pirates_and_the_money() -> AppResult<()> {
        let mut app = App::test_default()?;
        let (mut trade, proposer_id, target_id, own_id, target_team_id) =
            crew_swap_setup(&mut app)?;
        trade.satoshis = 5_000;

        let own_before = app.world.teams.get_or_err(&own_id)?.balance();
        let target_before = app.world.teams.get_or_err(&target_team_id)?.balance();

        app.world.apply_trade(&trade, Tick::now())?;

        assert_eq!(
            app.world.players.get_or_err(&proposer_id)?.team,
            Some(target_team_id)
        );
        assert_eq!(app.world.players.get_or_err(&target_id)?.team, Some(own_id));
        assert_eq!(
            app.world.teams.get_or_err(&own_id)?.balance(),
            own_before - 5_000
        );
        assert_eq!(
            app.world.teams.get_or_err(&target_team_id)?.balance(),
            target_before + 5_000
        );
        Ok(())
    }

    /// The old release-then-add pair could half-apply, leaving both pirates as
    /// free agents. Nothing may change when a trade is refused.
    #[test]
    fn test_a_refused_trade_changes_nothing() -> AppResult<()> {
        let mut app = App::test_default()?;
        let (mut trade, proposer_id, target_id, own_id, target_team_id) =
            crew_swap_setup(&mut app)?;

        // More satoshi than the proposer has: refused at validation.
        trade.satoshis = app.world.teams.get_or_err(&own_id)?.balance() as i64 + 1;

        let own_roster = app.world.teams.get_or_err(&own_id)?.player_ids.clone();
        let target_roster = app
            .world
            .teams
            .get_or_err(&target_team_id)?
            .player_ids
            .clone();
        let own_balance = app.world.teams.get_or_err(&own_id)?.balance();
        let target_balance = app.world.teams.get_or_err(&target_team_id)?.balance();

        assert!(app.world.apply_trade(&trade, Tick::now()).is_err());

        assert_eq!(app.world.teams.get_or_err(&own_id)?.player_ids, own_roster);
        assert_eq!(
            app.world.teams.get_or_err(&target_team_id)?.player_ids,
            target_roster
        );
        assert_eq!(app.world.teams.get_or_err(&own_id)?.balance(), own_balance);
        assert_eq!(
            app.world.teams.get_or_err(&target_team_id)?.balance(),
            target_balance
        );
        assert_eq!(
            app.world.players.get_or_err(&proposer_id)?.team,
            Some(own_id)
        );
        assert_eq!(
            app.world.players.get_or_err(&target_id)?.team,
            Some(target_team_id)
        );
        Ok(())
    }

    #[test]
    fn test_apply_trade_is_idempotent() -> AppResult<()> {
        let mut app = App::test_default()?;
        let (mut trade, _, _, own_id, target_team_id) = crew_swap_setup(&mut app)?;
        trade.satoshis = 5_000;

        let own_before = app.world.teams.get_or_err(&own_id)?.balance();
        app.world.apply_trade(&trade, Tick::now())?;
        let own_after = app.world.teams.get_or_err(&own_id)?.balance();

        // A redelivered Ack must not move the money a second time.
        app.world.apply_trade(&trade, Tick::now())?;
        assert_eq!(app.world.teams.get_or_err(&own_id)?.balance(), own_after);
        assert_eq!(own_after, own_before - 5_000);
        let _ = target_team_id;
        Ok(())
    }

    /// Releasing applied a morale malus and hiring floored morale at the hire
    /// bonus, so a trade used to leave a low-morale pirate happier than before.
    #[test]
    fn test_a_traded_pirate_keeps_their_morale() -> AppResult<()> {
        let mut app = App::test_default()?;
        let (trade, proposer_id, _, own_id, _) = crew_swap_setup(&mut app)?;

        let mut player = app.world.players.get_or_err(&proposer_id)?.clone();
        player.morale = 3.0;
        app.world.players.insert(proposer_id, player);

        app.world.apply_trade(&trade, Tick::now())?;

        let moved = app.world.players.get_or_err(&proposer_id)?;
        assert_eq!(moved.morale, 3.0, "a trade is not a firing plus a hiring");
        // They remember the old crew, and start fresh with the new one.
        assert!(moved
            .opinions
            .contains_key(&crate::core::PlayerOpinion::Team { team_id: own_id }));
        assert!(moved
            .opinions
            .contains_key(&crate::core::PlayerOpinion::OwnTeam));
        Ok(())
    }

    #[test]
    fn test_app_version_mismatch_is_detected() -> AppResult<()> {
        let mut app = App::test_default()?;
        let (mut trade, _, _, _, _) = crew_swap_setup(&mut app)?;
        assert!(trade.app_version_matches());

        trade.app_version[1] += 1;
        assert!(
            !trade.app_version_matches(),
            "a peer on another minor version speaks a different wire format"
        );
        Ok(())
    }

    #[test]
    fn test_offered_phrase_reads_naturally() {
        use super::offered_phrase;
        use crate::ui::utils::format_satoshi;

        assert_eq!(offered_phrase(None, 2_000), format_satoshi(2_000));
        assert_eq!(
            offered_phrase(None, -2_000),
            format!("nothing, asking {}", format_satoshi(2_000))
        );
        assert_eq!(offered_phrase(None, 0), "nothing");
    }

    #[test]
    fn test_a_negative_balance_makes_the_target_pay() -> AppResult<()> {
        let mut app = App::test_default()?;
        let (mut trade, _, _, own_id, target_team_id) = crew_swap_setup(&mut app)?;
        trade.satoshis = -3_000;

        let own_before = app.world.teams.get_or_err(&own_id)?.balance();
        let target_before = app.world.teams.get_or_err(&target_team_id)?.balance();
        app.world.apply_trade(&trade, Tick::now())?;

        assert_eq!(
            app.world.teams.get_or_err(&own_id)?.balance(),
            own_before + 3_000
        );
        assert_eq!(
            app.world.teams.get_or_err(&target_team_id)?.balance(),
            target_before - 3_000
        );
        Ok(())
    }

    #[ignore]
    #[test]
    fn test_network_trade() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;
        let rng = &mut ChaCha8Rng::from_rng(&mut rand::rng());

        let home_planet_id = world.planets.keys().next().unwrap().clone();

        let target_team_id = world.generate_random_team(
            rng,
            home_planet_id,
            "target team".into(),
            "ship_name".into(),
        )?;

        let mut target_team = world.teams.get_or_err(&target_team_id)?.clone();

        let target_team_peer_id = PeerId::random();
        target_team.peer_id = Some(target_team_peer_id);
        let target_player_id = target_team.player_ids[0];
        world.teams.insert(target_team.id, target_team);

        let own_team = world.get_own_team()?;
        let own_team_id = own_team.id;
        let own_team_peer_id = PeerId::random();

        let proposer_player_id = own_team.player_ids[0];
        let proposer_player = world.players.get_or_err(&proposer_player_id)?.clone();

        let target_player = world.players.get_or_err(&target_player_id)?.clone();

        let _trade = Trade::new(
            OfferKind::Direct,
            own_team_peer_id,
            target_team_peer_id,
            own_team_id,
            target_team_id,
            Some(proposer_player),
            target_player,
            0,
        );

        Ok(())
    }
}
