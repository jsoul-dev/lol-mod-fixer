//! Mod name and folder beautification engine.
//!
//! Formats raw mod archives and extracted directories into clean, client-friendly
//! titles conforming to the pattern: `<Skin Name> <Champion Name> v<Version>`.
//!
//! Automatically synchronizes disk directories, `META/info.json`, and Rose's
//! `rose_mod_targets.json`.

use std::path::{Path, PathBuf};

use crate::champions::detect_champion;
use crate::error::FixerResult;
use crate::rose::update_rose_manifest_if_present;

/// Normalize a raw version string into standard `vX.Y` or `vX.Y.Z` format.
///
/// Rules:
/// - Strips leading 'v' / 'V' and whitespace.
/// - Single number `X` -> `vX.0` (e.g. "2" -> "v2.0").
/// - Two numbers `X.Y` -> `vX.Y` (e.g. "2.0" -> "v2.0", "1.1" -> "v1.1").
/// - Three numbers `X.Y.Z`:
///   - If `Z == 0`: `vX.Y` (e.g. "1.0.0" -> "v1.0", "1.1.0" -> "v1.1").
///   - If `Z != 0`: `vX.Y.Z` (e.g. "1.1.2" -> "v1.1.2", "1.1.3" -> "v1.1.3").
pub fn normalize_version(raw: &str) -> String {
    let s = raw.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let parts: Vec<&str> = s.split('.').map(|p| p.trim()).collect();

    if parts.is_empty() || parts[0].is_empty() {
        return "v1.0".to_string();
    }

    match parts.len() {
        1 => {
            if parts[0].chars().all(|c| c.is_ascii_digit()) {
                format!("v{}.0", parts[0])
            } else {
                "v1.0".to_string()
            }
        }
        2 => {
            format!("v{}.{}", parts[0], parts[1])
        }
        3 => {
            let z = parts[2];
            if z == "0" {
                format!("v{}.{}", parts[0], parts[1])
            } else {
                format!("v{}.{}.{}", parts[0], parts[1], z)
            }
        }
        _ => {
            format!("v{}", parts.join("."))
        }
    }
}

/// Extract version from a folder name string if it contains a version suffix.
/// Returns (stripped_name, Option<raw_version>).
pub fn extract_folder_version(name: &str) -> (String, Option<String>) {
    // Check for branch suffixes like -main, -Main, -master, -Master
    let lower = name.to_ascii_lowercase();
    for branch in ["-main", "_main", "-master", "_master", " main", " master"] {
        if lower.ends_with(branch) {
            let stripped = &name[..name.len() - branch.len()];
            return (stripped.trim().to_string(), None);
        }
    }

    // Try splitting on delimiter ('-', '_', or ' ') from the right
    if let Some(idx) = name.rfind(|c| c == '-' || c == '_' || c == ' ') {
        let candidate = &name[idx + 1..];
        let trimmed_cand = candidate.trim_start_matches(|c| c == 'v' || c == 'V');
        // A valid version contains digits and may contain dots
        if !trimmed_cand.is_empty()
            && trimmed_cand.chars().any(|c| c.is_ascii_digit())
            && trimmed_cand
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.')
        {
            let stripped = &name[..idx];
            return (stripped.trim().to_string(), Some(candidate.to_string()));
        }
    }

    (name.trim().to_string(), None)
}

