use super::types::NetworkRequestState;
use crate::world::{player::Player, skill::Rated};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trade {
    pub state: NetworkRequestState,
    pub proposer_peer_id: PeerId,
    pub target_peer_id: PeerId,
    pub proposer_player: Player,
    pub target_player: Player,
    pub extra_satoshis: i64,
}

impl Trade {
    pub fn new(
        proposer_peer_id: PeerId,
        target_peer_id: PeerId,
        proposer_player: Player,
        target_player: Player,
        extra_satoshis: i64,
    ) -> Self {
        Self {
            state: NetworkRequestState::Syn,
            proposer_peer_id,
            target_peer_id,
            proposer_player,
            target_player,
            extra_satoshis,
        }
    }

    pub fn format(&self) -> String {
        format!(
            "Trade ({}): {} {} ⇄ {} {} {:+}",
            self.state,
            self.proposer_player.info.short_name(),
            self.proposer_player.stars(),
            self.target_player.info.short_name(),
            self.target_player.stars(),
            self.extra_satoshis
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Trade;
    use crate::{app::App, types::AppResult, ui::ui_callback::UiCallback, world::skill::MAX_SKILL};
    use libp2p::PeerId;
    use rand::{seq::IteratorRandom, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_local_trade_success() -> AppResult<()> {
        let mut app = App::test_default()?;

        let own_team = app.world.get_team_or_err(&app.world.own_team_id)?.clone();
        let proposer_player_id = own_team.player_ids[0];
        let mut proposer_player = app.world.players.get(&proposer_player_id).unwrap().clone();
        assert!(proposer_player.team == Some(own_team.id));

        // Increase player stats to increase bare value and make the trade accepted
        proposer_player.info.age = proposer_player.info.population.min_age();
        proposer_player.special_trait = Some(crate::world::player::Trait::Killer);
        proposer_player.athletics.quickness = MAX_SKILL;
        proposer_player.athletics.strength = MAX_SKILL;
        proposer_player.athletics.vertical = MAX_SKILL;
        proposer_player.athletics.stamina = MAX_SKILL;
        proposer_player.offense.brawl = MAX_SKILL;
        proposer_player.offense.close_range = MAX_SKILL;
        proposer_player.offense.medium_range = MAX_SKILL;
        proposer_player.offense.long_range = MAX_SKILL;

        app.world
            .players
            .insert(proposer_player.id, proposer_player);

        // FIXME: This fails if there is only one team
        let mut target_team = app
            .world
            .teams
            .values()
            .filter(|team| team.id != own_team.id)
            .choose(&mut rand::rng())
            .expect("There should be one other team")
            .clone();
        target_team.current_location = own_team.current_location;
        let target_team_id = target_team.id;
        let target_player_id = target_team.player_ids[0];
        app.world.teams.insert(target_team.id, target_team);

        let target_player = app.world.get_player_or_err(&target_player_id)?;
        assert!(target_player.team == Some(target_team_id));

        let cb = UiCallback::CreateTradeProposal {
            proposer_player_id,
            target_player_id,
        };
        assert!(cb.call(&mut app).is_ok());

        let proposer_player = app.world.get_player_or_err(&proposer_player_id)?;
        assert!(proposer_player.team == Some(target_team_id));

        let target_player = app.world.get_player_or_err(&target_player_id)?;
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
        let target_player = app.world.get_player_or_err(&target_player_id)?;
        assert!(target_player.team == Some(target_team.id));

        let cb = UiCallback::CreateTradeProposal {
            proposer_player_id,
            target_player_id,
        };

        assert!(cb.call(&mut app).unwrap_err().to_string() == "Not on the same planet".to_string());

        let proposer_player = app.world.get_player_or_err(&proposer_player_id)?;
        assert!(proposer_player.team == Some(app.world.own_team_id));

        let target_player = app.world.get_player_or_err(&target_player_id)?;
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

        let cb = UiCallback::CreateTradeProposal {
            proposer_player_id,
            target_player_id,
        };
        assert!(
            cb.call(&mut app).unwrap_err().to_string() == "Cannot trade with oneself".to_string()
        );

        let proposer_player = app.world.get_player_or_err(&proposer_player_id)?;
        assert!(proposer_player.team == Some(app.world.own_team_id));

        let target_player = app.world.get_player_or_err(&target_player_id)?;
        assert!(target_player.team == Some(app.world.own_team_id));

        Ok(())
    }

    #[ignore]
    #[test]
    fn test_network_trade() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;
        let rng = &mut ChaCha8Rng::from_os_rng();

        let home_planet_id = world.planets.keys().next().unwrap().clone();

        let target_team_id = world.generate_random_team(
            rng,
            home_planet_id,
            "target team".into(),
            "ship_name".into(),
        )?;

        let mut target_team = world.get_team_or_err(&target_team_id)?.clone();

        let target_team_peer_id = PeerId::random();
        target_team.peer_id = Some(target_team_peer_id);
        let target_player_id = target_team.player_ids[0];
        world.teams.insert(target_team.id, target_team);

        let own_team = world.get_own_team()?;
        let own_team_peer_id = PeerId::random();

        let proposer_player_id = own_team.player_ids[0];
        let proposer_player = world.get_player_or_err(&proposer_player_id)?.clone();

        let target_player = world.get_player_or_err(&target_player_id)?.clone();

        let _trade = Trade::new(
            own_team_peer_id,
            target_team_peer_id,
            proposer_player,
            target_player,
            0,
        );

        Ok(())
    }
}
