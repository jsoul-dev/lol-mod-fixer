//! Mod repair engine reusing LTK Manager's ModLibrary and repair algorithms.

use fs_err as fs;
use ltk_manager_assets::hashtables::WadPathResolverState;
use ltk_manager_base::config::Config as LtkConfig;
use ltk_manager_base::events::NullEventSink;
use ltk_manager_library::mods::{
    ChecksumMismatchState, ExportScope, ExportShape, LinkedBinState, ModLibrary, WadReportState,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

use crate::error::{FixerError, FixerResult};
use crate::formats::ModFormat;
use crate::health::{ModHealthStatus, check_mod_health};
use crate::replacement::{replace_file_safely, replace_folder_safely};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum RepairResult {
    /// Mod was healthy or repairs applied 0 fixes; left untouched.
    Unchanged { path: PathBuf, format: ModFormat },
    /// Mod was repaired and verified.
    Repaired {
        path: PathBuf,
        format: ModFormat,
        fixes_applied: u32,
        output_path: PathBuf,
        verified: bool,
    },
    /// Mod has problems but none are repairable by the engine.
    Unrepairable {
        path: PathBuf,
        format: ModFormat,
        reason: String,
    },
    /// Format is not supported for repair (e.g. standalone .wad.client).
    Unsupported {
        path: PathBuf,
        format: ModFormat,
        reason: String,
    },
    /// Repair or import failed with an error; original left untouched.
    Failed {
        path: PathBuf,
        format: ModFormat,
        error: String,
    },
}

impl RepairResult {
    pub fn is_repaired(&self) -> bool {
        matches!(self, Self::Repaired { .. })
    }

    pub fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    pub fn is_unrepairable(&self) -> bool {
        matches!(self, Self::Unrepairable { .. })
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported { .. })
    }
}