/// Capitalize a token cleanly into Title Case.
fn title_case(token: &str) -> String {
    let lower = token.to_ascii_lowercase();
    // Words to lowercase in titles unless at start
    if lower == "the" || lower == "of" || lower == "and" {
        // We'll capitalize them for clean folder display (e.g. "Shadow The Hedgehog")
        return format!(
            "{}{}",
            token[..1].to_ascii_uppercase(),
            token[1..].to_ascii_lowercase()
        );
    }

    // If already has uppercase (e.g. "Chun", "Li", "Ekko", "Zacian", "Eto"), keep existing casing
    if token.chars().any(|c| c.is_uppercase()) {
        return token.to_string();
    }

    // Otherwise capitalize first letter
    let mut chars = token.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Clean and extract the Skin Name tokens, stripping out champion name and version/branch artifacts.
pub fn clean_skin_name(raw_name: &str, champion: Option<&str>) -> String {
    // Split on delimiters like '_', '-', whitespace, parentheses, brackets
    let tokens: Vec<&str> = raw_name
        .split(|c: char| c == '_' || c == '-' || c == ' ' || c == '(' || c == ')' || c == '[' || c == ']')
        .filter(|t| !t.is_empty())
        .collect();

    // Prepare champion tokens to filter out
    let champ_tokens: Vec<String> = champion
        .map(|c| {
            c.split(|ch: char| !ch.is_alphanumeric())
                .filter(|t| !t.is_empty())
                .map(|t| t.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();

    let mut filtered = Vec::new();
    for token in tokens {
        let t_lower = token.to_ascii_lowercase();

        // Skip artifact words
        if t_lower == "main"
            || t_lower == "master"
            || t_lower == "repath"
            || t_lower == "fixed"
            || t_lower == "mod"
        {
            continue;
        }

        // Skip version-like tokens (e.g. "v1.0", "1.0", "v1.1.2", "2.0")
        let trimmed_v = t_lower.trim_start_matches('v');
        if !trimmed_v.is_empty()
            && trimmed_v.chars().any(|c| c.is_ascii_digit())
            && trimmed_v.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
            continue;
        }

        // Skip champion name tokens
        if champ_tokens.iter().any(|ct| ct == &t_lower) {
            continue;
        }

        filtered.push(title_case(token));
    }

    if filtered.is_empty() {
        return "Custom".to_string();
    }

    filtered.join(" ")
}

/// Compute the beautified title for a mod directory or archive.
pub fn compute_beautified_title(
    folder_name: &str,
    parent_skin_id: Option<&str>,
    info_name: Option<&str>,
    info_version: Option<&str>,
    wad_files: &[&str],
) -> String {
    // 1. Detect champion
    let champ_opt = detect_champion(parent_skin_id, folder_name, info_name, wad_files);

    // 2. Extract version (prefer folder, fallback to info.json, default "1.0")
    let (stripped_folder_name, folder_ver_opt) = extract_folder_version(folder_name);
    let raw_ver = match (folder_ver_opt.as_deref(), info_version) {
        (Some(fv), Some(iv)) => {
            let iv_clean = iv.trim().trim_start_matches(|c| c == 'v' || c == 'V');
            let fv_clean = fv.trim().trim_start_matches(|c| c == 'v' || c == 'V');
            if (fv_clean == "1.0" || fv_clean == "1.0.0") && iv_clean != "1.0" && iv_clean != "1.0.0" {
                iv
            } else {
                fv
            }
        }
        (Some(fv), None) => fv,
        (None, Some(iv)) => iv,
        (None, None) => "1.0",
    };
    let version = normalize_version(raw_ver);

    // 3. Clean skin name
    let skin_name = clean_skin_name(&stripped_folder_name, champ_opt);

    // 4. Assemble: <Skin Name> <Champion Name> <Version>
    match champ_opt {
        Some(champ) => format!("{skin_name} {champ} {version}"),
        None => format!("{skin_name} {version}"),
    }
}

/// Inspect an extracted mod folder, beautify its directory name on disk if needed,
/// update `META/info.json`, and re-synchronize `rose_mod_targets.json`.
///
/// Returns `Ok(Some(new_path))` if renamed/updated, or `Ok(None)` if already up to date.
pub fn beautify_and_sync_folder(mod_dir: &Path) -> FixerResult<Option<PathBuf>> {
    if !mod_dir.is_dir() {
        return Ok(None);
    }

    let current_name = match mod_dir.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return Ok(None),
    };

    let parent_name = mod_dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str());

    // Read info.json if present
    let (info_name, info_version) = {
        let info_path = mod_dir.join("META").join("info.json");
        let info_lower = mod_dir.join("meta").join("info.json");
        let p = if info_path.is_file() {
            Some(info_path)
        } else if info_lower.is_file() {
            Some(info_lower)
        } else {
            None
        };

        if let Some(path) = p {
            if let Ok(content) = fs_err::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let n = val
                        .get("Name")
                        .or_else(|| val.get("name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let v = val
                        .get("Version")
                        .or_else(|| val.get("version"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    (n, v)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    };

    // Collect WAD filenames inside mod_dir
    let mut wad_names = Vec::new();
    let wad_dir = mod_dir.join("WAD");
    let wad_dir_lower = mod_dir.join("wad");
    let active_wad_dir = if wad_dir.is_dir() {
        Some(wad_dir)
    } else if wad_dir_lower.is_dir() {
        Some(wad_dir_lower)
    } else {
        None
    };

    if let Some(wdir) = active_wad_dir {
        if let Ok(entries) = fs_err::read_dir(wdir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    wad_names.push(name.to_string());
                }
            }
        }
    }
    let wad_refs: Vec<&str> = wad_names.iter().map(|s| s.as_str()).collect();

    // Compute target beautified name
    let beautified = compute_beautified_title(
        current_name,
        parent_name,
        info_name.as_deref(),
        info_version.as_deref(),
        &wad_refs,
    );

    let parent_dir = match mod_dir.parent() {
        Some(p) => p,
        None => return Ok(None),
    };
    let target_dir = parent_dir.join(&beautified);

    let needs_rename = current_name != beautified;

    let active_path = if needs_rename {
        // Perform atomic directory rename
        if let Err(e) = fs_err::rename(mod_dir, &target_dir) {
            tracing::warn!(
                "Failed to rename mod folder from '{}' to '{}': {e}",
                mod_dir.display(),
                target_dir.display()
            );
            return Ok(None);
        }
        target_dir
    } else {
        mod_dir.to_path_buf()
    };

    // Update META/info.json with clean Name
    let info_path = active_path.join("META").join("info.json");
    let info_path_lower = active_path.join("meta").join("info.json");
    let target_info_path = if info_path.is_file() {
        Some(info_path)
    } else if info_path_lower.is_file() {
        Some(info_path_lower)
    } else {
        None
    };

    if let Some(ip) = target_info_path {
        if let Ok(content) = fs_err::read_to_string(&ip) {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("Name".to_string(), serde_json::json!(&beautified));
                    let has_valid_version = obj
                        .get("Version")
                        .or_else(|| obj.get("version"))
                        .and_then(|v| v.as_str())
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false);
                    if !has_valid_version {
                        let (_, raw_v) = extract_folder_version(&beautified);
                        let v_clean = normalize_version(raw_v.as_deref().unwrap_or("1.0"));
                        let v_num = v_clean.trim_start_matches('v');
                        obj.insert("Version".to_string(), serde_json::json!(v_num));
                    }
                }
                if let Ok(serialized) = serde_json::to_string_pretty(&val) {
                    let _ = fs_err::write(&ip, serialized);
                }
            }
        }
    }

    // Always re-synchronize Rose manifest with new path & folder hashes
    let _ = update_rose_manifest_if_present(&active_path);

    if needs_rename {
        Ok(Some(active_path))
    } else {
        Ok(None)
    }
}
