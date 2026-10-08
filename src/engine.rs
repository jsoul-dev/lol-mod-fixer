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
        let report = check_mod_health(path, &config)?;
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
    printer: &Printer,
) -> FixerResult<i32> {
    let (config, _) = resolve_ltk_config(cli_league, None);
    let input_path = input.unwrap_or_else(resolve_default_dir);

    if input_path.is_file() || crate::formats::is_fantome_folder(&input_path) {
        // Single file or extracted mod folder repair
        printer.print_banner(None);
        let res = repair_mod_archive(&input_path, output.as_deref(), &config, backup, dry_run)?;
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

    let mut candidates = scan_directory(&input_path, recursive)?;
    if candidates.is_empty() && !recursive {
        let sub_candidates = scan_directory(&input_path, true)?;
        if !sub_candidates.is_empty() {
            candidates = sub_candidates;
        }
    }

    let total = candidates.len();
    if total == 0 {
        if !printer.json {
            println!("No mod archives (.fantome, .modpkg, .wad.client) found.");
        }
        printer.print_repair_summary(&[], Some(&input_path));
        return Ok(exit_codes::SUCCESS);
    }

    let mut results = Vec::with_capacity(total);
    for (idx, path) in candidates.iter().enumerate() {
        let res = repair_mod_archive(path, None, &config, backup, dry_run)?;
        printer.print_repair_item(idx, total, &res);
        results.push(res);
    }

    printer.print_repair_summary(&results, Some(&input_path));

    let has_failures = results.iter().any(|r| r.is_failed() || r.is_unrepairable());
    if has_failures {
        Ok(exit_codes::REPAIRABLE_OR_FAILED)
    } else {
        Ok(exit_codes::SUCCESS)
    }
}
