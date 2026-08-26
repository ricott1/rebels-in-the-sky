use crate::{
    core::{GameSkill, Skill, MAX_SKILL},
    types::PlayerId,
};
use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

const POTENTIAL_TIERS: [&str; 8] = [
    "washed-up",
    "benchwarmer",
    "role player",
    "steady contributor",
    "promising prospect",
    "red giant",
    "supernova",
    "galactic talent",
];
// Half-width of the shown band, in tiers, at zero scouting.
const POTENTIAL_UNCERTAINTY_MAX: f32 = 3.0;
const SKILLS_REVEALED_PER_SCOUTING: f32 = 1.25;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ScoutReport {
    scouting: Skill,
    seed: u64, // player.id.as_u64_pair().0 - stable per-player randomness
    #[serde(skip)]
    skills: [usize; 20], // Cached order of scouted skills, derived from seed. Not serialized.
}

impl ScoutReport {
    pub fn new(id: PlayerId, scouting: Skill) -> Self {
        let mut report = Self {
            scouting,
            seed: id.as_u64_pair().0,
            skills: [0; 20],
        };
        report.rebuild_caches();
        report
    }

    // Rebuild the #[serde(skip)] caches from the serialized seed; call after load.
    pub fn rebuild_caches(&mut self) {
        let rng = &mut ChaCha8Rng::seed_from_u64(self.seed);
        self.skills = std::array::from_fn(|i| i);
        self.skills.shuffle(rng);
    }

    pub fn scouting(&self) -> Skill {
        self.scouting
    }

    pub fn add_scouting(&mut self, delta: Skill) {
        self.scouting = (self.scouting + delta).bound();
    }

    pub fn raise_scouting_to(&mut self, floor: Skill) {
        self.scouting = self.scouting.max(floor).bound();
    }

    pub fn is_skill_scouted(&self, skill_index: usize) -> bool {
        let known = ((SKILLS_REVEALED_PER_SCOUTING * self.scouting).bound() as usize)
            .min(self.skills.len());
        self.skills[..known].contains(&skill_index)
    }

    pub fn is_role_scouted(&self, role_value: Skill) -> bool {
        self.scouting >= MAX_SKILL - role_value
    }

    pub fn is_special_trait_scouted(&self) -> bool {
        self.scouting >= 0.5 * MAX_SKILL
    }

    pub fn potential_description(&self, potential: Skill) -> String {
        let n = POTENTIAL_TIERS.len() as i32;
        let tier = ((potential / MAX_SKILL * n as f32) as i32).clamp(0, n - 1);

        let h =
            ((MAX_SKILL - self.scouting) / MAX_SKILL * POTENTIAL_UNCERTAINTY_MAX).round() as i32;
        if h <= 0 {
            return POTENTIAL_TIERS[tier as usize].to_string();
        }

        // Window of fixed width `2h` that always contains the true tier.
        let full = 2 * h;
        let a = (self.seed % (full as u64 + 1)) as i32;
        let lo = (tier - a).max(0);
        let hi = (tier + (full - a)).min(n - 1);

        if lo == hi {
            POTENTIAL_TIERS[lo as usize].to_string()
        } else {
            format!(
                "{} to {}",
                POTENTIAL_TIERS[lo as usize], POTENTIAL_TIERS[hi as usize]
            )
        }
    }
}
