//! Directory scanning and mod file discovery.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::FixerResult;
use crate::formats::{ModFormat, is_fantome_folder};

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

    // Only allow files with recognized League mod extensions
    let is_mod_extension = file_name.ends_with(".fantome")
        || file_name.ends_with(".modpkg")
        || file_name.ends_with(".wad.client")
        || file_name.ends_with(".wad")
        || file_name.ends_with(".zip");

    if !is_mod_extension {
        return false;
    }

    // Check if it matches known format (and for .zip, validates internal structure)
    let format = ModFormat::detect(path);
    format != ModFormat::Unsupported
}

/// Scan a directory for mod archives and extracted Fantome mod folders.
pub fn scan_directory(dir: &Path, recursive: bool) -> FixerResult<Vec<PathBuf>> {
    let mut candidates = Vec::new();
    let mut restricted_count = 0;

    // Only attempt to unlock the root directory if we encounter access restriction on it
    if crate::permissions::is_elevated() {
        let is_restricted = match fs_err::read_dir(dir) {
            Ok(_) => false,
            Err(e) => {
                let err = e.to_string();
                err.contains("os error 5") || err.contains("Access is denied")
            }
        };
        if is_restricted {
            crate::permissions::unlock_folder_permissions(dir);
        }
    }

    if recursive {
        let mut it = WalkDir::new(dir).follow_links(false).into_iter();
        while let Some(entry_res) = it.next() {
            let entry = match entry_res {
                Ok(e) => e,
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("os error 5") || err_msg.contains("Access is denied") {
                        restricted_count += 1;
                        if crate::permissions::is_elevated() {
                            if let Some(err_path) = e.path() {
                                crate::permissions::unlock_folder_permissions(err_path);
                            }
                        }
                        tracing::debug!("Restricted entry encountered: {e}");
                    } else {
                        tracing::debug!("Skipping unreadable entry in {}: {e}", dir.display());
                    }
                    continue;
                }
            };
            let path = entry.path();
            if path == dir {
                continue;
            }
            if path.is_dir() {
                let dir_name = entry.file_name().to_string_lossy();
                if dir_name.starts_with('.')
                    || dir_name.eq_ignore_ascii_case("hematite-fixed")
                    || dir_name.eq_ignore_ascii_case("backup")
                {
                    it.skip_current_dir();
                    continue;
                }
                if is_fantome_folder(path) {
                    candidates.push(path.to_path_buf());
                    it.skip_current_dir(); // Don't descend into META/WAD of this mod!
                }
            } else if path.is_file() && is_candidate_file(path) {
                candidates.push(path.to_path_buf());
            }
        }
    } else if let Ok(read_dir) = fs_err::read_dir(dir) {
        for entry in read_dir.filter_map(Result::ok) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || name.eq_ignore_ascii_case("hematite-fixed")
                || name.eq_ignore_ascii_case("backup")
            {
                continue;
            }
            if (path.is_dir() && is_fantome_folder(&path))
                || (path.is_file() && is_candidate_file(&path))
            {
                candidates.push(path);
            }
        }
    }

    if restricted_count > 0 && !crate::permissions::is_elevated() {
        use crossterm::style::Stylize;
        eprintln!(
            "{} {} folder(s) were inaccessible due to Windows/Rose permissions.",
            "[!] Notice:".yellow().bold(),
            restricted_count.to_string().white().bold()
        );
        eprintln!("    To automatically unlock and repair all folders, run as Administrator:");
        eprintln!("    -> Right-click lol-mod-fixer.exe and select 'Run as administrator', OR");
        eprintln!("    -> Run with: lol-mod-fixer.exe --elevate\n");
    }

    candidates.sort();
    Ok(candidates)
}
