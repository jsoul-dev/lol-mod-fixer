//! Mod health and problem inspection using LTK Manager's Problems engine.

use ltk_manager_assets::hashtables::WadPathResolverState;
use ltk_manager_base::budget::Budget;
use ltk_manager_base::config::Config as LtkConfig;
use ltk_manager_problems::{self as problems, ProblemSeverity};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::FixerResult;
use crate::formats::ModFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModHealthStatus {
    /// No errors or warnings detected (or only Info-level notices).
    Healthy,
    /// Has issues that LTK Manager's repair engine can fix.
    Repairable,
    /// Has issues, but none can be repaired automatically.
    Unrepairable,
    /// Archive is broken / corrupted / unreadable.
    Broken,
    /// File format is not supported for inspection/repair.
    Unsupported,
    /// Cannot determine health because required hashtables/game data are missing.
    Unavailable,
}

impl ModHealthStatus {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Healthy => "HEALTHY",
            Self::Repairable => "REPAIRABLE",
            Self::Unrepairable => "UNREPAIRABLE",
            Self::Broken => "BROKEN",
            Self::Unsupported => "UNSUPPORTED",
            Self::Unavailable => "DATA UNAVAILABLE",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemItem {
    pub rule_id: String,
    pub severity: String,
    pub description: String,
    pub file: String,
    pub repairable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub path: PathBuf,
    pub file_name: String,
    pub format: ModFormat,
    pub status: ModHealthStatus,
    pub problem_count: usize,
    pub repairable_count: usize,
    pub unrepairable_count: usize,
    pub problems: Vec<ProblemItem>,
    pub reason: Option<String>,
}

/// Inspect a mod archive without modifying anything.
pub fn check_mod_health(path: &Path, config: &LtkConfig) -> FixerResult<HealthReport> {
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    if !path.exists() {
        return Ok(HealthReport {
            path: path.to_path_buf(),
            file_name,
            format: ModFormat::Unsupported,
            status: ModHealthStatus::Broken,
            problem_count: 0,
            repairable_count: 0,
            unrepairable_count: 0,
            problems: Vec::new(),
            reason: Some(format!("File does not exist: {}", path.display())),
        });
    }

    let format = ModFormat::detect(path);

    match format {
        ModFormat::Fantome => check_fantome_health(path, file_name, config),
        ModFormat::FantomeFolder => check_fantome_folder_health(path, file_name, config),
        ModFormat::Modpkg => Ok(HealthReport {
            path: path.to_path_buf(),
            file_name,
            format: ModFormat::Modpkg,
            status: ModHealthStatus::Unrepairable,
            problem_count: 0,
            repairable_count: 0,
            unrepairable_count: 0,
            problems: Vec::new(),
            reason: Some(
                "A .modpkg is read straight out of its archive and has no unpacked form. \
                 LTK Manager does not support modifying or repairing .modpkg archives."
                    .to_string(),
            ),
        }),
        ModFormat::WadClient => Ok(HealthReport {
            path: path.to_path_buf(),
            file_name,
            format: ModFormat::WadClient,
            status: ModHealthStatus::Unsupported,
            problem_count: 0,
            repairable_count: 0,
            unrepairable_count: 0,
            problems: Vec::new(),
            reason: Some(
                "The input is a packed .wad.client archive. Standalone WADs contain only \
                 xxHash64 hashes in their table of contents and cannot recover author-made \
                 file paths without mod project metadata. LTK Manager only repairs supported \
                 mod archives (.fantome)."
                    .to_string(),
            ),
        }),
        ModFormat::Unsupported => Ok(HealthReport {
            path: path.to_path_buf(),
            file_name,
            format: ModFormat::Unsupported,
            status: ModHealthStatus::Unsupported,
            problem_count: 0,
            repairable_count: 0,
            unrepairable_count: 0,
            problems: Vec::new(),
            reason: Some(
                "File format is not a supported League of Legends mod archive.".to_string(),
            ),
        }),
    }
}

fn check_fantome_health(
    path: &Path,
    file_name: String,
    config: &LtkConfig,
) -> FixerResult<HealthReport> {
    // Resolver for WAD chunks
    let resolver_state = WadPathResolverState::default();
    let resolver = resolver_state.get();

    let game_content = None; // GameContent is optional; rules work without installed game where possible

    let run_res = problems::analyze_archive(
        path,
        config,
        Budget::repair(),
        resolver.as_ref(),
        game_content,
    );

    let run = match run_res {
        Ok(run) => run,
        Err(e) => {
            return Ok(HealthReport {
                path: path.to_path_buf(),
                file_name,
                format: ModFormat::Fantome,
                status: ModHealthStatus::Broken,
                problem_count: 0,
                repairable_count: 0,
                unrepairable_count: 0,
                problems: Vec::new(),
                reason: Some(format!("Failed to analyze archive: {e}")),
            });
        }
    };

    let mut problems_list = Vec::new();
    let mut repairable_count: usize = 0;
    let mut wrong_count: usize = 0;

    for problem in run.live_problems() {
        let is_repairable = problem.fix.is_some();
        if is_repairable {
            repairable_count += 1;
        }

        // Info severity does not count as "broken"
        if !matches!(problem.severity, ProblemSeverity::Info) {
            wrong_count += 1;
        }

        let severity_str = match problem.severity {
            ProblemSeverity::Fatal => "Fatal",
            ProblemSeverity::Error => "Error",
            ProblemSeverity::Warning => "Warning",
            ProblemSeverity::Info => "Info",
        };

        let file_loc = format!("{}:{}", problem.site.layer, problem.site.path);

        let rule_desc = run
            .rules
            .iter()
            .find(|r| r.id == problem.rule)
            .map(|r| r.description.clone())
            .unwrap_or_else(|| problem.rule.to_string());
        let description = problem.message.clone().unwrap_or(rule_desc);

        problems_list.push(ProblemItem {
            rule_id: problem.rule.to_string(),
            severity: severity_str.to_string(),
            description,
            file: file_loc,
            repairable: is_repairable,
        });
    }

    let unrepairable_count = wrong_count.saturating_sub(repairable_count);
    let total_problems = problems_list.len();

    let status = if wrong_count == 0 {
        ModHealthStatus::Healthy
    } else if repairable_count > 0 {
        ModHealthStatus::Repairable
    } else {
        ModHealthStatus::Unrepairable
    };

    Ok(HealthReport {
        path: path.to_path_buf(),
        file_name,
        format: ModFormat::Fantome,
        status,
        problem_count: total_problems,
        repairable_count,
        unrepairable_count,
        problems: problems_list,
        reason: None,
    })
}

fn check_fantome_folder_health(
    path: &Path,
    file_name: String,
    config: &LtkConfig,
) -> FixerResult<HealthReport> {
    let temp = tempfile::TempDir::new()?;
    let temp_archive = temp.path().join("archive.fantome");
    if let Err(e) = crate::formats::pack_fantome_folder(path, &temp_archive) {
        return Ok(HealthReport {
            path: path.to_path_buf(),
            file_name,
            format: ModFormat::FantomeFolder,
            status: ModHealthStatus::Broken,
            problem_count: 0,
            repairable_count: 0,
            unrepairable_count: 0,
            problems: Vec::new(),
            reason: Some(format!("Failed to read extracted mod folder: {e}")),
        });
    }

    let mut report = check_fantome_health(&temp_archive, file_name, config)?;
    report.path = path.to_path_buf();
    report.format = ModFormat::FantomeFolder;
    Ok(report)
}
