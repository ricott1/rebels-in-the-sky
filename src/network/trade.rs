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
            self.proposer_player.info.shortened_name(),
            self.proposer_player.stars(),
            self.target_player.info.shortened_name(),
            self.target_player.stars(),
            self.extra_satoshis
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Trade;
    use crate::{app::App, types::AppResult};
    use libp2p::PeerId;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[ignore]
    #[test]
    fn test_trade() -> AppResult<()> {
        let mut app = App::test_default()?;

        let world = &mut app.world;
        let rng = &mut ChaCha8Rng::from_entropy();

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
