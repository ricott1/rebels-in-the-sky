use super::resources::Resource;
use super::utils::is_default;
use crate::core::{
    Population, Upgrade, UpgradeableElement, DAYS, DOCK_BID_LOG_SIZE, DOCK_MIN_BID_RAISE_PERCENT,
    MAX_TAVERN_POPULATION, WEEKS,
};
use crate::types::{PlanetId, PlayerId, ResourceMap, StorableResourceMap, TeamId, Tick, TradeId};
use libp2p::PeerId;
use rand::prelude::Distribution;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use rand_distr::weighted::WeightedIndex;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display};
use strum::Display;
use strum_macros::EnumIter;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct DrinkingCompetition {
    participants: [PlayerId; 2],
}
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct Tavern {
    // The tavern increases the cove asteroid population,
    // which in turns means that tick_free_pirates populate the asteroid with free pirates.
    pub upkeep_cost: ResourceMap,
    pub drinking_competition: Option<DrinkingCompetition>,
}

impl Tavern {
    pub fn refresh_populations(
        &self,
        parent_planet_populations: &HashMap<Population, u32>,
        available_rum: u32,
        rng: &mut ChaCha8Rng,
    ) -> HashMap<Population, u32> {
        const POPULATION_WEIGHT_CAP: u32 = 1;
        // Rum/day at which the tavern fills all MAX slots ~50% of the time.
        const RUM_FOR_HALF_MAX: f64 = 5.0;

        // Per-slot fill chance, tuned so that available_rum == RUM_FOR_HALF_MAX gives
        // a full house (MAX pirates) 50% of the time, and double that guarantees MAX.
        // Only the rum actually consumed is considered.
        let fill_chance = (0.5_f64.powf(1.0 / MAX_TAVERN_POPULATION as f64) * available_rum as f64
            / RUM_FOR_HALF_MAX)
            .clamp(0.0, 1.0);

        let weights: Vec<(Population, u32)> = parent_planet_populations
            .iter()
            .map(|(&pop, &value)| (pop, value.min(POPULATION_WEIGHT_CAP)))
            .filter(|(_, weight)| *weight > 0)
            .collect();

        let mut populations = HashMap::default();
        if let Ok(distribution) = WeightedIndex::new(weights.iter().map(|(_, weight)| weight)) {
            for _ in 0..MAX_TAVERN_POPULATION {
                if rng.random_range(0.0..1.0) < fill_chance {
                    let population = weights[distribution.sample(rng)].0;
                    *populations.entry(population).or_insert(0) += 1;
                }
            }
        }

        populations
    }
}

/// A standing bid on a dock listing. The seller is the sole authority on which
/// bid is leading; this is the copy every peer sees, carried inside NetworkTeam.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct DockBid {
    pub team_id: TeamId,
    pub peer_id: PeerId,
    pub trade_id: TradeId,
    pub amount: u32,
    pub placed_on: Tick,
}

/// A bid that was made, kept for the auction log. Deliberately leaner than
/// `DockBid`: the peer and trade ids are settlement plumbing, and only the
/// standing bid ever needs them.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct DockBidRecord {
    pub team_id: TeamId,
    pub amount: u32,
    pub placed_on: Tick,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum DockOutcome {
    #[serde(alias = "Sold")]
    Signed {
        team_id: TeamId,
        #[serde(alias = "amount")]
        fee: u32,
    },
    Lapsed,
}

/// The seller's word on how one of their auctions ended.
///
/// Settlement used to be a three-way handshake, which fell apart whenever a
/// message was lost or the winner was away at the deadline. Now the seller just
/// decides, publishes this with their team, and every other crew acts on it
/// whenever they next see it - the outcome waits for you instead of being a
/// moment you can miss.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct DockReceipt {
    pub player_id: PlayerId,
    /// Together with `player_id`, identifies the auction: a pirate can be listed
    /// more than once.
    pub listed_on: Tick,
    pub outcome: DockOutcome,
    pub settled_on: Tick,
}

