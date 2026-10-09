//! Automatic cleanup of empty folders and orphan Rose manifests.
//!
//! Emulates and extends `CleanupEmptyFolders.py`:
//! 1. Recursively removes empty `Hematite-Fixed` folders.
//! 2. Removes empty mod subdirectories inside target directories.
//! 3. Removes target directories that are completely empty or contain only
//!    orphan `rose_mod_targets.json` / `rose_wad_targets.json`.
//! 4. Performs a final pass on empty `Hematite-Fixed` directories.

use std::path::{Path, PathBuf};

use crate::error::FixerResult;

const ROSE_MOD_TARGETS: &str = "rose_mod_targets.json";
const ROSE_WAD_TARGETS: &str = "rose_wad_targets.json";

/// Summary report of deleted empty folders and orphan metadata files.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CleanupReport {
    /// Number of empty or orphan target directories removed (e.g. `105000`).
    pub deleted_targets: usize,
    /// Number of empty mod directories removed.
    pub deleted_mods: usize,
    /// Number of empty `Hematite-Fixed` directories removed.
    pub deleted_hematite_fixed: usize,
    /// Number of empty internal mod subdirectories removed (e.g. empty `META/hashes`).
    pub deleted_empty_subdirs: usize,
    /// Number of orphan JSON manifest files removed before folder deletion.
    pub deleted_manifest_files: usize,
}

impl CleanupReport {
    pub fn is_empty(&self) -> bool {
        self.deleted_targets == 0
            && self.deleted_mods == 0
            && self.deleted_hematite_fixed == 0
            && self.deleted_empty_subdirs == 0
            && self.deleted_manifest_files == 0
    }

    pub fn total_deleted_folders(&self) -> usize {
        self.deleted_targets
            + self.deleted_mods
            + self.deleted_hematite_fixed
            + self.deleted_empty_subdirs
    }
}


/// Check if a directory is completely empty (0 entries).
pub fn is_dir_empty(path: &Path) -> bool {
    match fs_err::read_dir(path) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => false,
    }
}

/// Check if a filename is a known Rose target manifest or related temp file.
fn is_rose_target_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == ROSE_MOD_TARGETS
        || lower == ROSE_WAD_TARGETS
        || (lower.starts_with('.') && lower.ends_with(".tmp"))
}

/// Recursively find and remove all empty directories named `Hematite-Fixed` (case-insensitive).
pub fn clean_empty_hematite_fixed(base_dir: &Path) -> usize {
    let mut deleted = 0;
    let mut hematite_folders = Vec::new();

    for entry in walkdir::WalkDir::new(base_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_dir()
            && entry.file_name().to_string_lossy().eq_ignore_ascii_case("hematite-fixed")
        {
            hematite_folders.push(entry.into_path());
        }
    }

    // Sort by path depth descending so nested empty folders are removed first
    hematite_folders.sort_by(|a, b| b.components().count().cmp(&a.components().count()));

    for folder in hematite_folders {
        if is_dir_empty(&folder) {
            match fs_err::remove_dir(&folder) {
                Ok(()) => {
                    tracing::debug!("Deleted empty Hematite-Fixed folder: {}", folder.display());
                    deleted += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        "Could not delete empty Hematite-Fixed folder {}: {e}",
                        folder.display()
                    );
                }
            }
        }
    }

    deleted
}

/// Recursively scan inside an active mod directory and remove any empty internal subdirectories
/// (e.g. an empty `META/hashes` folder left behind by mod export tools).
///
/// CRITICAL SAFETY GUARANTEE:
/// Any folder that contains ANY files or non-empty items (such as `META` with `info.json`,
/// or `META/hashes` with `*.hashes.txt` files) is 100% PRESERVED and left untouched.
pub fn clean_empty_leaf_subdirs(mod_dir: &Path, report: &mut CleanupReport) {
    let mut subdirs = Vec::new();
    for entry in walkdir::WalkDir::new(mod_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_dir() && entry.path() != mod_dir {
            subdirs.push(entry.into_path());
        }
    }

    // Sort by depth descending so innermost subdirectories are inspected first
    subdirs.sort_by(|a, b| b.components().count().cmp(&a.components().count()));

    for dir in subdirs {
        if is_dir_empty(&dir) {
            let rel = dir.strip_prefix(mod_dir).unwrap_or(&dir);
            match fs_err::remove_dir(&dir) {
                Ok(()) => {
                    tracing::info!(
                        "Deleted empty internal mod subfolder: {}\\{}",
                        mod_dir.file_name().unwrap_or_default().to_string_lossy(),
                        rel.display()
                    );
                    report.deleted_empty_subdirs += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        "Could not delete empty internal subfolder {}: {e}",
                        dir.display()
                    );
                }
            }
        }
    }
}

