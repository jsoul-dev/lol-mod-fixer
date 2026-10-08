//! Directory scanning and mod file discovery.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::FixerResult;
use crate::formats::ModFormat;

/// Resolve the default directory to scan.
///
/// Priority: directory containing the running executable.
/// Fallback: current working directory.
pub fn resolve_default_dir() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(parent) = exe_path.parent()
    {
        return parent.to_path_buf();
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Check if a file should be treated as a candidate mod file.
pub fn is_candidate_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    // Ignore temporary, backup, and replaced files
    if file_name.starts_with('.')
        || file_name.ends_with(".bak")
        || file_name.ends_with(".tmp")
        || file_name.ends_with(".repacked")
        || file_name.ends_with(".replaced")
    {
        return false;
    }

    // Ignore typical non-mod file extensions
    let non_mod_exts = [
        "exe", "dll", "txt", "json", "log", "png", "jpg", "jpeg", "webp", "ini", "toml", "yml",
        "yaml", "md",
    ];
    if let Some(ext) = path.extension().and_then(|s| s.to_str())
        && non_mod_exts.contains(&ext.to_ascii_lowercase().as_str())
    {
        return false;
    }

    // Check if it matches known format
    let format = ModFormat::detect(path);
    format != ModFormat::Unsupported
}

/// Scan a directory for mod archives.
pub fn scan_directory(dir: &Path, recursive: bool) -> FixerResult<Vec<PathBuf>> {
    let mut candidates = Vec::new();

    if recursive {
        for entry in WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if is_candidate_file(path) {
                candidates.push(path.to_path_buf());
            }
        }
    } else {
        if let Ok(read_dir) = fs_err::read_dir(dir) {
            for entry in read_dir.filter_map(Result::ok) {
                let path = entry.path();
                if is_candidate_file(&path) {
                    candidates.push(path);
                }
            }
        }
    }

    candidates.sort();
    Ok(candidates)
}
