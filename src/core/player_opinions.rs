use crate::{
    core::{Population, Skill, MAX_SKILL, OPINION_NEUTRAL_VALUE},
    types::{TeamId, Tick},
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::Display;

pub type PlayerOpinionMap = HashMap<PlayerOpinion, (Tick, Skill)>; // Opinion to last_event, value map.

const OPINION_DESCRIPTION_THRESHOLD: Skill = 3.0;
const STRONG_OPINION_DESCRIPTION_THRESHOLD: Skill = 7.0;

pub trait PlayerOpinionMapDescription {
    fn modifier(&self, opinion: PlayerOpinion) -> f32;
    fn description(&self) -> Vec<String>;
}

impl PlayerOpinionMapDescription for PlayerOpinionMap {
    fn modifier(&self, opinion: PlayerOpinion) -> f32 {
        if let Some((_, value)) = self.get(&opinion) {
            (value - OPINION_NEUTRAL_VALUE) / MAX_SKILL
        } else {
            0.0
        }
    }
    fn description(&self) -> Vec<String> {
        self.iter()
            .filter_map(|(opinion, (_, value))| {
                let deviation = value - OPINION_NEUTRAL_VALUE;
                let verb = if deviation >= STRONG_OPINION_DESCRIPTION_THRESHOLD {
                    "really likes"
                } else if deviation >= OPINION_DESCRIPTION_THRESHOLD {
                    "somewhat likes"
                } else if deviation <= -STRONG_OPINION_DESCRIPTION_THRESHOLD {
                    "really dislikes"
                } else if deviation <= -OPINION_DESCRIPTION_THRESHOLD {
                    "somewhat dislikes"
                } else {
                    return None;
                };
                let object = match opinion {
                    PlayerOpinion::Adventures => "space adventures".to_string(),
                    PlayerOpinion::Drinking => "drinking".to_string(),
                    PlayerOpinion::Games => "games".to_string(),
                    PlayerOpinion::Gold => "gold".to_string(),
                    PlayerOpinion::Populations { population } => population.to_string(),
                    PlayerOpinion::Team { name, .. } => name.clone(),
                };
                Some((*value, format!("{verb} {object}")))
            })
            .sorted_by(|(a, _), (b, _)| b.total_cmp(a))
            .map(|(_, text)| text)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Display, PartialEq, Eq, Hash)]
pub enum PlayerOpinion {
    Adventures,
    Drinking,
    Games,
    Gold,
    Populations { population: Population },
    Team { team_id: TeamId, name: String },
}
