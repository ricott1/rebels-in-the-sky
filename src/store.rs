use crate::{
    game_engine::game::Game,
    network::types::TeamRanking,
    types::{AppResult, GameId, TeamId},
    world::world::World,
};
use anyhow::anyhow;
use directories;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::PathBuf,
};

pub static ASSETS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/");
static PERSISTED_WORLD_FILENAME: &str = "world";
static PERSISTED_GAMES_PREFIX: &str = "game_";
static PERSISTED_TEAM_RANKING_FILENAME: &str = "team_ranking";
const COMPRESSION_LEVEL: u32 = 3;

fn prefixed_world_filename(store_prefix: &str) -> String {
    format!("{}_{}", store_prefix, PERSISTED_WORLD_FILENAME)
}

fn save_to_json<T: Serialize>(filename: &str, data: &T) -> AppResult<()> {
    std::fs::write(
        store_path(&format!("{}.json.compressed", filename))?,
        &serialize(data)?,
    )?;
    Ok(())
}

fn load_from_json<T: for<'a> Deserialize<'a>>(filename: &str) -> AppResult<T> {
    let data: T =
        if let Ok(bytes) = std::fs::read(store_path(&format!("{}.json.compressed", filename))?) {
            log::info!("Read {} bytes", bytes.len());
            deserialize(&bytes)?
        } else {
            // This fallback serves to migrate old files to the new compressed format
            let file = std::fs::File::open(store_path(&format!("{}.json", filename))?)?;
            serde_json::from_reader(file)?
        };

    Ok(data)
}

fn compress(bytes: &Vec<u8>, level: u32) -> AppResult<Vec<u8>> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::new(level));
    e.write_all(bytes)?;
    let compressed_bytes = e.finish()?;
    Ok(compressed_bytes)
}

fn decompress(bytes: &[u8]) -> AppResult<Vec<u8>> {
    let mut d = ZlibDecoder::new(&bytes[..]);
    let mut buf = Vec::new();
    d.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn serialize<T: Serialize>(value: &T) -> AppResult<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    let compressed = compress(&bytes, COMPRESSION_LEVEL)?;
    Ok(compressed)
}

pub fn deserialize<T: for<'a> Deserialize<'a>>(bytes: &Vec<u8>) -> AppResult<T> {
    let value = decompress(&bytes)?;
    let data = serde_json::from_slice::<T>(&value)?;
    Ok(data)
}

pub fn store_path(filename: &str) -> AppResult<PathBuf> {
    let dirs = directories::ProjectDirs::from("org", "frittura", "rebels")
        .ok_or(anyhow!("Failed to get directories"))?;
    let config_dirs = dirs.config_dir();
    if !config_dirs.exists() {
        std::fs::create_dir_all(config_dirs)?;
    }
    let path = config_dirs.join(filename);
    Ok(path)
}

pub fn save_world(world: &World, with_backup: bool, store_prefix: &str) -> AppResult<()> {
    let data = world.to_store()?;
    let filename = prefixed_world_filename(store_prefix);
    save_to_json(&filename, &data)?;
    if with_backup {
        let backup_filename = format!("{}.back", filename);
        save_to_json(&backup_filename, &data)?;
    }
    Ok(())
}

pub fn load_world(store_prefix: &str) -> AppResult<World> {
    load_from_json::<World>(&prefixed_world_filename(store_prefix))
}

pub fn save_game(game: &Game) -> AppResult<()> {
    save_to_json(&format!("{}{}", PERSISTED_GAMES_PREFIX, game.id), &game)?;
    Ok(())
}

pub fn load_game(game_id: GameId) -> AppResult<Game> {
    load_from_json::<Game>(&format!("{}{}", PERSISTED_GAMES_PREFIX, game_id))
}

pub fn save_team_ranking(
    team_ranking: &HashMap<TeamId, TeamRanking>,
    with_backup: bool,
) -> AppResult<()> {
    save_to_json(PERSISTED_TEAM_RANKING_FILENAME, &team_ranking)?;
    if with_backup {
        let backup_filename = format!("{}.back", PERSISTED_TEAM_RANKING_FILENAME);
        save_to_json(&backup_filename, &team_ranking)?;
    }
    Ok(())
}

