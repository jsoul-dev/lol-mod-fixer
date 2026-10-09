//! Quarantine mechanism for corrupted and unrepairable mods.
//!
//! Moves unrepairable or broken mods into a hidden `.broken` folder,
//! isolating them so Rose, League of Legends, and mapping generators
//! never load or see them, while preserving files safely on disk.

use std::path::{Path, PathBuf};

use crate::error::{FixerError, FixerResult};

pub const BROKEN_DIR_NAME: &str = ".broken";

/// Determine the target quarantine path for a broken mod.
pub fn resolve_quarantine_path(mod_path: &Path, root_dir: Option<&Path>) -> PathBuf {
    let file_name = mod_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown_mod".to_string());

    if let Some(root) = root_dir {
        if let Ok(rel) = mod_path.strip_prefix(root) {
            return root.join(BROKEN_DIR_NAME).join(rel);
        }
    }

    // Check if the mod is inside a numeric target folder (e.g. skins/119000/mod)
    if let Some(parent) = mod_path.parent() {
        let parent_name = parent
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let is_numeric_target =
            parent_name.chars().all(|c| c.is_ascii_digit()) && !parent_name.is_empty();

        if is_numeric_target {
            if let Some(grandparent) = parent.parent() {
                return grandparent
                    .join(BROKEN_DIR_NAME)
                    .join(parent_name)
                    .join(&file_name);
            }
        }
        return parent.join(BROKEN_DIR_NAME).join(&file_name);
    }

    PathBuf::from(BROKEN_DIR_NAME).join(&file_name)
}

/// Recursively copy a directory and its contents.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs_err::create_dir_all(dst)?;
    for entry in fs_err::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_child)?;
        } else {
            fs_err::copy(entry.path(), &dest_child)?;
        }
    }
    Ok(())
}

/// Safely move a broken mod file or directory to the `.broken` quarantine folder.
///
/// Returns the new destination path in the `.broken` directory upon success.
pub fn quarantine_mod(mod_path: &Path, root_dir: Option<&Path>) -> FixerResult<PathBuf> {
    if !mod_path.exists() {
        return Err(FixerError::Repair(format!(
            "Cannot quarantine nonexistent path: {}",
            mod_path.display()
        )));
    }

    let dest = resolve_quarantine_path(mod_path, root_dir);

    // Ensure the parent directory of dest exists
    if let Some(dest_parent) = dest.parent() {
        fs_err::create_dir_all(dest_parent)?;
    }

    // Remove existing target if it already exists in .broken
    if dest.exists() {
        if dest.is_dir() {
            fs_err::remove_dir_all(&dest)?;
        } else {
            fs_err::remove_file(&dest)?;
        }
    }

    // Attempt fast atomic rename first
    let move_res = fs_err::rename(mod_path, &dest);
    if move_res.is_err() {
        // Fallback for cross-device moves or locked directories
        if mod_path.is_file() {
            fs_err::copy(mod_path, &dest)?;
            fs_err::remove_file(mod_path)?;
        } else if mod_path.is_dir() {
            copy_dir_all(mod_path, &dest)
                .map_err(|e| FixerError::Repair(format!("Failed to copy directory: {e}")))?;
            fs_err::remove_dir_all(mod_path)?;
        }
    }

    // Rebuild or update Rose target manifest if inside a numeric skin target directory
    if let Some(parent) = mod_path.parent() {
        if let Some(target_id) = parent
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            let _ = crate::migrate::rebuild_target_manifest(parent, target_id);
        }
    }

    Ok(dest)
}