/// Clean empty folders and orphan target manifests in the specified directory.
pub fn cleanup_empty_rose_folders(base_dir: &Path) -> FixerResult<CleanupReport> {

    let mut report = CleanupReport::default();

    if !base_dir.is_dir() {
        return Ok(report);
    }

    // Step 0: Initial pass on empty Hematite-Fixed folders
    report.deleted_hematite_fixed += clean_empty_hematite_fixed(base_dir);

    // Identify candidate target directories:
    // Either numeric names (e.g. "106000", "33000") or directories containing rose target metadata.
    let mut target_dirs: Vec<PathBuf> = Vec::new();

    let read_entries = match fs_err::read_dir(base_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("Could not read directory {}: {e}", base_dir.display());
            return Ok(report);
        }
    };

    for entry in read_entries.filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let is_numeric = name.chars().all(|c| c.is_ascii_digit()) && !name.is_empty();
        let has_manifest = p.join(ROSE_MOD_TARGETS).is_file() || p.join(ROSE_WAD_TARGETS).is_file();

        if is_numeric || has_manifest {
            target_dirs.push(p);
        }
    }

    // Sort targets numerically if possible, otherwise by string
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
        // Step 1: Clean empty mod subdirectories inside this target directory
        if let Ok(entries) = fs_err::read_dir(&target_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let mod_path = entry.path();
                if mod_path.is_dir() {
                    let mod_name = entry.file_name().to_string_lossy().to_string();
                    if mod_name.eq_ignore_ascii_case("hematite-fixed") {
                        continue;
                    }
                    if is_dir_empty(&mod_path) {
                        match fs_err::remove_dir(&mod_path) {
                            Ok(()) => {
                                tracing::info!(
                                    "Deleted empty mod folder: {}\\{}",
                                    target_dir.file_name().unwrap_or_default().to_string_lossy(),
                                    mod_name
                                );
                                report.deleted_mods += 1;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Could not delete empty mod folder {}: {e}",
                                    mod_path.display()
                                );
                            }
                        }
                    } else {
                        // Mod is not empty. Clean truly empty internal subfolders (e.g. empty META/hashes)
                        clean_empty_leaf_subdirs(&mod_path, &mut report);
                    }
                }
            }
        }


        // Step 2: Check target folder after mod cleanup
        let remaining_items: Vec<_> = match fs_err::read_dir(&target_dir) {
            Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
            Err(_) => continue,
        };

        if remaining_items.is_empty() {
            // Target is completely empty
            match fs_err::remove_dir(&target_dir) {
                Ok(()) => {
                    tracing::info!(
                        "Deleted empty target folder: {}",
                        target_dir.file_name().unwrap_or_default().to_string_lossy()
                    );
                    report.deleted_targets += 1;
                }
                Err(e) => {
                    tracing::warn!("Could not delete target folder {}: {e}", target_dir.display());
                }
            }
            continue;
        }

        // Check if target contains ONLY Rose target manifest files and 0 subdirectories/other files
        let has_subdirectories = remaining_items.iter().any(|item| item.path().is_dir());
        let all_files_are_manifests = !remaining_items.is_empty()
            && remaining_items.iter().all(|item| {
                item.path().is_file()
                    && is_rose_target_file(&item.file_name().to_string_lossy())
            });

        if !has_subdirectories && all_files_are_manifests {
            // Unlink the orphan manifest files
            let mut all_unlinked = true;
            for item in &remaining_items {
                let p = item.path();
                match fs_err::remove_file(&p) {
                    Ok(()) => {
                        report.deleted_manifest_files += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Could not remove manifest {}: {e}", p.display());
                        all_unlinked = false;
                    }
                }
            }

            if all_unlinked {
                match fs_err::remove_dir(&target_dir) {
                    Ok(()) => {
                        tracing::info!(
                            "Deleted empty target folder (orphan manifest): {}",
                            target_dir.file_name().unwrap_or_default().to_string_lossy()
                        );
                        report.deleted_targets += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Could not delete empty target folder {}: {e}",
                            target_dir.display()
                        );
                    }
                }
            }
        }
    }

    // Step 3: Final pass on empty Hematite-Fixed folders
    report.deleted_hematite_fixed += clean_empty_hematite_fixed(base_dir);

    Ok(report)
}
