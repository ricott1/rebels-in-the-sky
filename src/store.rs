#[cfg(feature = "relayer")]
use crate::network::types::{PlayerRanking, TeamRanking};
use crate::{
    core::world::World,
    game_engine::{game::Game, Tournament, TournamentId},
    types::*,
};
use anyhow::anyhow;
use directories;
use flate2::{
    read::{GzDecoder, ZlibDecoder},
    write::GzEncoder,
    Compression,
};
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};

#[cfg(feature = "relayer")]
use std::collections::HashMap;
use std::{
    io::{Read, Write},
    path::PathBuf,
};

pub static ASSETS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/");
static PERSISTED_WORLD_FILENAME: &str = "world";
static PERSISTED_GAMES_PREFIX: &str = "games/game_";
static PERSISTED_TOURNAMENTS_PREFIX: &str = "tournaments/tournament_";
static LEGACY_PERSISTED_GAMES_PREFIX: &str = "game_";
#[cfg(feature = "relayer")]
static PERSISTED_TEAM_RANKING_FILENAME: &str = "relayer/team_ranking";
#[cfg(feature = "relayer")]
static PERSISTED_PLAYER_RANKING_FILENAME: &str = "relayer/player_ranking";
const COMPRESSION_LEVEL: u32 = 5;

fn prefixed_world_filename(store_prefix: &str) -> String {
    format!("{store_prefix}_{PERSISTED_WORLD_FILENAME}")
}

fn save_to_json<T: Serialize>(filename: &str, data: &T) -> AppResult<()> {
    let path = store_path(&format!("{filename}.json.gz"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, serialize(data)?)?;

    Ok(())
}

fn load_from_json<T: for<'a> Deserialize<'a> + Serialize>(filename: &str) -> AppResult<T> {
    // New gzip format
    if let Ok(bytes) = std::fs::read(store_path(&format!("{filename}.json.gz"))?) {
        return deserialize(&bytes);
    }

    // This fallback serves to migrate old zlib compression to the new gz format
    let legacy_zlib = store_path(&format!("{filename}.json.compressed"))?;
    if let Ok(bytes) = std::fs::read(&legacy_zlib) {
        let data: T = deserialize(&bytes)?;
        save_to_json(filename, &data)?; // writes .json.gz
        if let Err(e) = std::fs::remove_file(&legacy_zlib) {
            log::warn!("Failed to delete legacy file {legacy_zlib:?}: {e}");
        }
        return Ok(data);
    }

    // This fallback serves to migrate old files to the new gz format
    let file = std::fs::File::open(store_path(&format!("{filename}.json"))?)?;
    Ok(serde_json::from_reader(file)?)
}

fn compress(bytes: &[u8], level: u32) -> AppResult<Vec<u8>> {
    let mut e = GzEncoder::new(Vec::new(), Compression::new(level));
    e.write_all(bytes)?;
    Ok(e.finish()?)
}

fn decompress(bytes: &[u8]) -> AppResult<Vec<u8>> {
    // gzip magic bytes: 1F 8B
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut d = GzDecoder::new(bytes);
        let mut buf = Vec::new();
        d.read_to_end(&mut buf)?;
        Ok(buf)
    } else {
        // FIXME: remove legacy zlib
        let mut d = ZlibDecoder::new(bytes);
        let mut buf = Vec::new();
        d.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

pub fn serialize<T: Serialize>(value: &T) -> AppResult<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    let compressed = compress(&bytes, COMPRESSION_LEVEL)?;
    Ok(compressed)
}

pub fn deserialize<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> AppResult<T> {
    let value = decompress(bytes)?;
    let data = serde_json::from_slice::<T>(&value)?;
    Ok(data)
}

fn config_dirs() -> AppResult<PathBuf> {
    // Linux:   /home/alice/.config/rebels
    // Windows: C:\Users\Alice\AppData\Roaming\frittura\rebels
    // macOS:   /Users/Alice/Library/Application Support/org.frittura.rebels
    let dirs = directories::ProjectDirs::from("org", "frittura", "rebels")
        .ok_or_else(|| anyhow!("Failed to get directories"))?;
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
        store_path(&format!("{filename}.json"))?,
        &serde_json::to_string_pretty(&world)?,
    )?;

    Ok(())
}

pub fn save_world(
    world: &World,
    store_prefix: &str,
    with_backup: bool,
    with_uncompressed: bool,
) -> AppResult<()> {
    let data = world.to_store()?;
    let filename = prefixed_world_filename(store_prefix);
    save_to_json(&filename, &data)?;
    if with_backup {
        let backup_filename = format!("{filename}.back");
        save_to_json(&backup_filename, &data)?;
    }

    if with_uncompressed {
        std::fs::write(
            store_path(&format!("{filename}.json"))?,
            &serde_json::to_string_pretty(&data)?,
        )?;
    }

    Ok(())
}

pub fn load_world(store_prefix: &str) -> AppResult<World> {
    load_from_json::<World>(&prefixed_world_filename(store_prefix))
}

pub fn save_game(game: &Game) -> AppResult<()> {
    save_to_json(&format!("{}{}", PERSISTED_GAMES_PREFIX, game.id), game)?;
    Ok(())
}

pub fn load_game(game_id: &GameId) -> AppResult<Game> {
    // FIXME: remove this code, currently needed for migrating to new folder
    if let Ok(game) = load_from_json::<Game>(&format!("{PERSISTED_GAMES_PREFIX}{game_id}")) {
        log::info!("Found game {game_id}");
        Ok(game)
    } else {
        let game = load_from_json::<Game>(&format!("{LEGACY_PERSISTED_GAMES_PREFIX}{game_id}"))?;
        log::info!("Found legacy game {game_id}");
        save_game(&game)?;
        Ok(game)
    }
}

