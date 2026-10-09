//! Rose skin folders and champion ID mapping generator.
//!
//! Scans skin target folders, cross-references folder IDs with champion and skin
//! names from `skin_ids.json`, and writes:
//! - `skin_mappings.txt`: Human-readable alignment table for Notepad/viewing.
//! - `skin_mappings.json`: Structured JSON for programmatic access.
//!
//! Automatically synchronizes whenever folders are added or deleted.

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::champions::{champion_by_id, skin_by_id};
use crate::error::FixerResult;

pub const MAPPING_TXT_FILENAME: &str = "skin_mappings.txt";
pub const MAPPING_JSON_FILENAME: &str = "skin_mappings.json";

/// Mapping entry for a single skin folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModFolderMapping {
    /// Directory name (e.g. "106000").
    pub folder: String,
    /// Numeric skin ID if the folder name is numeric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin_id: Option<u32>,
    /// Champion ID (skin_id / 1000) if numeric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub champion_id: Option<u32>,
    /// Canonical Champion name (e.g. "Volibear").
    pub champion: String,
    /// Skin name (e.g. "Volibear" or "Thunder Lord Volibear").
    pub skin: String,
    /// Whether this is the champion's default / base skin.
    pub is_base_skin: bool,
    /// List of installed mod names in this skin directory.
    pub mods: Vec<String>,
}

/// JSON root structure for `skin_mappings.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingFileManifest {
    pub title: String,
    pub generated_at: String,
    pub directory: String,
    pub total_folders: usize,
    pub total_mods: usize,
    pub mappings: Vec<ModFolderMapping>,
}

/// Summary returned after mapping generation.
#[derive(Debug, Default, Clone)]
pub struct MappingSummary {
    pub total_folders: usize,
    pub total_mods: usize,
    pub txt_path: PathBuf,
    pub json_path: PathBuf,
}

/// Collect mappings for all skin directories present in `base_dir`.
pub fn collect_mappings(base_dir: &Path) -> FixerResult<Vec<ModFolderMapping>> {
    let mut mappings = Vec::new();

    if !base_dir.is_dir() {
        return Ok(mappings);
    }

    let entries = match fs_err::read_dir(base_dir) {
        Ok(e) => e,
        Err(_) => return Ok(mappings),
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        if folder_name.starts_with('.')
            || folder_name.eq_ignore_ascii_case("hematite-fixed")
            || folder_name.eq_ignore_ascii_case("backup")
        {
            continue;
        }

        let skin_id = folder_name.parse::<u32>().ok();

        // Check if this folder contains mods or Rose target manifests
        let mut mods = Vec::new();
        if let Ok(sub_entries) = fs_err::read_dir(&path) {
            for sub_entry in sub_entries.filter_map(|e| e.ok()) {
                let sub_path = sub_entry.path();
                let sub_name = sub_entry.file_name().to_string_lossy().to_string();
                if sub_name.starts_with('.') || sub_name.eq_ignore_ascii_case("hematite-fixed") {
                    continue;
                }

                if sub_path.is_dir() {
                    mods.push(sub_name);
                } else if sub_path.is_file() {
                    let lower = sub_name.to_ascii_lowercase();
                    if lower.ends_with(".fantome")
                        || lower.ends_with(".modpkg")
                        || lower.ends_with(".wad.client")
                    {
                        let stem = sub_path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or(sub_name);
                        mods.push(stem);
                    }
                }
            }
        }

        // Only include folders that have installed mods
        if mods.is_empty() {
            continue;
        }

        mods.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));

        let (champion_id, champion, skin, is_base_skin) = if let Some(id) = skin_id {
            let cid = id / 1000;
            let champ = champion_by_id(id).unwrap_or("Unknown Champion");
            let sk = skin_by_id(id).unwrap_or(champ);
            let is_base = id % 1000 == 0;
            (Some(cid), champ.to_string(), sk.to_string(), is_base)
        } else {
            let champ = crate::champions::detect_champion(None, &folder_name, None, &[])
                .unwrap_or("Custom Mod");
            (None, champ.to_string(), folder_name.clone(), false)
        };

        mappings.push(ModFolderMapping {
            folder: folder_name,
            skin_id,
            champion_id,
            champion,
            skin,
            is_base_skin,
            mods,
        });
    }

    // Sort folders numerically if skin_id is present, otherwise by name
    mappings.sort_by(|a, b| match (a.skin_id, b.skin_id) {
        (Some(ida), Some(idb)) => ida.cmp(&idb),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.folder.cmp(&b.folder),
    });

    Ok(mappings)
}

