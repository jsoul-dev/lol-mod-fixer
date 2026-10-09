//! League of Legends champions and skin database.
//!
//! Provides champion lookup by skin ID (or folder ID), WAD filename,
//! or fuzzy champion name matching, backed by bundled `skin_ids.json`.

use std::collections::HashMap;
use std::sync::OnceLock;

const SKIN_IDS_JSON: &str = include_str!("data/skin_ids.json");

#[derive(Debug, Clone)]
pub struct ChampionDb {
    /// Maps skin ID / champion ID (e.g. 106000, 106) -> Champion Name (e.g. "Volibear")
    id_to_name: HashMap<u32, &'static str>,
    /// Maps exact skin ID (e.g. 106001, 106000) -> Skin Name (e.g. "Thunder Lord Volibear")
    skin_id_to_name: HashMap<u32, &'static str>,
    /// Maps normalized lowercase name / alias -> Canonical Champion Name
    name_to_champ: HashMap<String, &'static str>,
}

static DB: OnceLock<ChampionDb> = OnceLock::new();

impl ChampionDb {
    fn load() -> Self {
        let parsed: HashMap<String, String> =
            serde_json::from_str(SKIN_IDS_JSON).unwrap_or_default();

        let mut id_to_name = HashMap::new();
        let mut skin_id_to_name = HashMap::new();
        let mut name_to_champ = HashMap::new();

        // Populate all skins and extract base champions (skin_id % 1000 == 0)
        for (id_str, skin_name) in &parsed {
            if let Ok(id) = id_str.parse::<u32>() {
                let static_name: &'static str = Box::leak(skin_name.clone().into_boxed_str());
                skin_id_to_name.insert(id, static_name);

                if id % 1000 == 0 && id > 0 {
                    let champ_id = id / 1000;
                    id_to_name.insert(id, static_name);
                    id_to_name.insert(champ_id, static_name);

                    // Add normalized lowercase name
                    let norm = normalize_champ_key(static_name);
                    name_to_champ.insert(norm, static_name);
                }
            }
        }


        // Add common League aliases and internal champion names
        let aliases = [
            ("monkeyking", "Wukong"),
            ("wukong", "Wukong"),
            ("drmundo", "Dr. Mundo"),
            ("mundo", "Dr. Mundo"),
            ("jarvan", "Jarvan IV"),
            ("jarvan4", "Jarvan IV"),
            ("jarvaniv", "Jarvan IV"),
            ("nunu", "Nunu & Willump"),
            ("willump", "Nunu & Willump"),
            ("tahm", "Tahm Kench"),
            ("tahmkench", "Tahm Kench"),
            ("missfortune", "Miss Fortune"),
            ("masteryi", "Master Yi"),
            ("yi", "Master Yi"),
            ("twistedfate", "Twisted Fate"),
            ("tf", "Twisted Fate"),
            ("xinzhao", "Xin Zhao"),
            ("xin", "Xin Zhao"),
            ("aurelionsol", "Aurelion Sol"),
            ("asol", "Aurelion Sol"),
            ("reksai", "Rek'Sai"),
            ("khazix", "Kha'Zix"),
            ("chogath", "Cho'Gath"),
            ("kogmaw", "Kog'Maw"),
            ("velkoz", "Vel'Koz"),
            ("kaisa", "Kai'Sa"),
            ("belveth", "Bel'Veth"),
            ("ksante", "K'Sante"),
            ("renata", "Renata Glasc"),
            ("renataglasc", "Renata Glasc"),
            ("blitz", "Blitzcrank"),
            ("cait", "Caitlyn"),
            ("cass", "Cassiopeia"),
            ("fiddle", "Fiddlesticks"),
            ("gp", "Gangplank"),
            ("hec", "Hecarim"),
            ("heimer", "Heimerdinger"),
            ("kass", "Kassadin"),
            ("kata", "Katarina"),
            ("lb", "LeBlanc"),
            ("leblanc", "LeBlanc"),
            ("malph", "Malphite"),
            ("morg", "Morgana"),
            ("naut", "Nautilus"),
            ("noc", "Nocturne"),
            ("ori", "Orianna"),
            ("panth", "Pantheon"),
            ("sej", "Sejuani"),
            ("soraka", "Soraka"),
            ("trist", "Tristana"),
            ("trynd", "Tryndamere"),
            ("vlad", "Vladimir"),
            ("warwick", "Warwick"),
            ("ww", "Warwick"),
            ("aphe", "Aphelios"),
            ("leesin", "Lee Sin"),
            ("eve", "Evelynn"),
            ("renek", "Renekton"),
            ("ez", "Ezreal"),
            ("morde", "Mordekaiser"),
            ("mord", "Mordekaiser"),
            ("kha", "Kha'Zix"),
            ("cho", "Cho'Gath"),
            ("vel", "Vel'Koz"),
            ("rek", "Rek'Sai"),
            ("sai", "Rek'Sai"),
            ("kai", "Kai'Sa"),
            ("sa", "Kai'Sa"),
            ("kog", "Kog'Maw"),
            ("maw", "Kog'Maw"),
            ("sol", "Aurelion Sol"),
            ("zhao", "Xin Zhao"),
            ("fate", "Twisted Fate"),
            ("fortune", "Miss Fortune"),
        ];

        for (alias, canonical) in aliases {
            name_to_champ.insert(alias.to_string(), canonical);
        }

        Self {
            id_to_name,
            skin_id_to_name,
            name_to_champ,
        }
    }
}

