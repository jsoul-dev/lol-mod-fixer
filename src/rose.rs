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

    // Find the matching entry for this mod:
    // 1. By entry.name == mod_name
    // 2. By key == mod_name
    // 3. By matching old folderHash or wadHashes
    let mut matched_key: Option<String> = None;
    for (key, val) in mods_obj.iter() {
        let entry_name = val.get("name").and_then(|n| n.as_str());
        if entry_name == Some(mod_name) || key == mod_name {
            matched_key = Some(key.clone());
            break;
        }
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

    tracing::info!(
        "Updated Rose mod manifest at {} for '{}' (new folderHash: {})",
        manifest_path.display(),
        mod_name,
        new_folder_hash
    );

    Ok(true)
}
