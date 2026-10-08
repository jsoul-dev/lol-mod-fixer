//! Formatted console output and JSON serialization.

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
        println!("League Mod Fixer (LTK Repair Engine)");
        println!("=====================================");
        if let Some(d) = dir {
            println!("Directory: {}", d.display());
        }
        println!();
    }

    pub fn print_check_item(&self, idx: usize, total: usize, report: &HealthReport) {
        if self.json {
            return;
        }
        println!("[{}/{}] {}", idx + 1, total, report.file_name);
        println!("      Format: {}", report.format.display_name());
        println!("      Status: {}", report.status.display_name());

        match report.status {
            ModHealthStatus::Healthy => {
                println!("      No fixes needed.");
            }
            ModHealthStatus::Repairable => {
                println!(
                    "      Problems found: {} ({} repairable)",
                    report.problem_count, report.repairable_count
                );
                for p in &report.problems {
                    if self.verbose || p.repairable {
                        let tag = if p.repairable {
                            "[REPAIRABLE]"
                        } else {
                            "[UNREPAIRABLE]"
                        };
                        println!("        {} {} ({})", tag, p.description, p.rule_id);
                    }
                }
            }
            ModHealthStatus::Unrepairable => {
                if let Some(reason) = &report.reason {
                    println!("      Reason: {reason}");
                } else {
                    println!(
                        "      Problems found: {} (none can be auto-repaired)",
                        report.problem_count
                    );
                    for p in &report.problems {
                        println!("        [UNREPAIRABLE] {} ({})", p.description, p.rule_id);
                    }
                }
                println!("      File left untouched.");
            }
            ModHealthStatus::Broken => {
                let err = report.reason.as_deref().unwrap_or("Archive corrupted");
                println!("      Reason: {err}");
                println!("      File left untouched.");
            }
            ModHealthStatus::Unsupported => {
                let reason = report.reason.as_deref().unwrap_or("Unsupported format");
                println!("      Reason: {reason}");
                println!("      File left untouched.");
            }
            ModHealthStatus::Unavailable => {
                println!("      Hashtable or game data is missing.");
                println!("      File left untouched.");
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
                println!("[{}/{}] {}", idx + 1, total, name);
                println!("      Format: {}", format.display_name());
                println!("      Status: HEALTHY / UNCHANGED");
                println!("      No fixes needed. File left untouched.");
            }
            RepairResult::Repaired {
                path,
                format,
                fixes_applied,
                output_path,
                verified,
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!("[{}/{}] {}", idx + 1, total, name);
                println!("      Format: {}", format.display_name());
                println!("      Status: REPAIRABLE");
                println!("      Applying fixes... Fixed: {fixes_applied}");
                print!("      Verifying... ");
                if *verified {
                    println!("SUCCESS (Clean / Verified)");
                } else {
                    println!("Repaired with remaining warnings");
                }
                if path == output_path {
                    println!("      Replaced original safely.");
                } else {
                    println!("      Saved to: {}", output_path.display());
                }
            }
            RepairResult::Unrepairable {
                path,
                format,
                reason,
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!("[{}/{}] {}", idx + 1, total, name);
                println!("      Format: {}", format.display_name());
                println!("      Status: UNREPAIRABLE");
                println!("      Reason: {reason}");
                println!("      File left untouched.");
            }
            RepairResult::Unsupported {
                path,
                format,
                reason,
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!("[{}/{}] {}", idx + 1, total, name);
                println!("      Format: {}", format.display_name());
                println!("      Status: UNSUPPORTED");
                println!("      Reason: {reason}");
                println!("      File left untouched.");
            }
            RepairResult::Failed {
                path,
                format,
                error,
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!("[{}/{}] {}", idx + 1, total, name);
                println!("      Format: {}", format.display_name());
                println!("      Status: REPAIR FAILED");
                println!("      Reason: {error}");
                println!("      File left untouched.");
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

        println!("--------------------------------");
        println!("Check complete\n");
        println!("Mods scanned:       {scanned}");
        println!("Healthy:            {healthy}");
        println!("Repairable:         {repairable}");
        println!("Unrepairable:       {unrepairable}");
        println!("Broken:             {broken}");
        println!("Unsupported:        {unsupported}");
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
                    },
                    RepairResult::Unrepairable {
                        path,
                        format,
                        reason,
                    } => JsonRepairModItem {
                        path: path.display().to_string(),
                        format: *format,
                        status: "unrepairable".to_string(),
                        modified: false,
                        output_path: None,
                        fixes_applied: None,
                        reason: Some(reason.clone()),
                        error: None,
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
                    },
                    RepairResult::Failed {
                        path,
                        format,
                        error,
                    } => JsonRepairModItem {
                        path: path.display().to_string(),
                        format: *format,
                        status: "failed".to_string(),
                        modified: false,
                        output_path: None,
                        fixes_applied: None,
                        reason: None,
                        error: Some(error.clone()),
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

        println!("--------------------------------");
        println!("Scan & Repair complete\n");
        println!("Mods scanned:       {scanned}");
        println!("Healthy:            {healthy}");
        println!("Repaired:           {repaired}");
        println!("Unrepairable:       {unrepairable}");
        println!("Failed:             {failed}");
        println!("Unsupported:        {unsupported}");
    }

    /// Prompt user to press enter when appropriate.
    pub fn maybe_pause(&self, pause: bool) {
        if self.json || !pause {
            return;
        }

        print!("\nPress Enter to exit...");
        let _ = std::io::stdout().flush();
        let mut buf = String::new();
        let _ = stdin().read_line(&mut buf);
    }
}
