//! Integration test that sweeps one player through every training focus and
//! dumps per-day skill snapshots to JSON for the Python plotting script at
//! `tests/player_evolution/plot_evolution.py`.
//!
//! Run with:
//!     cargo test --test player_evolution
//! then:
//!     python3 tests/player_evolution/plot_evolution.py

use itertools::Itertools;
use rebels::app::App;
use rebels::core::constants::{AGE_INCREASE_PER_LONG_TICK, SKILL_DECREMENT_PER_LONG_TICK};
use rebels::core::{Skill, TrainingFocus};
use rebels::types::{AppResult, HashMapWithResult};
use serde::Serialize;

#[derive(Serialize)]
struct Snapshot {
    rel_age: f32,
    overall: f32,
    skills: Vec<f32>,
}

#[derive(Serialize)]
struct Series {
    focus: String,
    snapshots: Vec<Snapshot>,
    /// Relative-age points at which the active focus changed (only used by
    /// the Switching series). Empty for non-switching series.
    transitions: Vec<f32>,
}

#[derive(Serialize)]
struct Dump {
    player_name: String,
    population: String,
    potential: f32,
    best_position: u8,
    initial_skills: Vec<f32>,
    series: Vec<Series>,
}

const CATEGORY_RANGES: [(usize, usize); 5] = [(0, 4), (4, 8), (8, 12), (12, 16), (16, 20)];

fn category_avg(skills: &[f32], cat_idx: usize) -> f32 {
    let (lo, hi) = CATEGORY_RANGES[cat_idx];
    let mut s = 0.0;
    for v in &skills[lo..hi] {
        s += v;
    }
    s / (hi - lo) as f32
}

