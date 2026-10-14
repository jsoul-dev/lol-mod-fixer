//! Rose mod manager integration.
//!
//! Handles synchronization of `rose_mod_targets.json` and legacy `rose_wad_targets.json`
//! when extracted mods are repaired in Rose's mod directories.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::{FixerError, FixerResult};

const TARGET_METADATA: &str = "rose_mod_targets.json";
const LEGACY_TARGET_METADATA: &str = "rose_wad_targets.json";
pub const ROSE_CATEGORY_MODS: &str = "rose_category_mods.json";

/// Known standard non-skin Rose categories under `Rose/mods`
pub const KNOWN_ROSE_CATEGORIES: &[&str] = &[
    "announcers",
    "fonts",
    "loading_screen",
    "maps",
    "others",
    "sfx",
    "ui",
    "vfx",
    "voiceover",
    "hud",
    "audio",
];

/// Compute Rose's exact rename-stable folder hash and hashes for contained WADs.
///
/// Implements `ModStorageService._hash_mod_folder` from `injection/mods/storage.py`:
/// - Collects all files recursively inside `mod_dir`.
/// - Sorts them by their lowercased relative POSIX path.
/// - For each file:
///   - relative_path: forward-slash normalized relative path.
///   - file_hash: SHA-256 of file bytes.
///   - folder_hash.update(relative_path.as_bytes())
///   - folder_hash.update(b"\0")
///   - folder_hash.update(file_digest) (raw 32-byte digest)
///   - folder_hash.update(b"\0")
///   - If file is `.wad` or `.wad.client`:
///     - wad_hashes[relative_path] = hex(file_hash)
pub fn compute_rose_hashes(mod_dir: &Path) -> FixerResult<(String, BTreeMap<String, String>)> {
    let mut files = Vec::new();
    for entry in WalkDir::new(mod_dir) {
        let entry = entry.map_err(|e| FixerError::Io(std::io::Error::other(e)))?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }

    // Sort files by relative posix path lowercased
    files.sort_by(|a, b| {
        let rel_a = a
            .strip_prefix(mod_dir)
            .unwrap_or(a)
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        let rel_b = b
            .strip_prefix(mod_dir)
            .unwrap_or(b)
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        rel_a.cmp(&rel_b)
    });

    let mut folder_hasher = Sha256::new();
    let mut wad_hashes = BTreeMap::new();

    for file_path in files {
        let rel_path = file_path
            .strip_prefix(mod_dir)
            .map_err(|e| FixerError::Archive(format!("Failed to strip prefix: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");

        let mut file_hasher = Sha256::new();
        let mut file = fs_err::File::open(&file_path)?;
        std::io::copy(&mut file, &mut file_hasher)?;
        let file_digest = file_hasher.finalize();

        folder_hasher.update(rel_path.as_bytes());
        folder_hasher.update(b"\0");
        folder_hasher.update(file_digest);
        folder_hasher.update(b"\0");

        let lower = rel_path.to_ascii_lowercase();
        if lower.ends_with(".wad") || lower.ends_with(".wad.client") {
            let hex_wad = format!("{:x}", file_digest);
            wad_hashes.insert(rel_path, hex_wad);
        }
    }

    let folder_hash = format!("{:x}", folder_hasher.finalize());
    Ok((folder_hash, wad_hashes))
}

/// Locate Rose mod targets manifest for a given mod directory if it exists.
pub fn find_rose_manifest_path(mod_dir: &Path) -> Option<PathBuf> {
    let parent = mod_dir.parent()?;
    let target_meta = parent.join(TARGET_METADATA);
    if target_meta.is_file() {
        return Some(target_meta);
    }
    let legacy_meta = parent.join(LEGACY_TARGET_METADATA);
    if legacy_meta.is_file() {
        return Some(legacy_meta);
    }
    None
}

/// Update Rose's `rose_mod_targets.json` if this mod folder is inside a Rose skin directory.
///
/// Returns `Ok(true)` if a Rose manifest was found and updated, `Ok(false)` if no manifest was present.
pub fn update_rose_manifest_if_present(mod_dir: &Path) -> FixerResult<bool> {
    let manifest_path = match find_rose_manifest_path(mod_dir) {
        Some(p) => p,
        None => return Ok(false),
    };

    let mod_name = match mod_dir.file_name().and_then(|n| n.to_str()) {
        Some(name) => name,
        None => return Ok(false),
    };

    let content = match fs_err::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "Failed to read Rose manifest at {}: {e}",
                manifest_path.display()
            );
            return Ok(false);
        }
    };

    let mut payload: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "Failed to parse Rose manifest at {}: {e}",
                manifest_path.display()
            );
            return Ok(false);
        }
    };

    let mods_obj = match payload.get_mut("mods").and_then(|m| m.as_object_mut()) {
        Some(o) => o,
        None => return Ok(false),
    };

    // Calculate new hashes for the repaired folder
    let (new_folder_hash, new_wad_hashes) = compute_rose_hashes(mod_dir)?;

    // Check if META/info.json provides an alternative mod Name
    let meta_info_name: Option<String> = {
        let meta_info = mod_dir.join("META").join("info.json");
        let meta_info_lower = mod_dir.join("meta").join("info.json");
        let info_path = if meta_info.is_file() {
            Some(meta_info)
        } else if meta_info_lower.is_file() {
            Some(meta_info_lower)
        } else {
            None
        };
        info_path
            .and_then(|p| fs_err::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| {
                v.get("Name")
                    .or_else(|| v.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            })
    };

    // Find the matching entry for this mod:
    // 0. By matching folderHash or key
    // 1. By matching wadHashes
    // 2. By exact name or case-insensitive folder name
    // 3. By matching metadata info.json Name
    // 4. Fallback: if only 1 mod exists in this champion's manifest
    let mut matched_key: Option<String> = None;
    for (key, val) in mods_obj.iter() {
        if key == &new_folder_hash
            || val.get("folderHash").and_then(|h| h.as_str()) == Some(&new_folder_hash)
        {
            matched_key = Some(key.clone());
            break;
        }

        if let Some(entry_wads) = val.get("wadHashes").and_then(|w| w.as_object()) {
            if !new_wad_hashes.is_empty() {
                let mut matches_all_wads = true;
                for (wad_path, wad_hash) in &new_wad_hashes {
                    if let Some(existing_hash) = entry_wads.get(wad_path).and_then(|h| h.as_str()) {
                        if existing_hash != wad_hash {
                            matches_all_wads = false;
                            break;
                        }
                    } else {
                        matches_all_wads = false;
                        break;
                    }
                }
                if matches_all_wads {
                    matched_key = Some(key.clone());
                    break;
                }
            }
        }

        let entry_name = val.get("name").and_then(|n| n.as_str());
        if entry_name == Some(mod_name)
            || key == mod_name
            || entry_name
                .map(|n| n.eq_ignore_ascii_case(mod_name))
                .unwrap_or(false)
        {
            matched_key = Some(key.clone());
            break;
        }

        if let Some(ref m_name) = meta_info_name {
            if entry_name == Some(m_name)
                || entry_name
                    .map(|n| n.eq_ignore_ascii_case(m_name))
                    .unwrap_or(false)
            {
                matched_key = Some(key.clone());
                break;
            }
        }
    }

    if matched_key.is_none() && mods_obj.len() == 1 {
        matched_key = mods_obj.keys().next().cloned();
    }

    let matched_key = match matched_key {
        Some(k) => k,
        None => {
            tracing::debug!(
                "Mod '{}' not found in Rose manifest {}",
                mod_name,
                manifest_path.display()
            );
            return Ok(false);
        }
    };

    // Extract and update entry
    let mut entry = mods_obj.remove(&matched_key).unwrap_or(serde_json::json!({
        "name": mod_name
    }));

    if let Some(entry_map) = entry.as_object_mut() {
        entry_map.insert("name".to_string(), serde_json::json!(mod_name));
        entry_map.insert(
            "folderHash".to_string(),
            serde_json::json!(&new_folder_hash),
        );
        entry_map.insert(
            "wadHashes".to_string(),
            serde_json::to_value(&new_wad_hashes).unwrap_or_default(),
        );
    }

    // Re-key under the new folder_hash (Rose standard format)
    mods_obj.insert(new_folder_hash.clone(), entry);

    // Save atomically
    let tmp_path = manifest_path.with_file_name(format!(
        ".{}.tmp",
        manifest_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    let serialized = serde_json::to_string_pretty(&payload)?;
    fs_err::write(&tmp_path, serialized)?;
    fs_err::rename(&tmp_path, &manifest_path)?;

    tracing::debug!(
        "Updated Rose mod manifest at {} for '{}' (new folderHash: {})",
        manifest_path.display(),
        mod_name,
        new_folder_hash
    );

    Ok(true)
}

/// Check if a directory is an active Rose category directory (e.g. `Rose/mods/maps`, `Rose/mods/others`).
pub fn is_rose_category_dir(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    let dir_name = match dir.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_ascii_lowercase(),
        None => return false,
    };

    if dir_name == "skins" || dir_name.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // If an existing manifest is present, process it
    if dir.join(ROSE_CATEGORY_MODS).is_file() {
        return true;
    }

    let path_str = dir.to_string_lossy().to_ascii_lowercase();
    let is_in_rose_mods = path_str.contains("rose\\mods") || path_str.contains("rose/mods");

    // Check if dir actually contains non-empty subdirectories or candidate archives
    let mut has_mods = false;
    if let Ok(entries) = fs_err::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || name.eq_ignore_ascii_case("hematite-fixed")
                || name.eq_ignore_ascii_case("backup")
                || name.eq_ignore_ascii_case(".broken")
            {
                continue;
            }
            if entry.path().is_dir() && !crate::cleanup::is_dir_empty(&entry.path()) {
                has_mods = true;
                break;
            }
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".fantome") || lower.ends_with(".zip") || lower.ends_with(".modpkg") {
                has_mods = true;
                break;
            }
        }
    }

    if has_mods {
        if is_in_rose_mods && KNOWN_ROSE_CATEGORIES.iter().any(|&c| c == dir_name) {
            return true;
        }

        if let Some(parent) = dir.parent() {
            let parent_name = parent
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if parent_name == "mods" {
                return true;
            }
        }
    }

    false
}

