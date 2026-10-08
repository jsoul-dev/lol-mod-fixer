//! Persistent user configuration and League installation discovery.

use directories::ProjectDirs;
use fs_err as fs;
use ltk_manager_base::config::Config as LtkConfig;
use ltk_manager_base::utils::game::GameDir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{FixerError, FixerResult};

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "LeagueToolkit";
const APPLICATION: &str = "LoLModFixer";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Path to League of Legends installation (either root `.../League of Legends` or `.../League of Legends/Game`).
    pub league_path: Option<PathBuf>,
    /// Whether to prompt "Press Enter to exit..." when run interactively.
    pub pause_on_exit: Option<bool>,
}

impl AppConfig {
    /// Returns the configuration file path in the OS user app data directory.
    pub fn file_path() -> Option<PathBuf> {
        let proj_dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)?;
        Some(proj_dirs.config_dir().join(CONFIG_FILE))
    }

    /// Load configuration from disk, or return default if missing or unreadable.
    pub fn load() -> Self {
        let Some(path) = Self::file_path() else {
            return Self::default();
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save configuration to disk.
    pub fn save(&self) -> FixerResult<()> {
        let path = Self::file_path().ok_or_else(|| {
            FixerError::Config("Could not determine user app data directory".to_string())
        })?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }
}

/// Auto-detect League of Legends installation path.
pub fn auto_detect_league_path() -> Option<PathBuf> {
    // 1. Check RiotClientInstalls.json
    let installs_json = Path::new(r"C:\ProgramData\Riot Games\RiotClientInstalls.json");
    if let Ok(content) = fs::read_to_string(installs_json) {
        #[derive(Deserialize)]
        struct Installs {
            #[serde(default)]
            associated_client: HashMap<String, String>,
        }
        if let Ok(parsed) = serde_json::from_str::<Installs>(&content) {
            for key in parsed.associated_client.keys() {
                let candidate = PathBuf::from(key.trim_end_matches(['/', '\\']));
                if is_valid_league_dir(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    // 2. Common default paths
    let standard_paths = [
        r"C:\Riot Games\League of Legends",
        r"D:\Riot Games\League of Legends",
        r"E:\Riot Games\League of Legends",
        r"C:\Program Files\Riot Games\League of Legends",
        r"C:\Program Files (x86)\Riot Games\League of Legends",
    ];

    for path_str in standard_paths {
        let p = PathBuf::from(path_str);
        if is_valid_league_dir(&p) {
            return Some(p);
        }
    }

    None
}

/// Check if a directory is a valid League install root or Game directory.
pub fn is_valid_league_dir(path: &Path) -> bool {
    let dummy_cfg = LtkConfig {
        league_path: Some(path.to_path_buf()),
        ..LtkConfig::default()
    };
    GameDir::resolve(&dummy_cfg).is_ok()
}

/// Build LTK Config struct with league path and optional mod storage path.
pub fn resolve_ltk_config(
    cli_league: Option<PathBuf>,
    storage_dir: Option<PathBuf>,
) -> (LtkConfig, Option<PathBuf>) {
    let config = AppConfig::load();

    // Priority:
    // 1. CLI flag --league
    // 2. Environment variable LOL_MOD_FIXER_LEAGUE or LEAGUE_PATH
    // 3. Saved config in appdata
    // 4. Auto-detected path from Riot client or standard folders
    let league_path = cli_league
        .or_else(|| std::env::var_os("LOL_MOD_FIXER_LEAGUE").map(PathBuf::from))
        .or_else(|| std::env::var_os("LEAGUE_PATH").map(PathBuf::from))
        .or(config.league_path)
        .or_else(auto_detect_league_path);

    let ltk_cfg = LtkConfig {
        league_path: league_path.clone(),
        mod_storage_path: storage_dir,
        ..LtkConfig::default()
    };

    (ltk_cfg, league_path)
}
