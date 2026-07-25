use crate::{
    core::{Population, Region, Skill, MAX_SKILL, OPINION_NEUTRAL_VALUE},
    types::{TeamId, Tick},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::Display;

pub type PlayerOpinionMap = HashMap<PlayerOpinion, (Tick, Skill)>; // Opinion to last_event, value map.

const OPINION_DESCRIPTION_THRESHOLD: Skill = 3.0;
const STRONG_OPINION_DESCRIPTION_THRESHOLD: Skill = 7.0;

fn join_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

pub trait PlayerOpinionMapDescription {
    fn modifier(&self, opinion: PlayerOpinion) -> f32;
    fn team_mood(&self) -> &'static str;
    fn describe_opinions(&self) -> Vec<String>;
}

impl PlayerOpinionMapDescription for PlayerOpinionMap {
    fn modifier(&self, opinion: PlayerOpinion) -> f32 {
        if let Some((_, value)) = self.get(&opinion) {
            (value - OPINION_NEUTRAL_VALUE) / MAX_SKILL
        } else {
            0.0
        }
    }

    fn team_mood(&self) -> &'static str {
        let value = self
            .get(&PlayerOpinion::OwnTeam)
            .map(|(_, value)| *value)
            .unwrap_or(OPINION_NEUTRAL_VALUE);
        match value {
            x if x < 1.0 => "disgusted by the crew",
            x if x < 3.0 => "fed up with the crew",
            x if x < 5.0 => "strongly opposed to the crew",
            x if x < 7.0 => "unhappy about the crew",
            x if x < 9.0 => "lukewarm about the crew",
            x if x < 13.0 => "content with the crew",
            x if x < 15.0 => "pleased with the crew",
            x if x < 17.0 => "happy with the crew",
            x if x < 19.0 => "delighted with the crew",
            _ => "ecstatic about the crew",
        }
    }

    fn describe_opinions(&self) -> Vec<String> {
        const VERBS: [&str; 4] = [
            "really likes",
            "somewhat likes",
            "somewhat dislikes",
            "really dislikes",
        ];
        let verb_index = |value: Skill| -> Option<usize> {
            let deviation = value - OPINION_NEUTRAL_VALUE;
            if deviation >= STRONG_OPINION_DESCRIPTION_THRESHOLD {
                Some(0)
            } else if deviation >= OPINION_DESCRIPTION_THRESHOLD {
                Some(1)
            } else if deviation <= -STRONG_OPINION_DESCRIPTION_THRESHOLD {
                Some(3)
            } else if deviation <= -OPINION_DESCRIPTION_THRESHOLD {
                Some(2)
            } else {
                None
            }
        };

        let mut objects: [Vec<String>; 4] = [vec![], vec![], vec![], vec![]];
        let mut human_regions: [Vec<Region>; 4] = [vec![], vec![], vec![], vec![]];

        for (opinion, (_, value)) in self.iter() {
            let Some(i) = verb_index(*value) else {
                continue;
            };
            match opinion {
                PlayerOpinion::Drinking => objects[i].push("drinking".to_string()),
                PlayerOpinion::Games => objects[i].push("games".to_string()),
                PlayerOpinion::Gold => objects[i].push("gold".to_string()),
                PlayerOpinion::OwnTeam => {}
                PlayerOpinion::Populations { population } => match population {
                    Population::Human { region } => human_regions[i].push(*region),
                    other => objects[i].push(other.to_string()),
                },
                PlayerOpinion::Space => objects[i].push("space".to_string()),
                PlayerOpinion::Team { .. } => objects[i].push("a former crew".to_string()),
            }
        }

        let mut lines = vec![];
        for i in 0..VERBS.len() {
            if !human_regions[i].is_empty() {
                let mut regions: Vec<String> =
                    human_regions[i].iter().map(|r| r.to_string()).collect();
                regions.sort();
                objects[i].push(format!("Humans from {}", join_and(&regions)));
            }
            objects[i].sort();
            if !objects[i].is_empty() {
                lines.push(format!("{} {}", VERBS[i], join_and(&objects[i])));
            }
        }
        lines
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Display, PartialEq, Eq, Hash)]
pub enum PlayerOpinion {
    Drinking,
    Games,
    Gold,
    OwnTeam,
    Populations { population: Population },
    Space,
    Team { team_id: TeamId },
}

impl PlayerOpinion {
    pub fn satisfaction_modifier(&self) -> f32 {
        match self {
            Self::Drinking => 0.85,
            Self::Space => 1.25,
            Self::Gold => 1.5,
            _ => 1.0,
        }
    }
}