#[test]
fn dump_player_evolution() -> AppResult<()> {
    let mut app = App::test_default()?;
    let world = &mut app.world;

    // Highest-potential player makes the differences between focuses most visible.
    let pid = world
        .players
        .values()
        .sorted_by(|a, b| {
            a.info
                .relative_age()
                .partial_cmp(&b.info.relative_age())
                .unwrap()
        })
        .next()
        .expect("at least one player")
        .id;

    let base = world.players.get_or_err(&pid)?.clone();
    let best_pos = base
        .game_position_fitness
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(idx, _)| idx as u8)
        .unwrap_or(0);

    let mut focuses: Vec<(String, Option<TrainingFocus>)> = vec![
        ("None".to_string(), None),
        ("Athletics".to_string(), Some(TrainingFocus::Athletics)),
        ("Offense".to_string(), Some(TrainingFocus::Offense)),
        ("Defense".to_string(), Some(TrainingFocus::Defense)),
        ("Technical".to_string(), Some(TrainingFocus::Technical)),
        ("Mental".to_string(), Some(TrainingFocus::Mental)),
    ];
    for p in 0..5u8 {
        focuses.push((
            format!("Position({p})"),
            Some(TrainingFocus::GamePosition(p)),
        ));
    }

    let mut all_series = Vec::new();
    for (label, focus) in &focuses {
        let mut player = base.clone();
        player.potential = 20.0;
        player.skills_training = [0.0; 20];

        let mut snapshots = Vec::new();
        snapshots.push(Snapshot {
            rel_age: player.info.relative_age(),
            overall: player.average_skill(),
            skills: player.current_skill_array().to_vec(),
        });

        let mut iterations = 0usize;
        loop {
            let rel_age = player.info.relative_age();
            if rel_age >= 1.0 || iterations > 5000 {
                break;
            }

            // One game per day: 1920 seconds split 60/13.3/13.3/13.3/0 across
            // positions ranked by current game_position_fitness.
            let mut ranked: Vec<usize> = (0..5).collect();
            ranked.sort_by(|&a, &b| {
                player.game_position_fitness[b]
                    .partial_cmp(&player.game_position_fitness[a])
                    .unwrap()
            });
            let mut experience_at_position = [0u32; 5];
            experience_at_position[ranked[0]] = 1152;
            experience_at_position[ranked[1]] = 256;
            experience_at_position[ranked[2]] = 256;
            experience_at_position[ranked[3]] = 256;
            player.update_skills_training(experience_at_position, 1.5, *focus);

            // Inline the relevant body of World::tick_players_update.
            for idx in 0..player.skills_training.len() {
                let age_modifier = player.age_modifier_to_skill_update(idx);
                player.modify_skill(idx, SKILL_DECREMENT_PER_LONG_TICK * age_modifier);
                let training = player.skills_training[idx];
                player.modify_skill(idx, training);
            }
            player.skills_training = [Skill::default(); 20];
            player.info.age += AGE_INCREASE_PER_LONG_TICK;
            iterations += 1;

            if iterations % 2 == 0 {
                snapshots.push(Snapshot {
                    rel_age: player.info.relative_age(),
                    overall: player.average_skill(),
                    skills: player.current_skill_array().to_vec(),
                });
            }
        }

        all_series.push(Series {
            focus: label.clone(),
            snapshots,
            transitions: Vec::new(),
        });
    }

    // ---- Switching scenario --------------------------------------------------
    // Start with Athletics focus. When the current focus' category average
    // reaches CATEGORY_CAP_THRESHOLD, switch to the next category. Order:
    // Athletics -> Offense -> Defense -> Technical -> Mental, then stay.
    const CATEGORY_CAP_THRESHOLD: f32 = 19.0;
    let switching_chain = [
        TrainingFocus::Athletics,
        TrainingFocus::Offense,
        TrainingFocus::Defense,
        TrainingFocus::Technical,
        TrainingFocus::Mental,
    ];

    let mut player = base.clone();
    player.info.age = player.info.population.min_age();
    player.skills_training = [0.0; 20];

    let mut snapshots = Vec::new();
    let mut transitions = Vec::new();
    let mut chain_idx = 0usize;
    snapshots.push(Snapshot {
        rel_age: player.info.relative_age(),
        overall: player.average_skill(),
        skills: player.current_skill_array().to_vec(),
    });

    let mut iterations = 0usize;
    loop {
        let rel_age = player.info.relative_age();
        if rel_age >= 1.0 || iterations > 5000 {
            break;
        }

        let focus = Some(switching_chain[chain_idx]);

        let mut ranked: Vec<usize> = (0..5).collect();
        ranked.sort_by(|&a, &b| {
            player.game_position_fitness[b]
                .partial_cmp(&player.game_position_fitness[a])
                .unwrap()
        });
        let mut experience_at_position = [0u32; 5];
        experience_at_position[ranked[0]] = 1152;
        experience_at_position[ranked[1]] = 256;
        experience_at_position[ranked[2]] = 256;
        experience_at_position[ranked[3]] = 256;
        player.update_skills_training(experience_at_position, 1.5, focus);

        for idx in 0..player.skills_training.len() {
            let age_modifier = player.age_modifier_to_skill_update(idx);
            player.modify_skill(idx, SKILL_DECREMENT_PER_LONG_TICK * age_modifier);
            let training = player.skills_training[idx];
            player.modify_skill(idx, training);
        }
        player.skills_training = [0.0; 20];
        player.info.age += AGE_INCREASE_PER_LONG_TICK;
        iterations += 1;

        // Check whether the current focus category is full and we should switch.
        if chain_idx + 1 < switching_chain.len() {
            let skills = player.current_skill_array();
            if category_avg(&skills, chain_idx) >= CATEGORY_CAP_THRESHOLD {
                chain_idx += 1;
                transitions.push(player.info.relative_age());
                println!(
                    "Switching: rel_age={:.3} -> focus={:?}",
                    player.info.relative_age(),
                    switching_chain[chain_idx]
                );
            }
        }

        if iterations % 2 == 0 {
            snapshots.push(Snapshot {
                rel_age: player.info.relative_age(),
                overall: player.average_skill(),
                skills: player.current_skill_array().to_vec(),
            });
        }
    }

    all_series.push(Series {
        focus: "Switching".to_string(),
        snapshots,
        transitions,
    });

    let dump = Dump {
        player_name: base.info.full_name(),
        population: format!("{:?}", base.info.population),
        potential: player.potential,
        best_position: best_pos,
        initial_skills: base.current_skill_array().to_vec(),
        series: all_series,
    };

    std::fs::create_dir_all("tests/player_evolution")?;
    std::fs::write(
        "tests/player_evolution/evolution_data.json",
        serde_json::to_vec_pretty(&dump)?,
    )?;
    println!(
        "Wrote tests/player_evolution/evolution_data.json ({} focuses)",
        dump.series.len()
    );

    Ok(())
}
//cargo test --test player_evolution && python3 tests/player_evolution/plot_evolution.py
