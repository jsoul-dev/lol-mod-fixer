//! Migration from outdated Rose mod skins structure to the modern Rose structure.
//!
//! In legacy Rose (or older backups), mods were stored as raw `.fantome` and `.zip`
//! archive files directly inside numeric skin target folders (e.g. `skins/24000/Mod.fantome`)
//! without extracted folders and without `rose_mod_targets.json`.
//!
//! This module:
//! 1. Detects if a directory is a Rose skins directory with outdated archive files.
//! 2. Safely extracts each archive into its extracted mod folder (`META/`, `WAD/`, etc.).
//! 3. Flattens single-root folder wrappers (e.g. `ModName/META/` -> `META/`).
//! 4. Validates the extracted mod folder.
//! 5. Unlinks the original archive upon verified extraction.
//! 6. Builds/updates `rose_mod_targets.json` with exact `folderHash` and `wadHashes`
//!    matching Rose's official hashing algorithm for full client and party mode injection support.
//! 7. Leaves already updated structures (modern extracted folders) 100% untouched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

use crate::error::{FixerError, FixerResult};
use crate::rose::compute_rose_hashes;

const ROSE_MOD_TARGETS: &str = "rose_mod_targets.json";
const MAX_FLATTEN_DEPTH: usize = 10;

/// Migration summary report.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Number of numeric skin target directories scanned.
    pub scanned_targets: usize,
    /// Number of archives successfully extracted and converted.
    pub extracted_archives: usize,
    /// Number of `rose_mod_targets.json` manifests built or updated.
    pub manifests_rebuilt: usize,
    /// Number of archives skipped (e.g. already extracted).
    pub skipped_archives: usize,
    /// Number of archives that failed extraction.
    pub failed_archives: usize,
}

impl MigrationReport {
    pub fn is_empty(&self) -> bool {
        self.extracted_archives == 0 && self.manifests_rebuilt == 0 && self.failed_archives == 0
    }
}