/// A pirate left at the dock. They still belong to the team and still take up a
/// crew seat, but they sit out games and hold no role until the auction closes.
/// The dock is not a place: every crew's listings are one galaxy-wide board.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct DockListing {
    pub player_id: PlayerId,
    /// Binding: pay this and the pirate signs on the spot, no waiting.
    #[serde(alias = "buy_now_price")]
    pub release_fee: u32,
    pub min_bid: u32,
    pub listed_on: Tick,
    pub expires_at: Tick,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub highest_bid: Option<DockBid>,
    /// Bids that have been outbid, newest first, capped at `DOCK_BID_LOG_SIZE`.
    /// The standing bid is not in here - it is `highest_bid`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub bid_log: Vec<DockBidRecord>,
}

impl DockListing {
    pub fn new(
        player_id: PlayerId,
        release_fee: u32,
        min_bid: u32,
        duration: Tick,
        now: Tick,
    ) -> Self {
        Self {
            player_id,
            release_fee,
            min_bid,
            listed_on: now,
            expires_at: now + duration,
            highest_bid: None,
            bid_log: Vec::new(),
        }
    }

    pub fn has_expired(&self, now: Tick) -> bool {
        now >= self.expires_at
    }

    /// Smallest amount that would take the lead: the reserve until someone bids,
    /// then the standing bid plus the minimum raise, never past the ceiling. A
    /// raise over the buy-now price would make the binding price unbuyable,
    /// since buying now is validated as a bid like any other.
    pub fn next_valid_bid(&self) -> u32 {
        match self.highest_bid.as_ref() {
            Some(bid) => bid
                .amount
                .saturating_add(Self::minimum_raise(bid.amount))
                .min(self.release_fee),
            None => self.min_bid,
        }
    }

    /// A percentage of a small enough bid rounds down to nothing, so the raise
    /// never drops below one satoshi.
    fn minimum_raise(amount: u32) -> u32 {
        let raise = amount as u64 * DOCK_MIN_BID_RAISE_PERCENT as u64 / 100;
        (raise as u32).max(1)
    }

    /// Takes the lead, retiring whoever held it into the log. Returns the
    /// displaced bid, which the caller refunds.
    pub fn record_bid(&mut self, bid: DockBid) -> Option<DockBid> {
        let displaced = self.highest_bid.replace(bid);
        if let Some(previous) = displaced.as_ref() {
            self.bid_log.insert(
                0,
                DockBidRecord {
                    team_id: previous.team_id,
                    amount: previous.amount,
                    placed_on: previous.placed_on,
                },
            );
            self.bid_log.truncate(DOCK_BID_LOG_SIZE);
        }
        displaced
    }
}

#[derive(Debug, Display, Clone, Copy, Serialize_repr, Deserialize_repr, PartialEq)]
#[repr(u8)]
pub enum SpaceCoveState {
    UnderConstruction,
    Ready,
}

fn default_upgrades() -> HashSet<SpaceCoveUpgradeTarget> {
    HashSet::from([SpaceCoveUpgradeTarget::TeleportationPad])
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpaceCove {
    state: SpaceCoveState,
    pub planet_id: PlanetId,
    #[serde(default)]
    pub name: String,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub pending_upgrade: Option<Upgrade<SpaceCoveUpgradeTarget>>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default = "default_upgrades")]
    pub upgrades: HashSet<SpaceCoveUpgradeTarget>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub tavern: Option<Tavern>,
    #[serde(skip_serializing_if = "is_default")]
    #[serde(default)]
    pub resources: ResourceMap,
}

impl SpaceCove {
    pub fn under_construction(planet_id: PlanetId) -> Self {
        Self {
            state: SpaceCoveState::UnderConstruction,
            planet_id,
            name: String::new(),
            pending_upgrade: None,
            upgrades: HashSet::default(),
            tavern: None,
            resources: ResourceMap::default(),
        }
    }

    pub fn finish_contruction(&mut self) {
        self.state = SpaceCoveState::Ready;
        self.upgrades
            .insert(SpaceCoveUpgradeTarget::TeleportationPad);
    }