/// Recursively discover all Rose category directories under `base_dir` (or `base_dir` itself).
pub fn find_rose_category_directories(base_dir: &Path) -> Vec<PathBuf> {
    if !base_dir.is_dir() {
        return Vec::new();
    }

    if is_rose_category_dir(base_dir) {
        return vec![base_dir.to_path_buf()];
    }

    let mut found = Vec::new();

    // Check direct children first (e.g. if base_dir is `Rose/mods`)
    if let Ok(entries) = fs_err::read_dir(base_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() && is_rose_category_dir(&p) {
                found.push(p);
            }
        }
    }

    // Also search subdirectories up to depth 4
    for entry in walkdir::WalkDir::new(base_dir)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.into_path();
        if p.is_dir() && is_rose_category_dir(&p) && !found.contains(&p) {
            found.push(p);
        }
    }

    found.sort();
    found
}

/// Rebuild or synchronize `rose_category_mods.json` for a Rose category directory (e.g. `maps`, `others`).
pub fn rebuild_rose_category_manifest(category_dir: &Path) -> FixerResult<bool> {
    if !category_dir.is_dir() {
        return Ok(false);
    }

    let manifest_path = category_dir.join(ROSE_CATEGORY_MODS);
    let category_name = category_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("others")
        .to_string();

    // Collect valid mod subdirectories
    let mut mod_folders = Vec::new();
    if let Ok(entries) = fs_err::read_dir(category_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.')
                    || name.eq_ignore_ascii_case("hematite-fixed")
                    || name.eq_ignore_ascii_case("backup")
                    || name.eq_ignore_ascii_case(".broken")
                {
                    continue;
                }
                if crate::cleanup::is_dir_empty(&p) {
                    continue;
                }
                mod_folders.push(name);
            }
        }
    }

    mod_folders.sort();

    // If there are NO mod folders in this category directory:
    if mod_folders.is_empty() {
        if manifest_path.is_file() {
            let _ = fs_err::remove_file(&manifest_path);
            return Ok(true);
        }
        return Ok(false);
    }

    // Read existing manifest if present
    let existing_raw = if manifest_path.is_file() {
        fs_err::read_to_string(&manifest_path).ok()
    } else {
        None
    };

    let existing_json: Option<serde_json::Value> = existing_raw
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    let version = existing_json
        .as_ref()
        .and_then(|j| j.get("version"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;

    let cat = existing_json
        .as_ref()
        .and_then(|j| j.get("category"))
        .and_then(|c| c.as_str())
        .unwrap_or(&category_name)
        .to_string();

    let mut mods_map: std::collections::BTreeMap<String, serde_json::Value> = std::collections::BTreeMap::new();
    for name in &mod_folders {
        mods_map.insert(
            name.clone(),
            serde_json::json!({
                "name": name,
                "path": name
            }),
        );
    }

    let manifest_data = serde_json::json!({
        "version": version,
        "category": cat,
        "mods": mods_map
    });

    // Check if identical to existing
    if let Some(existing) = &existing_json {
        if existing == &manifest_data {
            return Ok(false);
        }
    }

    let tmp_path = category_dir.join(format!(".{}.tmp", ROSE_CATEGORY_MODS));
    let content = serde_json::to_string_pretty(&manifest_data)?;
    fs_err::write(&tmp_path, content)?;
    fs_err::rename(&tmp_path, &manifest_path)?;

    tracing::info!(
        "Synchronized Rose category manifest at {} with {} mod(s)",
        manifest_path.display(),
        mods_map.len()
    );

    Ok(true)
}

/// Update Rose category manifest if a mod directory resides within a Rose category.
pub fn update_rose_category_manifest_if_present(mod_dir: &Path) -> FixerResult<bool> {
    if let Some(parent) = mod_dir.parent() {
        if is_rose_category_dir(parent) {
            return rebuild_rose_category_manifest(parent);
        }
    }
    Ok(false)
}