/// Check if a directory is a Rose skins root or contains numeric skin target folders.
pub fn is_rose_skins_directory(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    let path_str = dir.to_string_lossy().to_ascii_lowercase();
    if path_str.contains("rose\\mods\\skins") || path_str.contains("rose/mods/skins") {
        return true;
    }

    // Check if base_dir contains numeric subdirectories
    if let Ok(entries) = fs_err::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.chars().all(|c| c.is_ascii_digit()) && !name.is_empty() {
                    // Check if numeric folder contains archives or rose manifests
                    if let Ok(sub_entries) = fs_err::read_dir(&p) {
                        for sub in sub_entries.filter_map(|e| e.ok()) {
                            let sub_name = sub.file_name().to_string_lossy().to_ascii_lowercase();
                            if sub_name == ROSE_MOD_TARGETS
                                || sub_name.ends_with(".fantome")
                                || sub_name.ends_with(".zip")
                            {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    false
}

/// Safely extract a zip or fantome archive into destination.
/// Prevents zip-slip path traversal attacks.
pub fn safe_extract_archive(archive_path: &Path, destination: &Path) -> FixerResult<()> {
    let file = fs_err::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| {
        FixerError::Archive(format!(
            "Failed to open archive {}: {e}",
            archive_path.display()
        ))
    })?;

    for i in 0..archive.len() {
        let mut zip_file = archive.by_index(i).map_err(|e| {
            FixerError::Archive(format!(
                "Failed to read archive entry #{i} in {}: {e}",
                archive_path.display()
            ))
        })?;

        // enclosed_name() ensures paths do not escape destination (prevents zip-slip)
        let enclosed = match zip_file.enclosed_name() {
            Some(p) => p.to_owned(),
            None => continue,
        };

        let out_path = destination.join(enclosed);

        if zip_file.is_dir() {
            fs_err::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                if !parent.exists() {
                    fs_err::create_dir_all(parent)?;
                }
            }
            let mut outfile = fs_err::File::create(&out_path)?;
            std::io::copy(&mut zip_file, &mut outfile)?;
        }
    }

    Ok(())
}

/// Flatten single-root wrapper directories if an archive was zipped with an extra parent folder.
/// E.g. `SkinName/META/...` -> `META/...`
pub fn flatten_single_root_folder(destination: &Path) -> FixerResult<()> {
    for _ in 0..MAX_FLATTEN_DEPTH {
        let entries: Vec<_> = match fs_err::read_dir(destination) {
            Ok(e) => e.filter_map(|item| item.ok()).collect(),
            Err(_) => return Ok(()),
        };

        if entries.len() != 1 {
            return Ok(());
        }

        let single_entry = &entries[0];
        let root = single_entry.path();
        if !root.is_dir() {
            return Ok(());
        }

        let root_entries: Vec<_> = match fs_err::read_dir(&root) {
            Ok(e) => e.filter_map(|item| item.ok()).collect(),
            Err(_) => return Ok(()),
        };

        let has_mod_markers = root_entries.iter().any(|e| {
            let name = e.file_name().to_string_lossy().to_ascii_uppercase();
            name == "META" || name == "WAD" || name == "RAW" || name == "DATA"
        });

        if !has_mod_markers {
            return Ok(());
        }

        // Move all items from root to destination
        for item in root_entries {
            let target = destination.join(item.file_name());
            if target.exists() {
                if target.is_dir() {
                    fs_err::remove_dir_all(&target)?;
                } else {
                    fs_err::remove_file(&target)?;
                }
            }
            fs_err::rename(item.path(), target)?;
        }

        let _ = fs_err::remove_dir(&root);
    }

    Ok(())
}

/// Verify that an extracted mod folder contains usable files.
pub fn validate_mod_folder(folder: &Path) -> bool {
    if !folder.is_dir() {
        return false;
    }

    walkdir::WalkDir::new(folder)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| e.file_type().is_file())
}

/// Check if a directory looks like an extracted Rose mod (has META or WAD or RAW).
pub fn looks_like_mod_folder(folder: &Path) -> bool {
    if !folder.is_dir() {
        return false;
    }

    if let Ok(entries) = fs_err::read_dir(folder) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
            if name == "META" || name == "WAD" || name == "RAW" {
                return true;
            }
        }
    }

    false
}

/// Build or rebuild `rose_mod_targets.json` for a specific skin target directory.
///
/// Ensures exact `folderHash`, `wadHashes`, and `targets` match Rose's hashing algorithm
/// so mods are fully recognized by Rose client and injected in party mode.
pub fn rebuild_target_manifest(target_folder: &Path, target: u32) -> FixerResult<bool> {
    let champion_id = target / 1000;
    let json_path = target_folder.join(ROSE_MOD_TARGETS);

    // Collect all valid mod subdirectories
    let mut mod_folders = Vec::new();
    if let Ok(entries) = fs_err::read_dir(target_folder) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name.eq_ignore_ascii_case("hematite-fixed") {
                    continue;
                }
                if validate_mod_folder(&p) {
                    mod_folders.push(p);
                }
            }
        }
    }

    if mod_folders.is_empty() {
        return Ok(false);
    }

    mod_folders.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let mut mods_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for mod_dir in mod_folders {
        let mod_name = mod_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let (folder_hash, wad_hashes) = match compute_rose_hashes(&mod_dir) {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("Could not compute Rose hashes for {}: {e}", mod_dir.display());
                continue;
            }
        };

        let entry = serde_json::json!({
            "name": mod_name,
            "folderHash": folder_hash,
            "wadHashes": wad_hashes,
            "targets": [ target ]
        });

        mods_map.insert(folder_hash, entry);
    }

    if mods_map.is_empty() {
        return Ok(false);
    }

    let manifest_data = serde_json::json!({
        "version": 1,
        "championId": champion_id,
        "targets": [ target ],
        "mods": mods_map
    });

    let tmp_path = target_folder.join(format!(".{}.tmp", ROSE_MOD_TARGETS));
    let content = serde_json::to_string_pretty(&manifest_data)?;
    fs_err::write(&tmp_path, content)?;
    fs_err::rename(&tmp_path, &json_path)?;

    tracing::info!(
        "Rebuilt Rose manifest at {} with {} mod(s)",
        json_path.display(),
        mods_map.len()
    );

    Ok(true)
}

