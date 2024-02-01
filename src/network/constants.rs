pub struct SubscriptionTopic {}

impl SubscriptionTopic {
    pub const TEAM: &'static str = "rebels-b2b-team";
    pub const CHALLENGE: &'static str = "rebels-b2b-challenge";
    pub const MSG: &'static str = "rebels-b2b-msg";
    pub const GAME: &'static str = "rebels-b2b-game";
    pub const SEED_INFO: &'static str = "rebels-b2b-seed";
}

pub const DEFAULT_PORT: u16 = 37202;
pub const DEFAULT_PORT_BACKUP: u16 = 37203;
pub const SEED_ADDRESS: &'static str = "/ip4/85.214.130.204/tcp/37202";