/// Format the mappings table for `skin_mappings.txt`.
pub fn format_mappings_txt(
    base_dir: &Path,
    mappings: &[ModFolderMapping],
    timestamp_str: &str,
) -> String {
    let total_folders = mappings.len();
    let total_mods: usize = mappings.iter().map(|m| m.mods.len()).sum();

    let mut txt = String::new();
    txt.push_str(&"=".repeat(100));
    txt.push('\n');
    txt.push_str(" LEAGUE OF LEGENDS - ROSE SKIN FOLDER MAPPINGS\n");
    txt.push_str(&format!(" Directory   : {}\n", base_dir.display()));
    txt.push_str(&format!(" Generated   : {}\n", timestamp_str));
    txt.push_str(&format!(" Total Skin Folders : {}\n", total_folders));
    txt.push_str(&format!(" Total Installed Mods: {}\n", total_mods));
    txt.push_str(&"=".repeat(100));
    txt.push_str("\n\n");

    txt.push_str(&format!(
        "{:<12} {:<20} {:<28} {}\n",
        "FOLDER ID", "CHAMPION", "SKIN NAME", "INSTALLED MODS"
    ));
    txt.push_str(&"-".repeat(100));
    txt.push('\n');

    for item in mappings {
        let folder_id_str = &item.folder;
        let champ_str = &item.champion;
        let skin_display = if item.is_base_skin {
            format!("{} (Base Skin)", item.skin)
        } else {
            item.skin.clone()
        };

        if item.mods.is_empty() {
            txt.push_str(&format!(
                "{:<12} {:<20} {:<28} (no mods installed)\n",
                folder_id_str, champ_str, skin_display
            ));
        } else {
            for (idx, mod_name) in item.mods.iter().enumerate() {
                if idx == 0 {
                    txt.push_str(&format!(
                        "{:<12} {:<20} {:<28} • {}\n",
                        folder_id_str, champ_str, skin_display, mod_name
                    ));
                } else {
                    txt.push_str(&format!(
                        "{:<12} {:<20} {:<28} • {}\n",
                        "", "", "", mod_name
                    ));
                }
            }
        }
    }

    txt.push_str(&format!("\n{}\n", "=".repeat(100)));
    txt
}

/// Generate `skin_mappings.txt` and `skin_mappings.json` beside the skin folders in `base_dir`.
///
/// Returns `Ok(Some(summary))` if mappings were generated, or `Ok(None)` if no skin folders were found.
pub fn generate_skin_mappings(base_dir: &Path) -> FixerResult<Option<MappingSummary>> {
    let mappings = collect_mappings(base_dir)?;

    let txt_path = base_dir.join(MAPPING_TXT_FILENAME);
    let json_path = base_dir.join(MAPPING_JSON_FILENAME);

    // If no skin folders exist, don't generate files unless previous files existed
    if mappings.is_empty() && !txt_path.exists() && !json_path.exists() {
        return Ok(None);
    }

    let timestamp_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let total_folders = mappings.len();
    let total_mods: usize = mappings.iter().map(|m| m.mods.len()).sum();

    // 1. Write skin_mappings.txt atomically
    let txt_content = format_mappings_txt(base_dir, &mappings, &timestamp_str);
    let tmp_txt = base_dir.join(format!(".{}.tmp", MAPPING_TXT_FILENAME));
    fs_err::write(&tmp_txt, &txt_content)?;
    fs_err::rename(&tmp_txt, &txt_path)?;

    // 2. Write skin_mappings.json atomically
    let manifest = MappingFileManifest {
        title: "Rose Skin Folder Mappings".to_string(),
        generated_at: timestamp_str,
        directory: base_dir.display().to_string(),
        total_folders,
        total_mods,
        mappings,
    };
    let json_content = serde_json::to_string_pretty(&manifest)?;
    let tmp_json = base_dir.join(format!(".{}.tmp", MAPPING_JSON_FILENAME));
    fs_err::write(&tmp_json, &json_content)?;
    fs_err::rename(&tmp_json, &json_path)?;

    tracing::info!(
        "Generated skin mappings in {}: {} folders, {} mods",
        base_dir.display(),
        total_folders,
        total_mods
    );

    Ok(Some(MappingSummary {
        total_folders,
        total_mods,
        txt_path,
        json_path,
    }))
}