/// Migrate outdated Rose skins directory structure:
/// Extracts `.fantome` and `.zip` archives into proper folders and creates `rose_mod_targets.json`.
///
/// SAFE: Already updated structures (extracted folders with manifests) are left untouched.
pub fn migrate_outdated_rose_directory(base_dir: &Path) -> FixerResult<MigrationReport> {
    let mut report = MigrationReport::default();

    if !base_dir.is_dir() {
        return Ok(report);
    }

    // Collect numeric target subdirectories
    let mut target_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs_err::read_dir(base_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.chars().all(|c| c.is_ascii_digit()) && !name.is_empty() {
                    target_dirs.push(p);
                }
            }
        }
    }

    target_dirs.sort_by(|a, b| {
        let a_num = a
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.parse::<u64>().ok());
        let b_num = b
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.parse::<u64>().ok());
        match (a_num, b_num) {
            (Some(an), Some(bn)) => an.cmp(&bn),
            _ => a.cmp(b),
        }
    });

    for target_dir in target_dirs {
        report.scanned_targets += 1;

        let target_id: u32 = match target_dir
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            Some(id) => id,
            None => continue,
        };

        // Find raw archives (.fantome and .zip) directly in the numeric target folder
        let mut archives: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = fs_err::read_dir(&target_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() {
                    let lower = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase();
                    if lower.ends_with(".fantome") || lower.ends_with(".zip") {
                        archives.push(p);
                    }
                }
            }
        }

        let mut extracted_any = false;

        for archive in archives {
            let stem = match archive.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };

            let destination = target_dir.join(stem);

            // If folder already exists and is valid, skip extraction and remove redundant archive
            if destination.exists() {
                if validate_mod_folder(&destination) {
                    tracing::debug!(
                        "Mod folder already extracted: {}\\{}",
                        target_dir.file_name().unwrap_or_default().to_string_lossy(),
                        stem
                    );
                    let _ = fs_err::remove_file(&archive);
                    report.skipped_archives += 1;
                    continue;
                } else {
                    // Remove incomplete extraction folder
                    let _ = fs_err::remove_dir_all(&destination);
                }
            }

            // Extract archive into destination
            match safe_extract_archive(&archive, &destination) {
                Ok(()) => {
                    let _ = flatten_single_root_folder(&destination);

                    if validate_mod_folder(&destination) {
                        // Success! Safely unlink original archive
                        let _ = fs_err::remove_file(&archive);
                        tracing::info!(
                            "Migrated and extracted: {}\\{} -> {}",
                            target_dir.file_name().unwrap_or_default().to_string_lossy(),
                            archive.file_name().unwrap_or_default().to_string_lossy(),
                            stem
                        );
                        report.extracted_archives += 1;
                        extracted_any = true;
                    } else {
                        // Empty or invalid extraction: cleanup incomplete destination, preserve original
                        let _ = fs_err::remove_dir_all(&destination);
                        tracing::warn!(
                            "Extraction produced no usable files for {}",
                            archive.display()
                        );
                        report.failed_archives += 1;
                    }
                }
                Err(e) => {
                    let _ = fs_err::remove_dir_all(&destination);
                    tracing::warn!("Failed to extract archive {}: {e}", archive.display());
                    report.failed_archives += 1;
                }
            }
        }

        // Rebuild or ensure rose_mod_targets.json exists if archives were extracted
        // or if rose_mod_targets.json is missing while mod folders exist
        let manifest_path = target_dir.join(ROSE_MOD_TARGETS);
        if extracted_any || !manifest_path.exists() {
            if let Ok(rebuilt) = rebuild_target_manifest(&target_dir, target_id) {
                if rebuilt {
                    report.manifests_rebuilt += 1;
                }
            }
        }
    }

    Ok(report)
}