pub fn save_tournament(tournament: &Tournament) -> AppResult<()> {
    save_to_json(
        &format!("{}{}", PERSISTED_TOURNAMENTS_PREFIX, tournament.id),
        tournament,
    )?;
    Ok(())
}

pub fn load_tournament(tournament_id: &TournamentId) -> AppResult<Tournament> {
    load_from_json::<Tournament>(&format!("{PERSISTED_TOURNAMENTS_PREFIX}{tournament_id}"))
}

#[cfg(feature = "relayer")]
pub fn load_relayer_messages() -> AppResult<Vec<String>> {
    // Load every message in the 'relayer/messages' directory.
    let config_dirs = config_dirs()?;
    let relayer_messages_directory = config_dirs.join("relayer/messages");
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

#[cfg(feature = "relayer")]
pub fn save_team_ranking(
    team_ranking: &HashMap<TeamId, TeamRanking>,
    with_backup: bool,
) -> AppResult<()> {
    save_to_json(PERSISTED_TEAM_RANKING_FILENAME, &team_ranking)?;
    if with_backup {
        let backup_filename = format!("{PERSISTED_TEAM_RANKING_FILENAME}.back");
        save_to_json(&backup_filename, &team_ranking)?;
    }
    Ok(())
}

#[cfg(feature = "relayer")]
pub fn load_team_ranking() -> AppResult<HashMap<TeamId, TeamRanking>> {
    load_from_json::<HashMap<TeamId, TeamRanking>>(PERSISTED_TEAM_RANKING_FILENAME)
}

#[cfg(feature = "relayer")]
pub fn save_player_ranking(
    player_ranking: &HashMap<PlayerId, PlayerRanking>,
    with_backup: bool,
) -> AppResult<()> {
    save_to_json(PERSISTED_PLAYER_RANKING_FILENAME, &player_ranking)?;
    if with_backup {
        let backup_filename = format!("{PERSISTED_PLAYER_RANKING_FILENAME}.back");
        save_to_json(&backup_filename, &player_ranking)?;
    }
    Ok(())
}

#[cfg(feature = "relayer")]
pub fn load_player_ranking() -> AppResult<HashMap<PlayerId, PlayerRanking>> {
    load_from_json::<HashMap<PlayerId, PlayerRanking>>(PERSISTED_PLAYER_RANKING_FILENAME)
}

pub fn get_world_size(store_prefix: &str) -> AppResult<u64> {
    let size = world_file_data(store_prefix)?.len();
    Ok(size)
}

pub fn reset_store() -> AppResult<()> {
    let dirs = directories::ProjectDirs::from("org", "frittura", "rebels")
        .ok_or_else(|| anyhow!("Failed to get directories"))?;
    let config_dirs = dirs.config_dir();
    if config_dirs.exists() {
        std::fs::remove_dir_all(config_dirs)?;
    }
    std::fs::create_dir_all(config_dirs)?;
    Ok(())
}

pub fn save_game_exists(store_prefix: &str) -> bool {
    let filename = prefixed_world_filename(store_prefix);

    [
        format!("{filename}.json.gz"),
        format!("{filename}.json.compressed"),
        format!("{filename}.json"),
    ]
    .iter()
    .any(|f| store_path(f).map(|p| p.exists()).unwrap_or(false))
}

pub fn save_data<C: AsRef<[u8]>>(filename: &str, data: &C) -> AppResult<()> {
    std::fs::write(store_path(filename)?, data)?;
    Ok(())
}

pub fn load_data(filename: &str) -> AppResult<Vec<u8>> {
    let bytes = std::fs::read(store_path(filename)?)?;
    Ok(bytes)
}

pub fn world_file_data(store_prefix: &str) -> AppResult<std::fs::Metadata> {
    let filename = prefixed_world_filename(store_prefix);

    let candidates = [
        format!("{filename}.json.gz"),
        format!("{filename}.json.compressed"),
        format!("{filename}.json"),
    ];

    for name in candidates {
        if let Ok(path) = store_path(&name) {
            if let Ok(metadata) = std::fs::metadata(&path) {
                return Ok(metadata);
            }
        }
    }

    Err(anyhow!("No world file found for prefix '{store_prefix}'"))
}

#[cfg(test)]
mod tests {
    use crate::{
        core::{player::Player, team::Team, world::World, MIN_PLAYERS_PER_GAME},
        types::{AppResult, PlayerMap},
    };
    use directories;
    use itertools::Itertools;
    use libp2p::PeerId;
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
        super::save_world(&world, store_prefix, false, false)?;
        let _ = super::load_world(store_prefix)?;
        Ok(())
    }

    #[test]
    fn test_serialize_network_data() -> AppResult<()> {
        use super::{deserialize, serialize};
        use crate::network::types::{NetworkData, NetworkTeam};
        let value = NetworkData::Message {
            timestamp: 0,
            from_peer_id: PeerId::random(),
            author: "Test".to_string(),
            message: "Hello".to_string(),
        };
        let serialized_data = serialize(&value)?;
        let deserialized_data = deserialize(&serialized_data)?;
        assert!(value == deserialized_data);

        let mut team = Team::random(None);

        let mut players = PlayerMap::new();
        for _ in 0..MIN_PLAYERS_PER_GAME {
            let player = Player::default().randomize(None);
            team.player_ids.push(player.id);
            players.insert(player.id, player);
        }

        let value = NetworkData::Team {
            timestamp: 0,
            team: NetworkTeam::new(team, players, vec![]),
        };
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
        let relayer_messages_directory = config_dirs.join("relayer/messages");
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