    pub fn is_ready(&self) -> bool {
        self.state == SpaceCoveState::Ready
    }

    pub fn has_stadium(&self) -> bool {
        self.upgrades.contains(&SpaceCoveUpgradeTarget::Stadium)
    }

    pub fn has_market(&self) -> bool {
        self.upgrades.contains(&SpaceCoveUpgradeTarget::Market)
    }

    pub fn can_pay_tavern_upkeep(&self) -> bool {
        let rum_per_day = self
            .tavern
            .as_ref()
            .and_then(|tavern| tavern.upkeep_cost.get(&Resource::RUM).copied())
            .unwrap_or(0);
        self.resources.value(&Resource::RUM) >= rum_per_day
    }

    /// Draws the tavern's daily rum from the cove store, returning how much was actually available.
    pub fn consume_daily_rum(&mut self) -> u32 {
        let rum_per_day = self
            .tavern
            .as_ref()
            .and_then(|tavern| tavern.upkeep_cost.get(&Resource::RUM).copied())
            .unwrap_or(0);
        let effective_rum = rum_per_day.min(self.resources.value(&Resource::RUM));
        self.resources.saturating_sub(Resource::RUM, effective_rum);

        effective_rum
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SpaceCoveUpgradeTarget {
    Market,
    Stadium,
    Tavern,
    TeleportationPad, // NOTE: this exists also on the PlanetUpgradeTarget. We repeat it here for convenience
}

impl Display for SpaceCoveUpgradeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TeleportationPad => write!(f, "Teleportation Pad"),
            Self::Market => write!(f, "Market"),
            Self::Stadium => write!(f, "Stadium"),
            Self::Tavern => write!(f, "Tavern"),
        }
    }
}

impl UpgradeableElement for SpaceCoveUpgradeTarget {
    fn next(&self) -> Option<Self> {
        None
    }

    fn previous(&self) -> Option<Self> {
        None
    }

    fn can_be_upgraded(&self) -> bool {
        true
    }

    fn upgrade_cost(&self) -> Vec<(Resource, u32)> {
        match self {
            Self::TeleportationPad => {
                vec![]
            }
            Self::Market => {
                vec![
                    (Resource::SATOSHI, 80_000),
                    (Resource::SCRAPS, 60),
                    (Resource::GOLD, 5),
                    (Resource::RUM, 25),
                ]
            }
            Self::Stadium => vec![
                (Resource::SATOSHI, 70_000),
                (Resource::SCRAPS, 220),
                (Resource::GOLD, 80),
            ],
            Self::Tavern => vec![
                (Resource::SATOSHI, 60_000),
                (Resource::SCRAPS, 100),
                (Resource::RUM, 150),
            ],
        }
    }