pub fn load_team_ranking() -> AppResult<HashMap<TeamId, TeamRanking>> {
    load_from_json::<HashMap<TeamId, TeamRanking>>(PERSISTED_TEAM_RANKING_FILENAME)
}

pub fn get_world_size(store_prefix: &str) -> AppResult<u64> {
    let size = world_file_data(store_prefix)?.len();
    Ok(size)
}

pub fn reset() -> AppResult<()> {
    let dirs = directories::ProjectDirs::from("org", "frittura", "rebels")
        .ok_or(anyhow!("Failed to get directories"))?;
    let config_dirs = dirs.config_dir();
    if config_dirs.exists() {
        std::fs::remove_dir_all(config_dirs)?;
    }
    std::fs::create_dir_all(config_dirs)?;
    Ok(())
}

pub fn world_exists(store_prefix: &str) -> bool {
    let filename = prefixed_world_filename(store_prefix);
    let path = store_path(&format!("{}.json", filename));
    path.is_ok() && path.unwrap().exists()
}

pub fn world_file_data(store_prefix: &str) -> AppResult<std::fs::Metadata> {
    let filename = prefixed_world_filename(store_prefix);

    if let Ok(compressed_metadata) =
        std::fs::metadata(store_path(&format!("{}.json.compressed", filename))?)
    {
        Ok(compressed_metadata)
    } else {
        let metadata = std::fs::metadata(store_path(&format!("{}.json", filename))?)?;
        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::{deserialize, serialize};
    use crate::{
        network::types::{NetworkData, NetworkTeam},
        types::{AppResult, PlanetId, PlayerId, TeamId},
        world::{planet::Planet, player::Player, team::Team, world::World},
    };
    use directories;
    use itertools::Itertools;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use std::fs::File;

    #[test]
    fn test_path() {
        let dirs = directories::ProjectDirs::from("org", "frittura", "puma");
        assert!(dirs.is_some());
        let dirs_ok = dirs.unwrap();
        let config_dirs = dirs_ok.config_dir();
        println!("{:?}", config_dirs);
        if !config_dirs.exists() {
            std::fs::create_dir_all(config_dirs).unwrap();
        }
        let path = config_dirs.join("test");
        let file = File::create(path.clone());
        assert!(file.is_ok());
        assert!(path.is_file());
        if config_dirs.exists() {
            std::fs::remove_dir_all(config_dirs).unwrap();
        }
    }

    #[test]
    fn test_store_world() -> AppResult<()> {
        let store_prefix = "test";
        let mut world = World::new(None);
        world.initialize(true)?;
        world.own_team_id = world.teams.keys().collect_vec()[0].clone();
        super::save_world(&world, false, store_prefix)?;
        let _ = super::load_world(store_prefix)?;
        Ok(())
    }

    #[test]
    fn test_serialize_network_data() -> AppResult<()> {
        let value = NetworkData::Message(0, "Hello".to_string());
        let serialized_data = serialize(&value)?;
        let deserialized_data = deserialize(&serialized_data)?;
        assert!(value == deserialized_data);

        let mut team = Team::random(
            TeamId::new_v4(),
            PlanetId::new_v4(),
            "name".to_string(),
            "ship_name".to_string(),
        );

        let mut players = vec![];
        let rng = &mut ChaCha8Rng::from_entropy();
        for _ in 0..5 {
            let player = Player::random(rng, PlayerId::new_v4(), None, &Planet::default(), 0.0);
            team.player_ids.push(player.id);
            players.push(player);
        }

        let value = NetworkData::Team(0, NetworkTeam::new(team, players, vec![]));
        let serialized_data = serialize(&value)?;
        println!("Team size: {}", serialized_data.len());

        let deserialized_data = deserialize(&serialized_data)?;
        assert!(value == deserialized_data);
        Ok(())
    }
}
