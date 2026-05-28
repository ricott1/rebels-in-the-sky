//! Integration test that generates a large pool of random players and dumps
//! per-player potential / overall skill / age / population to JSON for the
//! Python plotting script at
//! `tests/player_potential_distribution/plot_potentials.py`.
//!
//! Run with:
//!     cargo test --test player_potential_distribution -- --nocapture
//! then:
//!     python3 tests/player_potential_distribution/plot_potentials.py

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rebels::core::Player;
use rebels::types::AppResult;
use serde::Serialize;

const NUM_PLAYERS: usize = 100_000;
const SEED: u64 = 0xC0FFEE;

#[derive(Serialize)]
struct PlayerSample {
    population: String,
    relative_age: f32,
    overall: f32,
    potential: f32,
}

#[derive(Serialize)]
struct Dump {
    num_players: usize,
    seed: u64,
    samples: Vec<PlayerSample>,
}

#[test]
fn dump_player_potential_distribution() -> AppResult<()> {
    let mut rng = ChaCha8Rng::seed_from_u64(SEED);

    let mut samples = Vec::with_capacity(NUM_PLAYERS);
    for _ in 0..NUM_PLAYERS {
        let player = Player::default().randomize(Some(&mut rng));
        samples.push(PlayerSample {
            population: format!("{}", player.info.population),
            relative_age: player.info.relative_age(),
            overall: player.average_skill(),
            potential: player.potential,
        });
    }

    let dump = Dump {
        num_players: NUM_PLAYERS,
        seed: SEED,
        samples,
    };

    std::fs::create_dir_all("tests/player_potential_distribution")?;
    std::fs::write(
        "tests/player_potential_distribution/potential_data.json",
        serde_json::to_vec_pretty(&dump)?,
    )?;
    println!(
        "Wrote tests/player_potential_distribution/potential_data.json ({} samples)",
        dump.samples.len()
    );

    Ok(())
}
//cargo test --test player_potential_distribution -- --nocapture && python3 tests/player_potential_distribution/plot_potentials.py