    fn upgrade_duration(&self) -> Tick {
        match self {
            Self::TeleportationPad => 0,
            Self::Market => 2 * DAYS,
            Self::Stadium => WEEKS,
            Self::Tavern => 3 * DAYS,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::TeleportationPad => "The teleportation pad allows to travel to the cove instantaneously for 1 Rum per pirate.",
            Self::Market => "A nice opportunity to trade your nice little goodies.",
            Self::Stadium => "Allows to organize tournaments in the space cove",
            Self::Tavern => "The best way to attract talented pirates to the cove",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TeamId;

    #[test]
    fn test_dock_listing_round_trips() {
        let player_id = PlayerId::new_v4();
        let mut listing = DockListing::new(player_id, 42_000, 7_000, 1_000, 500);
        listing.record_bid(DockBid {
            team_id: TeamId::new_v4(),
            peer_id: libp2p::PeerId::random(),
            trade_id: TradeId::new_v4(),
            amount: 8_000,
            placed_on: 550,
        });
        listing.record_bid(DockBid {
            team_id: TeamId::new_v4(),
            peer_id: libp2p::PeerId::random(),
            trade_id: TradeId::new_v4(),
            amount: 9_001,
            placed_on: 600,
        });

        let json = serde_json::to_string(&listing).unwrap();
        let restored: DockListing = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, listing);
        assert_eq!(restored.expires_at, 1_500);
        assert_eq!(restored.highest_bid.as_ref().unwrap().amount, 9_001);
        assert_eq!(restored.bid_log, listing.bid_log);
    }

    #[test]
    fn test_next_valid_bid_tracks_the_standing_bid() {
        let player_id = PlayerId::new_v4();
        let mut listing = DockListing::new(player_id, 42_000, 7_000, 1_000, 0);
        assert_eq!(
            listing.next_valid_bid(),
            7_000,
            "reserve until someone bids"
        );

        listing.highest_bid = Some(DockBid {
            team_id: TeamId::new_v4(),
            peer_id: libp2p::PeerId::random(),
            trade_id: TradeId::new_v4(),
            amount: 9_000,
            placed_on: 0,
        });
        assert_eq!(
            listing.next_valid_bid(),
            9_450,
            "must beat the lead by the minimum raise"
        );
    }

    #[test]
    fn test_the_minimum_raise_is_never_nothing() {
        // A percentage of a handful of satoshi rounds down to zero, which would
        // let a bidder retake the lead for free.
        let mut listing = DockListing::new(PlayerId::new_v4(), 100, 1, 1_000, 0);
        listing.highest_bid = Some(DockBid {
            team_id: TeamId::new_v4(),
            peer_id: libp2p::PeerId::random(),
            trade_id: TradeId::new_v4(),
            amount: 10,
            placed_on: 0,
        });
        assert_eq!(listing.next_valid_bid(), 11);
    }

    #[test]
    fn test_the_raise_never_climbs_past_the_release_fee() {
        // Otherwise the advertised binding price becomes unbuyable: the buy-now
        // button is validated as a bid, and a raise over the ceiling fails it.
        let mut listing = DockListing::new(PlayerId::new_v4(), 1_000, 200, 1_000, 0);
        listing.highest_bid = Some(DockBid {
            team_id: TeamId::new_v4(),
            peer_id: libp2p::PeerId::random(),
            trade_id: TradeId::new_v4(),
            amount: 990,
            placed_on: 0,
        });
        assert_eq!(listing.next_valid_bid(), 1_000);
    }

    #[test]
    fn test_the_bid_log_keeps_the_most_recent_outbid_bids_newest_first() {
        let mut listing = DockListing::new(PlayerId::new_v4(), 42_000, 1_000, 1_000, 0);

        let overflow = DOCK_BID_LOG_SIZE as u32 + 3;
        for amount in 1..=overflow {
            listing.record_bid(DockBid {
                team_id: TeamId::new_v4(),
                peer_id: libp2p::PeerId::random(),
                trade_id: TradeId::new_v4(),
                amount,
                placed_on: amount as Tick,
            });
        }

        assert_eq!(listing.bid_log.len(), DOCK_BID_LOG_SIZE);
        assert_eq!(
            listing.highest_bid.expect("the lead").amount,
            overflow,
            "the standing bid stays out of the log"
        );
        assert_eq!(listing.bid_log[0].amount, overflow - 1, "newest first");
        assert_eq!(
            listing.bid_log.last().expect("a logged bid").amount,
            overflow - DOCK_BID_LOG_SIZE as u32,
            "the oldest bids fall off the end"
        );
    }

    #[test]
    fn test_a_listing_without_bids_serializes_as_before() {
        let listing = DockListing::new(PlayerId::new_v4(), 42_000, 7_000, 1_000, 0);
        let json = serde_json::to_string(&listing).unwrap();
        assert!(
            !json.contains("bid_log"),
            "an auction nobody has bid on must not be written: {json}"
        );
    }

    #[test]
    fn test_legacy_ready_cove_defaults_to_teleportation_pad() {
        let json = r#"{"state":1,"planet_id":"00000000-0000-0000-0000-000000000000"}"#;
        let cove: SpaceCove = serde_json::from_str(json).unwrap();
        assert!(cove.is_ready());
        assert!(cove
            .upgrades
            .contains(&SpaceCoveUpgradeTarget::TeleportationPad));
    }
}
