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


/// Check if a filename is an OS junk/metadata file (e.g. desktop.ini, Thumbs.db, .DS_Store).
pub fn is_os_metadata_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "desktop.ini"
        || lower == "thumbs.db"
        || lower == ".ds_store"
        || lower.starts_with("._")
}

/// Safely remove a file, clearing readonly attribute if needed.
pub fn force_remove_file(path: &Path) -> std::io::Result<()> {
    if let Ok(meta) = fs_err::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = fs_err::set_permissions(path, perms);
        }
    }
    fs_err::remove_file(path)
}

/// Check if a directory is completely empty or contains ONLY OS junk/metadata files (e.g. desktop.ini).
pub fn is_dir_empty(path: &Path) -> bool {
    match fs_err::read_dir(path) {
        Ok(entries) => {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_file() && is_os_metadata_file(&name) {
                    continue;
                }
                return false;
            }
            true
        }
        Err(_) => false,
    }
}

/// Safely remove a directory, clearing readonly attribute and unlocking permissions if needed.
pub fn force_remove_dir(path: &Path) -> std::io::Result<()> {
    if let Ok(meta) = fs_err::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = fs_err::set_permissions(path, perms);
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn SetFileAttributesW(lpFileName: *const u16, dwFileAttributes: u32) -> i32;
        }
        const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
        unsafe {
            let _ = SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_DIRECTORY);
        }
    }

    match fs_err::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(e) => {
            if crate::permissions::is_elevated() {
                crate::permissions::unlock_folder_permissions(path);
                fs_err::remove_dir(path)
            } else {
                Err(e)
            }
        }
    }
}

/// Clean OS metadata files in a directory and remove the directory itself.
pub fn clean_and_remove_dir(path: &Path) -> std::io::Result<()> {
    if let Ok(entries) = fs_err::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if is_os_metadata_file(&name) || is_rose_target_file(&name) {
                    let _ = force_remove_file(&p);
                }
            }
        }
    }
    force_remove_dir(path)
}

/// Check if a filename is a known Rose target manifest or related temp file.
pub fn is_rose_target_file(name: &str) -> bool {
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
            match clean_and_remove_dir(&folder) {
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
            match clean_and_remove_dir(&dir) {
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
    // Either base_dir itself if numeric, or numeric children of base_dir, or directories with Rose metadata.
    let mut target_dirs: Vec<PathBuf> = Vec::new();

    let base_name = base_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let base_is_numeric = base_name.chars().all(|c| c.is_ascii_digit()) && !base_name.is_empty();

    if base_is_numeric {
        target_dirs.push(base_dir.to_path_buf());
    } else if let Ok(read_entries) = fs_err::read_dir(base_dir) {
        for entry in read_entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let is_numeric = name.chars().all(|c| c.is_ascii_digit()) && !name.is_empty();
            let has_manifest =
                p.join(ROSE_MOD_TARGETS).is_file() || p.join(ROSE_WAD_TARGETS).is_file();

            if is_numeric || has_manifest {
                target_dirs.push(p);
            }
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
        // Clean any misplaced skin_mappings.txt or skin_mappings.json inside individual target directories
        let stray_txt = target_dir.join(crate::mapping::MAPPING_TXT_FILENAME);
        let stray_json = target_dir.join(crate::mapping::MAPPING_JSON_FILENAME);
        if stray_txt.is_file() {
            let _ = force_remove_file(&stray_txt);
        }
        if stray_json.is_file() {
            let _ = force_remove_file(&stray_json);
        }

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
                        match clean_and_remove_dir(&mod_path) {
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

        let mut subdirs = Vec::new();
        let mut manifest_files = Vec::new();
        let mut metadata_files = Vec::new();
        let mut other_files = Vec::new();

        for item in &remaining_items {
            let p = item.path();
            let name = item.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                subdirs.push(p);
            } else if is_rose_target_file(&name) {
                manifest_files.push(p);
            } else if is_os_metadata_file(&name) {
                metadata_files.push(p);
            } else {
                other_files.push(p);
            }
        }

        // Target directory has no mod folders and no other files (may have desktop.ini and/or orphan manifests)
        if subdirs.is_empty() && other_files.is_empty() {
            for m in &manifest_files {
                if force_remove_file(m).is_ok() {
                    report.deleted_manifest_files += 1;
                }
            }
            for meta in &metadata_files {
                let _ = force_remove_file(meta);
            }

            match force_remove_dir(&target_dir) {
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
        }
    }

    // Step 3: Final pass on empty Hematite-Fixed folders
    report.deleted_hematite_fixed += clean_empty_hematite_fixed(base_dir);

    Ok(report)
}