/// Repair one mod archive safely.
///
/// If `output` is None, safely replaces `input` (only after successful repair and verification).
/// If `dry_run` is true, performs inspection and reports what would be done without writing anything.
pub fn repair_mod_archive(
    input: &Path,
    output: Option<&Path>,
    config: &LtkConfig,
    backup: bool,
    dry_run: bool,
) -> FixerResult<RepairResult> {
    let format = ModFormat::detect(input);

    if !format.is_repairable_format() {
        return match format {
            ModFormat::Modpkg => Ok(RepairResult::Unrepairable {
                path: input.to_path_buf(),
                format: ModFormat::Modpkg,
                reason: "A .modpkg is read straight out of its archive and has no unpacked form. \
                         LTK Manager does not support modifying or repairing .modpkg archives."
                    .to_string(),
            }),
            ModFormat::WadClient => Ok(RepairResult::Unsupported {
                path: input.to_path_buf(),
                format: ModFormat::WadClient,
                reason: "The input is a packed .wad.client archive. Standalone WADs contain only \
                         xxHash64 hashes in their table of contents and cannot recover author-made \
                         file paths without mod project metadata. Standalone .wad.client repair is \
                         not supported by LTK Manager."
                    .to_string(),
            }),
            _ => Ok(RepairResult::Unsupported {
                path: input.to_path_buf(),
                format,
                reason: "Unsupported mod format.".to_string(),
            }),
        };
    }

    // Step 1: Health inspection
    let initial_health = check_mod_health(input, config)?;
    match initial_health.status {
        ModHealthStatus::Healthy => {
            return Ok(RepairResult::Unchanged {
                path: input.to_path_buf(),
                format,
            });
        }
        ModHealthStatus::Unrepairable => {
            return Ok(RepairResult::Unrepairable {
                path: input.to_path_buf(),
                format,
                reason: initial_health.reason.unwrap_or_else(|| {
                    "Detected problems cannot be repaired automatically.".to_string()
                }),
            });
        }
        ModHealthStatus::Broken => {
            return Ok(RepairResult::Failed {
                path: input.to_path_buf(),
                format,
                error: initial_health
                    .reason
                    .unwrap_or_else(|| "Archive is corrupted or unreadable.".to_string()),
            });
        }
        ModHealthStatus::Unsupported => {
            return Ok(RepairResult::Unsupported {
                path: input.to_path_buf(),
                format,
                reason: initial_health
                    .reason
                    .unwrap_or_else(|| "Format not supported.".to_string()),
            });
        }
        ModHealthStatus::Unavailable => {
            return Ok(RepairResult::Failed {
                path: input.to_path_buf(),
                format,
                error: "Hashtable cache or game metadata unavailable.".to_string(),
            });
        }
        ModHealthStatus::Repairable => {}
    }

    let target_dest = output.unwrap_or(input);

    if dry_run {
        return Ok(RepairResult::Repaired {
            path: input.to_path_buf(),
            format,
            fixes_applied: initial_health.repairable_count as u32,
            output_path: target_dest.to_path_buf(),
            verified: false,
        });
    }

    // Step 2: Isolated temporary workspace
    let temp_dir = TempDir::new()?;
    let work_path = temp_dir.path();
    let storage = work_path.join("library");
    fs::create_dir_all(&storage)?;

    let lib_config = LtkConfig {
        league_path: config.league_path.clone(),
        mod_storage_path: Some(storage.clone()),
        ..LtkConfig::default()
    };

    let resolver_state = {
        let state = WadPathResolverState::default();
        if state.get().has_tables() {
            state
        } else {
            WadPathResolverState::preloaded(ltk_manager_assets::test_util::resolver_naming(&[
                "data/placeholder",
            ]))
        }
    };

    let library = ModLibrary::new(
        Arc::new(NullEventSink),
        Some(storage.clone()),
        "0.1.0",
        Arc::new(LinkedBinState::default()),
        Arc::new(ChecksumMismatchState::default()),
        Arc::new(WadReportState::new(Some(&storage))),
        Arc::new(resolver_state),
    );

    // Step 3: Install mod into temporary library
    let source_archive = if format == ModFormat::FantomeFolder {
        let temp_archive = work_path.join("source.fantome");
        if let Err(e) = crate::formats::pack_fantome_folder(input, &temp_archive) {
            return Ok(RepairResult::Failed {
                path: input.to_path_buf(),
                format,
                error: format!("Failed to read extracted mod folder: {e}"),
            });
        }
        temp_archive
    } else {
        input.to_path_buf()
    };

    let installed =
        match library.install_mod_from_package(&lib_config, &source_archive.to_string_lossy()) {
            Ok(outcome) => outcome.into_mod(),
            Err(e) => {
                return Ok(RepairResult::Failed {
                    path: input.to_path_buf(),
                    format,
                    error: format!("Failed to import mod into temporary library: {e}"),
                });
            }
        };

    // Step 4: Run LTK Manager repair engine
    let fix_report = match library.repair_mod(&lib_config, &installed.id) {
        Ok(report) => report,
        Err(e) => {
            return Ok(RepairResult::Failed {
                path: input.to_path_buf(),
                format,
                error: format!("LTK repair engine error: {e}"),
            });
        }
    };

    if fix_report.applied == 0 {
        return Ok(RepairResult::Unchanged {
            path: input.to_path_buf(),
            format,
        });
    }

    // Step 5: Export repaired mod
    let export_dir = work_path.join("export");
    if let Err(e) = library.export_mods(
        &lib_config,
        ExportScope::All,
        ExportShape::Folder,
        &export_dir,
    ) {
        return Ok(RepairResult::Failed {
            path: input.to_path_buf(),
            format,
            error: format!("Exporting repaired mod failed: {e}"),
        });
    }

    let exported_file = fs::read_dir(&export_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|p| p.is_file())
        .ok_or_else(|| FixerError::Repair("Exported repaired archive was not found".to_string()))?;

    // Step 6: Post-repair verification
    let post_health = check_mod_health(&exported_file, config)?;
    let verified = post_health.status == ModHealthStatus::Healthy
        || post_health.repairable_count < initial_health.repairable_count;

    if !verified && post_health.status == ModHealthStatus::Broken {
        return Ok(RepairResult::Failed {
            path: input.to_path_buf(),
            format,
            error: format!(
                "Verification failed: repaired archive is invalid ({})",
                post_health.reason.as_deref().unwrap_or("unknown error")
            ),
        });
    }

    // Step 7: Safe replacement / writing to output
    let is_output_folder = match output {
        Some(out) => {
            let out_str = out.to_string_lossy().to_ascii_lowercase();
            !out_str.ends_with(".fantome") && !out_str.ends_with(".zip")
        }
        None => format == ModFormat::FantomeFolder,
    };

    if is_output_folder {
        let temp_unpacked = work_path.join("unpacked_output");
        if let Err(e) = crate::formats::unpack_fantome_archive(&exported_file, &temp_unpacked) {
            return Ok(RepairResult::Failed {
                path: input.to_path_buf(),
                format,
                error: format!("Failed to unpack repaired archive: {e}"),
            });
        }
        if let Err(e) = replace_folder_safely(target_dest, &temp_unpacked, backup) {
            return Ok(RepairResult::Failed {
                path: input.to_path_buf(),
                format,
                error: format!("Failed to save repaired folder: {e}"),
            });
        }

        // If this mod is inside a Rose skin directory, synchronize rose_mod_targets.json
        if let Err(e) = crate::rose::update_rose_manifest_if_present(target_dest) {
            tracing::warn!("Failed to synchronize Rose targets manifest: {e}");
        }
    } else if let Err(e) = replace_file_safely(target_dest, &exported_file, backup) {
        return Ok(RepairResult::Failed {
            path: input.to_path_buf(),
            format,
            error: format!("Failed to save repaired file: {e}"),
        });
    }

    Ok(RepairResult::Repaired {
        path: input.to_path_buf(),
        format,
        fixes_applied: fix_report.applied,
        output_path: target_dest.to_path_buf(),
        verified,
    })
}
