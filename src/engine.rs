//! Main application workflow execution engine.

use ltk_manager_assets::hashtables::HashtableCache;
use ltk_manager_base::events::NullEventSink;
use std::path::PathBuf;

use crate::config::resolve_ltk_config;
use crate::error::{FixerError, FixerResult, exit_codes};
use crate::health::{ModHealthStatus, check_mod_health};
use crate::output::Printer;
use crate::repair::{RepairResult, repair_mod_archive};
use crate::scanner::{resolve_default_dir, scan_directory};

const USER_AGENT: &str = "LoL-Mod-Fixer/0.1 (+https://github.com/LeagueToolkit/ltk-manager)";

/// Initialize hashtable cache and sync if requested or missing.
pub fn ensure_hashtables(sync_requested: bool) -> FixerResult<Option<HashtableCache>> {
    let cache = match HashtableCache::shared() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Hashtables not available: {e}");
            return Ok(None);
        }
    };

    let needs_sync = sync_requested || cache.generation().is_none();
    if needs_sync {
        tracing::info!("Syncing hashtables from published mimir release...");
        match cache.sync(false, USER_AGENT, &NullEventSink) {
            Ok(report) => {
                if report.up_to_date {
                    tracing::info!("Hashtables are up to date.");
                } else {
                    tracing::info!("Installed tables: {:?}", report.installed);
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Hashtable sync failed: {e}. Operating with cached tables if present."
                );
            }
        }
    }

    Ok(Some(cache))
}

/// Execute Check command on a single path or directory.
pub fn execute_check(
    target: Option<PathBuf>,
    cli_league: Option<PathBuf>,
    recursive: bool,
    beautify: bool,
    printer: &Printer,
) -> FixerResult<i32> {
    let (config, _) = resolve_ltk_config(cli_league, None);
    let target_path = target.unwrap_or_else(resolve_default_dir);

    let (candidates, is_dir) =
        if target_path.is_file() || crate::formats::is_fantome_folder(&target_path) {
            printer.print_banner(None);
            (vec![target_path.clone()], false)
        } else {
            printer.print_banner(Some(&target_path));
            let mut list = scan_directory(&target_path, recursive)?;
            if list.is_empty() && !recursive {
                let sub_list = scan_directory(&target_path, true)?;
                if !sub_list.is_empty() {
                    list = sub_list;
                }
            }
            (list, true)
        };

    let mut reports = Vec::with_capacity(candidates.len());
    let total = candidates.len();

    if total == 0 {
        if !printer.json {
            println!("No mod archives (.fantome, .modpkg, .wad.client) found.");
        }
        printer.print_check_summary(&[], if is_dir { Some(&target_path) } else { None });
        return Ok(exit_codes::SUCCESS);
    }

    for (idx, path) in candidates.iter().enumerate() {
        let active_path = if beautify && crate::formats::is_fantome_folder(path) {
            match crate::beautify::beautify_and_sync_folder(path) {
                Ok(Some(new_p)) => new_p,
                _ => path.clone(),
            }
        } else {
            path.clone()
        };
        let report = check_mod_health(&active_path, &config)?;
        printer.print_check_item(idx, total, &report);
        reports.push(report);
    }

    printer.print_check_summary(&reports, if is_dir { Some(&target_path) } else { None });

    let has_unrepairable = reports.iter().any(|r| {
        r.status == ModHealthStatus::Unrepairable
            || r.status == ModHealthStatus::Broken
            || r.status == ModHealthStatus::Repairable
    });

    if has_unrepairable {
        Ok(exit_codes::REPAIRABLE_OR_FAILED)
    } else {
        Ok(exit_codes::SUCCESS)
    }
}

/// Execute Repair command on a target or directory.
pub fn execute_repair(
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    cli_league: Option<PathBuf>,
    backup: bool,
    dry_run: bool,
    recursive: bool,
    beautify: bool,
    cleanup: bool,
    mapping: bool,
    printer: &Printer,
) -> FixerResult<i32> {
    let (config, _) = resolve_ltk_config(cli_league, None);
    let input_path = input.unwrap_or_else(resolve_default_dir);

    if input_path.is_file() || crate::formats::is_fantome_folder(&input_path) {
        // Single file or extracted mod folder repair
        printer.print_banner(None);
        let effective_input = if beautify && crate::formats::is_fantome_folder(&input_path) {
            match crate::beautify::beautify_and_sync_folder(&input_path) {
                Ok(Some(new_p)) => new_p,
                _ => input_path.clone(),
            }
        } else {
            input_path.clone()
        };
        let res = repair_mod_archive(&effective_input, output.as_deref(), &config, backup, dry_run)?;
        printer.print_repair_item(0, 1, &res);
        printer.print_repair_summary(std::slice::from_ref(&res), None);

        return match res {
            RepairResult::Repaired { .. } | RepairResult::Unchanged { .. } => {
                Ok(exit_codes::SUCCESS)
            }
            _ => Ok(exit_codes::REPAIRABLE_OR_FAILED),
        };
    }

    // Directory repair
    if output.is_some() {
        return Err(FixerError::Repair(
            "An explicit output file path cannot be specified when scanning a directory."
                .to_string(),
        ));
    }

    printer.print_banner(Some(&input_path));

    // 1. Initial cleanup of empty folders and orphan target manifests
    if cleanup && !dry_run {
        match crate::cleanup::cleanup_empty_rose_folders(&input_path) {
            Ok(report) => printer.print_cleanup_summary(&report),
            Err(e) => tracing::warn!("Folder cleanup notice: {e}"),
        }
    }

    let mut candidates = scan_directory(&input_path, recursive)?;
    if candidates.is_empty() && !recursive {
        let sub_candidates = scan_directory(&input_path, true)?;
        if !sub_candidates.is_empty() {
            candidates = sub_candidates;
        }
    }

    let total = candidates.len();
    let mut results = Vec::with_capacity(total);

    if total == 0 {
        if !printer.json {
            println!("No mod archives (.fantome, .modpkg, .wad.client) found.");
        }
    } else {
        for (idx, path) in candidates.iter().enumerate() {
            let active_path = if beautify && crate::formats::is_fantome_folder(path) {
                match crate::beautify::beautify_and_sync_folder(path) {
                    Ok(Some(new_p)) => new_p,
                    _ => path.clone(),
                }
            } else {
                path.clone()
            };
            let res = repair_mod_archive(&active_path, None, &config, backup, dry_run)?;
            printer.print_repair_item(idx, total, &res);
            results.push(res);
        }
    }

    // 2. Post-repair cleanup pass (in case any operation left empty folders)
    if cleanup && !dry_run {
        match crate::cleanup::cleanup_empty_rose_folders(&input_path) {
            Ok(report) => {
                if !report.is_empty() {
                    printer.print_cleanup_summary(&report);
                }
            }
            Err(e) => tracing::warn!("Post-repair folder cleanup notice: {e}"),
        }
    }

    // 3. Generate/update skin ID mappings (skin_mappings.txt and skin_mappings.json)
    if mapping && !dry_run {
        match crate::mapping::generate_skin_mappings(&input_path) {
            Ok(Some(summary)) => printer.print_mapping_summary(&summary),
            Ok(None) => {}
            Err(e) => tracing::warn!("Skin mapping generation notice: {e}"),
        }
    }

    printer.print_repair_summary(&results, Some(&input_path));

    let has_failures = results.iter().any(|r| r.is_failed() || r.is_unrepairable());
    if has_failures {
        Ok(exit_codes::REPAIRABLE_OR_FAILED)
    } else {
        Ok(exit_codes::SUCCESS)
    }
}