pub fn get_db() -> &'static ChampionDb {
    DB.get_or_init(ChampionDb::load)
}

fn normalize_champ_key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Lookup champion name by skin ID (e.g. 106000) or champion ID (e.g. 106).
pub fn champion_by_id(id: u32) -> Option<&'static str> {
    let db = get_db();
    // Try exact id, base id (id / 1000 * 1000), or champ id (id / 1000)
    db.id_to_name
        .get(&id)
        .copied()
        .or_else(|| {
            let base = (id / 1000) * 1000;
            db.id_to_name.get(&base).copied()
        })
        .or_else(|| {
            let champ = id / 1000;
            db.id_to_name.get(&champ).copied()
        })
}

/// Lookup exact skin name by skin ID (e.g. 106001 -> "Thunder Lord Volibear", 106000 -> "Volibear").
pub fn skin_by_id(id: u32) -> Option<&'static str> {
    let db = get_db();
    db.skin_id_to_name.get(&id).copied()
}

/// Resolve champion name, skin name, and whether it's a base skin from a numeric folder/skin ID.
pub fn resolve_folder_skin_info(id: u32) -> (Option<&'static str>, Option<&'static str>, bool) {
    let champ = champion_by_id(id);
    let is_base = id % 1000 == 0;
    let skin = skin_by_id(id).or_else(|| {
        let base = (id / 1000) * 1000;
        if base > 0 {
            skin_by_id(base)
        } else {
            skin_by_id(id * 1000)
        }
    });
    (champ, skin, is_base)
}

/// Lookup champion name by raw name string (case/punctuation-insensitive).
pub fn champion_by_name(name: &str) -> Option<&'static str> {
    let db = get_db();
    let norm = normalize_champ_key(name);
    db.name_to_champ.get(&norm).copied()
}

/// Get all normalized alias tokens for a given canonical champion name.
pub fn champion_alias_tokens(canonical_name: &str) -> Vec<String> {
    let db = get_db();
    let norm_canonical = normalize_champ_key(canonical_name);
    let mut tokens = Vec::new();
    tokens.push(norm_canonical);
    for part in canonical_name.split(|c: char| !c.is_alphanumeric()) {
        if !part.is_empty() {
            tokens.push(part.to_ascii_lowercase());
        }
    }
    for (alias, &champ) in &db.name_to_champ {
        if champ.eq_ignore_ascii_case(canonical_name) {
            tokens.push(alias.clone());
        }
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Detect champion name from all available context.
///
/// Order of precedence:
/// 1. Numeric parent directory (Skin ID, e.g. "106000" -> "Volibear").
/// 2. Contained WAD filenames (e.g. "Garen.wad.client" -> "Garen", "rammus.en_us.wad.client" -> "Rammus").
/// 3. Matching champion name in folder name or META/info.json name.
pub fn detect_champion(
    parent_dir: Option<&str>,
    folder_name: &str,
    info_name: Option<&str>,
    wad_files: &[&str],
) -> Option<&'static str> {
    // 1. Parent skin ID check
    if let Some(parent) = parent_dir {
        if let Ok(id) = parent.trim().parse::<u32>() {
            if let Some(champ) = champion_by_id(id) {
                return Some(champ);
            }
        }
    }

    // 2. WAD filename check
    for wad in wad_files {
        // e.g. "Garen.wad.client", "rammus.en_us.wad.client", "base:rammus.en_us.wad.client"
        let file_part = wad.rsplit('/').next().unwrap_or(wad);
        let file_part = file_part.rsplit('\\').next().unwrap_or(file_part);
        let file_part = file_part.strip_prefix("base:").unwrap_or(file_part);
        let stem = file_part.split('.').next().unwrap_or(file_part);
        if let Some(champ) = champion_by_name(stem) {
            return Some(champ);
        }
    }

    // 3. Word tokens inside folder_name
    let tokens: Vec<&str> = folder_name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    for token in &tokens {
        if let Some(champ) = champion_by_name(token) {
            return Some(champ);
        }
    }

    // 4. Word tokens inside info_name
    if let Some(name) = info_name {
        let info_tokens: Vec<&str> = name
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .collect();
        for token in &info_tokens {
            if let Some(champ) = champion_by_name(token) {
                return Some(champ);
            }
        }
    }

    None
}
