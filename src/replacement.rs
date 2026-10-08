//! Safe atomic file and directory replacement with backup support.

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

/// Recursively copy a directory and all of its contents.
pub fn copy_dir_all(src: &Path, dst: &Path) -> FixerResult<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());
        if entry_path.is_dir() {
            copy_dir_all(&entry_path, &target_path)?;
        } else {
            fs::copy(&entry_path, &target_path)?;
        }
    }
    Ok(())
}

/// Replace `target` folder with the contents of `source` folder safely.
///
/// If `backup` is true, creates a `.bak` backup copy before replacing.
/// Performs safe folder replacement using atomic rename where possible.
pub fn replace_folder_safely(target: &Path, source: &Path, backup: bool) -> FixerResult<()> {
    if !source.exists() || !source.is_dir() {
        return Err(FixerError::Replacement(format!(
            "Source folder to replace with does not exist: {}",
            source.display()
        )));
    }

    let parent = target.parent().ok_or_else(|| {
        FixerError::Replacement(format!(
            "Target folder has no parent directory: {}",
            target.display()
        ))
    })?;

    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    let folder_name = target.file_name().and_then(|n| n.to_str()).unwrap_or("mod");

    // Create backup if requested
    if backup && target.exists() {
        let backup_path = parent.join(format!("{folder_name}.bak"));
        if backup_path.exists() {
            let _ = fs::remove_dir_all(&backup_path);
        }
        copy_dir_all(target, &backup_path)?;
    }

    // Temporary staging directory in the same parent (guarantees same volume)
    let temp_name = format!(".{folder_name}.tmp-{}", Uuid::new_v4());
    let temp_path = parent.join(temp_name);

    // Copy source contents to temporary staging path
    copy_dir_all(source, &temp_path)?;

    // If target doesn't exist yet, simple rename
    if !target.exists() {
        fs::rename(&temp_path, target)?;
        return Ok(());
    }

    // Target exists: safe swap
    let replaced_name = format!(".{folder_name}.replaced-{}", Uuid::new_v4());
    let replaced_path = parent.join(replaced_name);

    // Step 1: Move target to replaced_path
    if let Err(e) = fs::rename(target, &replaced_path) {
        let _ = fs::remove_dir_all(&temp_path);
        return Err(FixerError::Replacement(format!(
            "Failed to move existing target folder out of the way {}: {}",
            target.display(),
            e
        )));
    }

    // Step 2: Move temp_path to target
    if let Err(e) = fs::rename(&temp_path, target) {
        // Rollback: try to restore original
        let _ = fs::rename(&replaced_path, target);
        let _ = fs::remove_dir_all(&temp_path);
        return Err(FixerError::Replacement(format!(
            "Failed to put repaired folder into place {}: {}",
            target.display(),
            e
        )));
    }

    // Step 3: Remove old replaced directory
    let _ = fs::remove_dir_all(&replaced_path);

    Ok(())
}
