//! Safe atomic file replacement with backup support.

use fs_err as fs;
use std::path::Path;
use uuid::Uuid;

use crate::error::{FixerError, FixerResult};

/// Replace `target` with the contents of `source` safely.
///
/// If `backup` is true, creates a `.bak` backup copy before replacing.
/// The replacement writes to a temporary file in the target directory first,
/// then atomically swaps it into place so the original is never lost on interruption.
pub fn replace_file_safely(target: &Path, source: &Path, backup: bool) -> FixerResult<()> {
    if !source.exists() {
        return Err(FixerError::Replacement(format!(
            "Source file to replace with does not exist: {}",
            source.display()
        )));
    }

    let parent = target.parent().ok_or_else(|| {
        FixerError::Replacement(format!(
            "Target file has no parent directory: {}",
            target.display()
        ))
    })?;

    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    // Create backup if requested
    if backup && target.exists() {
        let backup_path = target.with_extension(format!(
            "{}.bak",
            target.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        fs::copy(target, &backup_path)?;
    }

    // Temporary file in the same directory (guarantees same filesystem/volume for rename)
    let temp_name = format!(
        ".{}.tmp-{}",
        target.file_name().and_then(|n| n.to_str()).unwrap_or("mod"),
        Uuid::new_v4()
    );
    let temp_path = parent.join(temp_name);

    // Copy source to temporary location
    fs::copy(source, &temp_path)?;

    // If target doesn't exist yet, simple rename
    if !target.exists() {
        fs::rename(&temp_path, target)?;
        return Ok(());
    }

    // Target exists: safe swap
    let replaced_name = format!(
        ".{}.replaced-{}",
        target.file_name().and_then(|n| n.to_str()).unwrap_or("mod"),
        Uuid::new_v4()
    );
    let replaced_path = parent.join(replaced_name);

    // Step 1: Move target to replaced_path
    if let Err(e) = fs::rename(target, &replaced_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(FixerError::Replacement(format!(
            "Failed to move existing target out of the way {}: {}",
            target.display(),
            e
        )));
    }

    // Step 2: Move temp_path to target
    if let Err(e) = fs::rename(&temp_path, target) {
        // Rollback: try to restore original
        let _ = fs::rename(&replaced_path, target);
        let _ = fs::remove_file(&temp_path);
        return Err(FixerError::Replacement(format!(
            "Failed to put repaired file into place {}: {}",
            target.display(),
            e
        )));
    }

    // Step 3: Remove old replaced file
    let _ = fs::remove_file(&replaced_path);

    // Verify target exists and is not empty
    let metadata = fs::metadata(target)?;
    if metadata.len() == 0 {
        return Err(FixerError::Verification(format!(
            "Replaced output file is empty: {}",
            target.display()
        )));
    }

    Ok(())
}
