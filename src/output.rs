use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use std::io::{Write, stdin};
use std::path::Path;

use crate::formats::ModFormat;
use crate::health::{HealthReport, ModHealthStatus};
use crate::repair::RepairResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonCheckSummary {
    pub scanned: usize,
    pub healthy: usize,
    pub repairable: usize,
    pub unrepairable: usize,
    pub broken: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRepairSummary {
    pub scanned: usize,
    pub healthy: usize,
    pub repaired: usize,
    pub unrepairable: usize,
    pub failed: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonCheckOutput {
    pub mode: &'static str,
    pub directory: Option<String>,
    pub summary: JsonCheckSummary,
    pub mods: Vec<HealthReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRepairModItem {
    pub path: String,
    pub format: ModFormat,
    pub status: String,
    pub modified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixes_applied: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarantined: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRepairOutput {
    pub mode: &'static str,
    pub directory: Option<String>,
    pub summary: JsonRepairSummary,
    pub mods: Vec<JsonRepairModItem>,
}

pub struct Printer {
    pub json: bool,
    pub verbose: bool,
}

impl Printer {
    pub fn new(json: bool, verbose: bool) -> Self {
        Self { json, verbose }
    }

    pub fn print_banner(&self, dir: Option<&Path>) {
        if self.json {
            return;
        }
        println!("{}", "League Mod Fixer (LTK Repair Engine)".cyan().bold());
        println!("{}", "====================================".dark_cyan());
        if let Some(d) = dir {
            println!("Directory: {}", d.display().to_string().white().bold());
        }

        let running_procs = crate::process::detect_running_conflicting_processes();
        if !running_procs.is_empty() {
            println!();
            println!(
                "{} {}",
                "[!] Warning:".yellow().bold(),
                format!(
                    "Detected running game process(es): {}",
                    running_procs.join(", ")
                )
                .white()
                .bold()
            );
            println!(
                "    {}",
                "Please close League of Legends to avoid file lock conflicts."
                    .yellow()
            );
        }

        println!();
    }

    pub fn print_check_item(&self, idx: usize, total: usize, report: &HealthReport) {
        if self.json {
            return;
        }
        println!(
            "{} {}",
            format!("[{}/{}]", idx + 1, total).dark_grey(),
            report.file_name.as_str().white().bold()
        );
        println!("      Format: {}", report.format.display_name().dark_grey());
        let status_text = match report.status {
            ModHealthStatus::Healthy => "HEALTHY".green().bold(),
            ModHealthStatus::Repairable => "REPAIRABLE".yellow().bold(),
            ModHealthStatus::Unrepairable => "UNREPAIRABLE".dark_red().bold(),
            ModHealthStatus::Broken => "BROKEN".red().bold(),
            ModHealthStatus::Unsupported => "UNSUPPORTED".magenta().bold(),
            ModHealthStatus::Unavailable => "UNAVAILABLE".dark_grey().bold(),
        };
        println!("      Status: {status_text}");

        match report.status {
            ModHealthStatus::Healthy => {
                println!("      {}", "No fixes needed.".dark_grey());
            }
            ModHealthStatus::Repairable => {
                println!(
                    "      Problems found: {} ({} repairable)",
                    report.problem_count.to_string().yellow().bold(),
                    report.repairable_count.to_string().cyan().bold()
                );
                for p in &report.problems {
                    if self.verbose || p.repairable {
                        let tag = if p.repairable {
                            "[REPAIRABLE]".yellow().bold()
                        } else {
                            "[UNREPAIRABLE]".red().bold()
                        };
                        println!("        {} {} ({})", tag, p.description, p.rule_id.as_str().dark_grey());
                    }
                }
            }
            ModHealthStatus::Unrepairable => {
                if let Some(reason) = &report.reason {
                    println!("      Reason: {}", reason.as_str().yellow());
                } else {
                    println!(
                        "      Problems found: {} (none can be auto-repaired)",
                        report.problem_count.to_string().red().bold()
                    );
                    for p in &report.problems {
                        println!(
                            "        {} {} ({})",
                            "[UNREPAIRABLE]".red().bold(),
                            p.description,
                            p.rule_id.as_str().dark_grey()
                        );
                    }
                }
                println!("      {}", "File left untouched.".dark_grey());
            }
            ModHealthStatus::Broken => {
                let err = report.reason.as_deref().unwrap_or("Archive corrupted");
                println!("      Reason: {}", err.red());
                println!("      {}", "File left untouched.".dark_grey());
            }
            ModHealthStatus::Unsupported => {
                let reason = report.reason.as_deref().unwrap_or("Unsupported format");
                println!("      Reason: {}", reason.magenta());
                println!("      {}", "File left untouched.".dark_grey());
            }
            ModHealthStatus::Unavailable => {
                println!("      {}", "Hashtable or game data is missing.".yellow());
                println!("      {}", "File left untouched.".dark_grey());
            }
        }
        println!();
    }

    pub fn print_repair_item(&self, idx: usize, total: usize, res: &RepairResult) {
        if self.json {
            return;
        }

        match res {
            RepairResult::Unchanged { path, format } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!(
                    "{} {}",
                    format!("[{}/{}]", idx + 1, total).dark_grey(),
                    name.white().bold()
                );
                println!("      Format: {}", format.display_name().dark_grey());
                println!("      Status: {}", "HEALTHY / UNCHANGED".green().bold());
                println!("      {}", "No fixes needed. File left untouched.".dark_grey());
            }
            RepairResult::Repaired {
                path,
                format,
                fixes_applied,
                output_path,
                verified,
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!(
                    "{} {}",
                    format!("[{}/{}]", idx + 1, total).dark_grey(),
                    name.white().bold()
                );
                println!("      Format: {}", format.display_name().dark_grey());
                println!("      Status: {}", "REPAIRED".green().bold());
                println!(
                    "      Applying fixes... Fixed: {}",
                    fixes_applied.to_string().cyan().bold()
                );
                print!("      Verifying... ");
                if *verified {
                    println!("{}", "SUCCESS (Clean / Verified)".green().bold());
                } else {
                    println!("{}", "Repaired with remaining warnings".yellow().bold());
                }
                if path == output_path {
                    println!("      {}", "Replaced original safely.".green());
                } else {
                    println!("      Saved to: {}", output_path.display().to_string().cyan());
                }
            }
            RepairResult::Unrepairable {
                path,
                format,
                reason,
                quarantined,
                deleted,
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!(
                    "{} {}",
                    format!("[{}/{}]", idx + 1, total).dark_grey(),
                    name.white().bold()
                );
                println!("      Format: {}", format.display_name().dark_grey());
                if let Some(q_path) = quarantined {
                    println!("      Status: {}", "UNREPAIRABLE (QUARANTINED)".yellow().bold());
                    println!("      Reason: {}", reason.as_str().yellow());
                    println!("      Quarantined: {}", format!("Moved to {}", q_path.display()).cyan());
                } else if *deleted {
                    println!("      Status: {}", "UNREPAIRABLE (CLEANED)".red().bold());
                    println!("      Reason: {}", reason.as_str().yellow());
                    println!("      {}", "Deleted unrepairable mod to prevent game crashes.".red());
                } else {
                    println!("      Status: {}", "UNREPAIRABLE".dark_red().bold());
                    println!("      Reason: {}", reason.as_str().yellow());
                    println!("      {}", "File left untouched.".dark_grey());
                }
            }
            RepairResult::Unsupported {
                path,
                format,
                reason,
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!(
                    "{} {}",
                    format!("[{}/{}]", idx + 1, total).dark_grey(),
                    name.white().bold()
                );
                println!("      Format: {}", format.display_name().dark_grey());
                println!("      Status: {}", "UNSUPPORTED".magenta().bold());
                println!("      Reason: {}", reason.as_str().magenta());
                println!("      {}", "File left untouched.".dark_grey());
            }
            RepairResult::Failed {
                path,
                format,
                error,
                quarantined,
                deleted,
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!(
                    "{} {}",
                    format!("[{}/{}]", idx + 1, total).dark_grey(),
                    name.white().bold()
                );
                println!("      Format: {}", format.display_name().dark_grey());
                if let Some(q_path) = quarantined {
                    println!("      Status: {}", "CORRUPTED / FAILED (QUARANTINED)".red().bold());
                    println!("      Reason: {}", error.as_str().red());
                    println!("      Quarantined: {}", format!("Moved to {}", q_path.display()).cyan());
                } else if *deleted {
                    println!("      Status: {}", "CORRUPTED / FAILED (CLEANED)".red().bold());
                    println!("      Reason: {}", error.as_str().red());
                    println!("      {}", "Deleted broken/corrupted file to prevent game crashes.".red());
                } else {
                    println!("      Status: {}", "REPAIR FAILED".red().bold());
                    println!("      Reason: {}", error.as_str().red());
                    println!("      {}", "File left untouched.".dark_grey());
                }
            }
        }
        println!();
    }

    pub fn print_check_summary(&self, reports: &[HealthReport], dir: Option<&Path>) {
        let scanned = reports.len();
        let healthy = reports
            .iter()
            .filter(|r| r.status == ModHealthStatus::Healthy)
            .count();
        let repairable = reports
            .iter()
            .filter(|r| r.status == ModHealthStatus::Repairable)
            .count();
        let unrepairable = reports
            .iter()
            .filter(|r| r.status == ModHealthStatus::Unrepairable)
            .count();
        let broken = reports
            .iter()
            .filter(|r| r.status == ModHealthStatus::Broken)
            .count();
        let unsupported = reports
            .iter()
            .filter(|r| r.status == ModHealthStatus::Unsupported)
            .count();

        if self.json {
            let output = JsonCheckOutput {
                mode: "check",
                directory: dir.map(|d| d.display().to_string()),
                summary: JsonCheckSummary {
                    scanned,
                    healthy,
                    repairable,
                    unrepairable,
                    broken,
                    unsupported,
                },
                mods: reports.to_vec(),
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output).unwrap_or_default()
            );
            return;
        }

        println!("{}", "--------------------------------".dark_cyan());
        println!("{}", "Check complete\n".white().bold());
        println!("Mods scanned:       {}", scanned.to_string().white().bold());
        println!(
            "Healthy:            {}",
            if healthy > 0 {
                healthy.to_string().green().bold()
            } else {
                healthy.to_string().dark_grey()
            }
        );
        println!(
            "Repairable:         {}",
            if repairable > 0 {
                repairable.to_string().yellow().bold()
            } else {
                repairable.to_string().dark_grey()
            }
        );
        println!(
            "Unrepairable:       {}",
            if unrepairable > 0 {
                unrepairable.to_string().dark_red().bold()
            } else {
                unrepairable.to_string().dark_grey()
            }
        );
        println!(
            "Broken:             {}",
            if broken > 0 {
                broken.to_string().red().bold()
            } else {
                broken.to_string().dark_grey()
            }
        );
        println!(
            "Unsupported:        {}",
            if unsupported > 0 {
                unsupported.to_string().magenta().bold()
            } else {
                unsupported.to_string().dark_grey()
            }
        );
    }

    pub fn print_repair_summary(&self, results: &[RepairResult], dir: Option<&Path>) {
        let scanned = results.len();
        let healthy = results.iter().filter(|r| r.is_unchanged()).count();
        let repaired = results.iter().filter(|r| r.is_repaired()).count();
        let unrepairable = results.iter().filter(|r| r.is_unrepairable()).count();
        let failed = results.iter().filter(|r| r.is_failed()).count();
        let unsupported = results.iter().filter(|r| r.is_unsupported()).count();

        if self.json {
            let mod_items = results
                .iter()
                .map(|r| match r {
                    RepairResult::Unchanged { path, format } => JsonRepairModItem {
                        path: path.display().to_string(),
                        format: *format,
                        status: "healthy".to_string(),
                        modified: false,
                        output_path: None,
                        fixes_applied: Some(0),
                        reason: None,
                        error: None,
                        quarantined: None,
                        deleted: None,
                    },
                    RepairResult::Repaired {
                        path,
                        format,
                        fixes_applied,
                        output_path,
                        verified: _,
                    } => JsonRepairModItem {
                        path: path.display().to_string(),
                        format: *format,
                        status: "repaired".to_string(),
                        modified: true,
                        output_path: Some(output_path.display().to_string()),
                        fixes_applied: Some(*fixes_applied),
                        reason: None,
                        error: None,
                        quarantined: None,
                        deleted: None,
                    },
                    RepairResult::Unrepairable {
                        path,
                        format,
                        reason,
                        quarantined,
                        deleted,
                    } => JsonRepairModItem {
                        path: path.display().to_string(),
                        format: *format,
                        status: "unrepairable".to_string(),
                        modified: *deleted || quarantined.is_some(),
                        output_path: None,
                        fixes_applied: None,
                        reason: Some(reason.clone()),
                        error: None,
                        quarantined: quarantined.as_ref().map(|p| p.display().to_string()),
                        deleted: if *deleted { Some(true) } else { None },
                    },
                    RepairResult::Unsupported {
                        path,
                        format,
                        reason,
                    } => JsonRepairModItem {
                        path: path.display().to_string(),
                        format: *format,
                        status: "unsupported".to_string(),
                        modified: false,
                        output_path: None,
                        fixes_applied: None,
                        reason: Some(reason.clone()),
                        error: None,
                        quarantined: None,
                        deleted: None,
                    },
                    RepairResult::Failed {
                        path,
                        format,
                        error,
                        quarantined,
                        deleted,
                    } => JsonRepairModItem {
                        path: path.display().to_string(),
                        format: *format,
                        status: "failed".to_string(),
                        modified: *deleted || quarantined.is_some(),
                        output_path: None,
                        fixes_applied: None,
                        reason: None,
                        error: Some(error.clone()),
                        quarantined: quarantined.as_ref().map(|p| p.display().to_string()),
                        deleted: if *deleted { Some(true) } else { None },
                    },
                })
                .collect();

            let output = JsonRepairOutput {
                mode: "repair",
                directory: dir.map(|d| d.display().to_string()),
                summary: JsonRepairSummary {
                    scanned,
                    healthy,
                    repaired,
                    unrepairable,
                    failed,
                    unsupported,
                },
                mods: mod_items,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output).unwrap_or_default()
            );
            return;
        }

        let unrepairable_quarantined = results
            .iter()
            .filter(|r| match r {
                RepairResult::Unrepairable { quarantined, .. } => quarantined.is_some(),
                _ => false,
            })
            .count();
        let unrepairable_cleaned = results
            .iter()
            .filter(|r| match r {
                RepairResult::Unrepairable { deleted, .. } => *deleted,
                _ => false,
            })
            .count();
        let failed_quarantined = results
            .iter()
            .filter(|r| match r {
                RepairResult::Failed { quarantined, .. } => quarantined.is_some(),
                _ => false,
            })
            .count();
        let failed_cleaned = results
            .iter()
            .filter(|r| match r {
                RepairResult::Failed { deleted, .. } => *deleted,
                _ => false,
            })
            .count();

        println!("{}", "--------------------------------".dark_cyan());
        println!("{}", "Scan & Repair complete\n".white().bold());
        println!("Mods scanned:       {}", scanned.to_string().white().bold());
        println!(
            "Healthy:            {}",
            if healthy > 0 {
                healthy.to_string().green().bold()
            } else {
                healthy.to_string().dark_grey()
            }
        );
        println!(
            "Repaired:           {}",
            if repaired > 0 {
                repaired.to_string().cyan().bold()
            } else {
                repaired.to_string().dark_grey()
            }
        );
        println!(
            "Unrepairable:       {}{}",
            if unrepairable > 0 {
                unrepairable.to_string().dark_red().bold()
            } else {
                unrepairable.to_string().dark_grey()
            },
            if unrepairable_quarantined > 0 {
                format!(" ({} quarantined to .broken)", unrepairable_quarantined).yellow().to_string()
            } else if unrepairable_cleaned > 0 {
                format!(" ({} cleaned)", unrepairable_cleaned).red().to_string()
            } else {
                String::new()
            }
        );
        println!(
            "Failed:             {}{}",
            if failed > 0 {
                failed.to_string().red().bold()
            } else {
                failed.to_string().dark_grey()
            },
            if failed_quarantined > 0 {
                format!(" ({} quarantined to .broken)", failed_quarantined).yellow().to_string()
            } else if failed_cleaned > 0 {
                format!(" ({} cleaned)", failed_cleaned).red().to_string()
            } else {
                String::new()
            }
        );
        println!(
            "Unsupported:        {}",
            if unsupported > 0 {
                unsupported.to_string().magenta().bold()
            } else {
                unsupported.to_string().dark_grey()
            }
        );
    }

    pub fn print_migration_summary(&self, report: &crate::migrate::MigrationReport) {
        if self.json || report.is_empty() {
            return;
        }
        println!("{}", "--- Rose Legacy Structure Migration ---".dark_grey());
        if report.extracted_archives > 0 {
            println!(
                "  {} Migrated and extracted {} archive(s) into modern Rose folders",
                "✓".green().bold(),
                report.extracted_archives.to_string().yellow().bold()
            );
        }
        if report.manifests_rebuilt > 0 {
            println!(
                "  {} Built/synchronized {} rose_mod_targets.json manifest(s)",
                "✓".green().bold(),
                report.manifests_rebuilt.to_string().cyan().bold()
            );
        }
        if report.skipped_archives > 0 {
            println!(
                "  {} Skipped {} archive(s) already extracted",
                "•".dark_grey(),
                report.skipped_archives.to_string().dark_grey()
            );
        }
        if !report.failed_archives.is_empty() {
            println!(
                "  {} {} archive(s) failed extraction",
                "✗".red().bold(),
                report.failed_archives.len().to_string().red().bold()
            );
            for (name, reason) in &report.failed_archives {
                println!(
                    "    {} {}: {}",
                    "•".dark_grey(),
                    name.as_str().white().bold(),
                    reason.as_str().yellow()
                );
            }
        }
        println!();
    }

    pub fn print_cleanup_summary(&self, report: &crate::cleanup::CleanupReport) {
        if self.json || report.is_empty() {
            return;
        }

        println!("{}", "--- Rose Directory Cleanup ---".dark_grey());
        if report.deleted_targets > 0 {
            println!(
                "  {} Deleted {} empty/orphan target folder(s)",
                "✓".green().bold(),
                report.deleted_targets.to_string().yellow().bold()
            );
        }
        if report.deleted_mods > 0 {
            println!(
                "  {} Deleted {} empty mod folder(s)",
                "✓".green().bold(),
                report.deleted_mods.to_string().yellow().bold()
            );
        }
        if report.deleted_hematite_fixed > 0 {
            println!(
                "  {} Deleted {} empty Hematite-Fixed folder(s)",
                "✓".green().bold(),
                report.deleted_hematite_fixed.to_string().yellow().bold()
            );
        }
        if report.deleted_empty_subdirs > 0 {
            println!(
                "  {} Deleted {} empty internal mod subfolder(s) (e.g. empty META/hashes)",
                "✓".green().bold(),
                report.deleted_empty_subdirs.to_string().yellow().bold()
            );
        }
        println!();
    }


    pub fn print_mapping_summary(&self, summary: &crate::mapping::MappingSummary) {
        if self.json {
            return;
        }
        println!("{}", "--- Skin ID Mappings Generated ---".dark_grey());
        println!(
            "  {} Mapped {} skin folder(s) with {} installed mod(s)",
            "✓".green().bold(),
            summary.total_folders.to_string().cyan().bold(),
            summary.total_mods.to_string().cyan().bold()
        );
        println!(
            "  {} Text file : {}",
            "•".dark_grey(),
            summary.txt_path.file_name().unwrap_or_default().to_string_lossy().white()
        );
        println!(
            "  {} JSON file : {}",
            "•".dark_grey(),
            summary.json_path.file_name().unwrap_or_default().to_string_lossy().white()
        );
        println!();
    }

    /// Prompt user to press enter when appropriate.
    pub fn maybe_pause(&self, pause: bool) {

        if self.json || !pause {
            return;
        }

        print!("{}", "\nPress Enter to exit...".cyan());
        let _ = std::io::stdout().flush();
        let mut buf = String::new();
        let _ = stdin().read_line(&mut buf);
    }
}
