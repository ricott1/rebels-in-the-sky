use crate::network::types::{PlayerRanking, TeamRanking};
use crate::{
    game_engine::game::Game,
    types::{AppResult, GameId, PlayerId, TeamId},
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
static PERSISTED_PLAYER_RANKING_FILENAME: &str = "player_ranking";
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

fn config_dirs() -> AppResult<PathBuf> {
    // Linux:   /home/alice/.config/rebels
    // Windows: C:\Users\Alice\AppData\Roaming\frittura\rebels
    // macOS:   /Users/Alice/Library/Application Support/org.frittura.rebels
    let dirs = directories::ProjectDirs::from("org", "frittura", "rebels")
        .ok_or(anyhow!("Failed to get directories"))?;
    let config_dirs = dirs.config_dir().to_path_buf();
    if !config_dirs.exists() {
        std::fs::create_dir_all(&config_dirs)?;
    }
    Ok(config_dirs)
}

pub fn store_path(filename: &str) -> AppResult<PathBuf> {
    let config_dirs = config_dirs()?;
    let path = config_dirs.join(filename);
    Ok(path)
}

pub fn save_world_uncompressed(world: &World, store_prefix: &str) -> AppResult<()> {
    let filename = prefixed_world_filename(store_prefix);
    std::fs::write(
        store_path(&format!("{}.json", filename))?,
        &serde_json::to_string_pretty(&world)?,
    )?;

    Ok(())
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

#[cfg(feature = "relayer")]
pub fn load_relayer_messages() -> AppResult<Vec<String>> {
    // Load every message in the 'relayer_messages' directory.
    let config_dirs = config_dirs()?;
    let relayer_messages_directory = config_dirs.join("relayer_messages");
    if !relayer_messages_directory.exists() {
        std::fs::create_dir_all(&relayer_messages_directory)?;
        return Ok(vec![]);
    }

    let mut messages = vec![];
    for entry in std::fs::read_dir(&relayer_messages_directory)? {
        let entry = entry?;
        let path = entry.path();
        messages.push(std::fs::read_to_string(&path)?);

        // Remove the file after reading it.
        std::fs::remove_file(path)?;
    }

    Ok(messages)
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

pub fn save_player_ranking(
    player_ranking: &HashMap<PlayerId, PlayerRanking>,
    with_backup: bool,
) -> AppResult<()> {
    save_to_json(PERSISTED_PLAYER_RANKING_FILENAME, &player_ranking)?;
    if with_backup {
        let backup_filename = format!("{}.back", PERSISTED_PLAYER_RANKING_FILENAME);
        save_to_json(&backup_filename, &player_ranking)?;
    }
    Ok(())
}

pub fn load_player_ranking() -> AppResult<HashMap<PlayerId, PlayerRanking>> {
    load_from_json::<HashMap<PlayerId, PlayerRanking>>(PERSISTED_PLAYER_RANKING_FILENAME)
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

pub fn save_game_exists(store_prefix: &str) -> bool {
    let filename = prefixed_world_filename(store_prefix);
    if let Ok(path) = store_path(&format!("{}.json.compressed", filename)) {
        if path.exists() {
            return true;
        }
    }

    if let Ok(path) = store_path(&format!("{}.json", filename)) {
        if path.exists() {
            return true;
        }
    }

    false
}

pub fn save_data<C: AsRef<[u8]>>(filename: &str, data: &C) -> AppResult<()> {
    std::fs::write(store_path(&filename)?, data)?;
    Ok(())
}

pub fn load_data(filename: &str) -> AppResult<Vec<u8>> {
    let bytes = std::fs::read(store_path(&filename)?)?;
    Ok(bytes)
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
    use crate::{
        types::AppResult,
        world::{player::Player, team::Team, types::Population, world::World},
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
        use super::{deserialize, serialize};
        use crate::network::types::{NetworkData, NetworkTeam};
        use crate::types::{PlanetId, PlayerId, TeamId};
        let value = NetworkData::Message(0, "Hello".to_string());
        let serialized_data = serialize(&value)?;
        let deserialized_data = deserialize(&serialized_data)?;
        assert!(value == deserialized_data);

        let rng = &mut ChaCha8Rng::from_os_rng();

        let mut team = Team::random(
            TeamId::new_v4(),
            PlanetId::new_v4(),
            "name".to_string(),
            "ship_name".to_string(),
            rng,
        );

        let mut players = vec![];
        let rng = &mut ChaCha8Rng::from_os_rng();
        for _ in 0..5 {
            let population = Population::default();
            let player = Player::random(
                rng,
                PlayerId::new_v4(),
                None,
                population,
                &PlanetId::default(),
                0.0,
            );
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

    #[cfg(feature = "relayer")]
    #[test]
    #[ignore]
    fn test_load_relayer_message() -> AppResult<()> {
        let message = "Hello, world!";
        let config_dirs = super::config_dirs()?;
        let relayer_messages_directory = config_dirs.join("relayer_messages");
        if !relayer_messages_directory.exists() {
            std::fs::create_dir_all(&relayer_messages_directory)?;
        }
        let path = relayer_messages_directory.join("test");
        std::fs::write(&path, message)?;
        let loaded_messages = super::load_relayer_messages()?;
        assert_eq!(loaded_messages, vec![message]);
        Ok(())
    }
}
