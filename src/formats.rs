//! Mod archive format classification and detection.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::{FixerError, FixerResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModFormat {
    /// Fantome zip archive (.fantome or .zip)
    Fantome,
    /// Extracted Fantome folder containing META/info.json and WAD/
    #[serde(rename = "fantome_folder")]
    FantomeFolder,
    /// Modpkg package (.modpkg)
    Modpkg,
    /// Standalone packed WAD (.wad.client)
    WadClient,
    /// Unknown / unsupported format
    Unsupported,
}

impl ModFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Fantome => "Fantome Archive",
            Self::FantomeFolder => "Fantome Folder",
            Self::Modpkg => "ModPkg",
            Self::WadClient => "WadClient",
            Self::Unsupported => "Unsupported",
        }
    }

    /// Whether this format can be repaired by LTK Manager's repair engine.
    pub fn is_repairable_format(&self) -> bool {
        matches!(self, Self::Fantome | Self::FantomeFolder | Self::Modpkg)
    }

    /// Detect format from path, directory layout, or file magic.
    pub fn detect(path: &Path) -> Self {
        if path.is_dir() {
            if is_fantome_folder(path) {
                return Self::FantomeFolder;
            }
            return Self::Unsupported;
        }

        let path_str = path.to_string_lossy().to_ascii_lowercase();

        if path_str.ends_with(".wad.client") || path_str.ends_with(".wad") {
            return Self::WadClient;
        }

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_ascii_lowercase().as_str() {
                "fantome" => return Self::Fantome,
                "modpkg" => return Self::Modpkg,
                "zip" => {
                    if is_fantome_zip(path) {
                        return Self::Fantome;
                    }
                    return Self::Unsupported;
                }
                // Known non-mod extensions: never treat as mod
                _ => return Self::Unsupported,
            }
        }

        // For extensionless files only, sniff magic with strict validation
        if let Ok(mut file) = File::open(path) {
            let mut magic = [0u8; 4];
            if file.read_exact(&mut magic).is_ok() {
                // WAD signature "RW4\x00" - "RW4\x03"
                if magic[0..3] == [0x52, 0x57, 0x34] {
                    return Self::WadClient;
                }
                // Zip signature PK\x03\x04 - ONLY if it verified contains Fantome mod structure
                if magic == [0x50, 0x4B, 0x03, 0x04] && is_fantome_zip(path) {
                    return Self::Fantome;
                }
            }
        }

        Self::Unsupported
    }
}

/// Check if a directory is an extracted Fantome mod folder.
///
/// An extracted Fantome mod has:
/// - A `META` directory containing `info.json`
/// - Mod assets or directories (`WAD`, `RAW`, `DATA`, etc.)
pub fn is_fantome_folder(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let has_meta_info = path.join("META").join("info.json").is_file()
        || path.join("meta").join("info.json").is_file();
    if !has_meta_info {
        return false;
    }
    let has_mod_content = path.join("WAD").exists()
        || path.join("wad").exists()
        || path.join("RAW").exists()
        || path.join("raw").exists()
        || path.join("DATA").exists()
        || path.join("data").exists();
    if has_mod_content {
        return true;
    }
    // Fallback: check if there is any other non-metadata entry
    if let Ok(entries) = fs_err::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
            if name != "META" && !crate::cleanup::is_os_metadata_file(&name) {
                return true;
            }
        }
    }
    false
}

/// Check if a path is a Fantome mod archive (.fantome file or valid mod .zip file).
pub fn is_fantome_archive(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("fantome") => true,
        Some("zip") => is_fantome_zip(path),
        _ => false,
    }
}

fn is_fantome_zip(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };

    // Check if zip contains META/info.json or WAD/ or RAW/ or DATA/ (top-level or inside subfolder)
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_ascii_uppercase().replace('\\', "/");
            if name.ends_with("META/INFO.JSON")
                || name.starts_with("META/INFO.JSON")
                || name.contains("/WAD/")
                || name.starts_with("WAD/")
                || name.contains("/RAW/")
                || name.starts_with("RAW/")
                || name.contains("/DATA/")
                || name.starts_with("DATA/")
            {
                return true;
            }
        }
    }
    false
}

/// Pack an extracted Fantome mod folder into a temporary or destination .fantome archive.
/// Uses `Stored` (no compression) for maximum speed and exact compatibility with LTK Manager.
pub fn pack_fantome_folder(folder: &Path, output_zip: &Path) -> FixerResult<()> {
    let file = fs_err::File::create(output_zip)?;
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for entry in walkdir::WalkDir::new(folder) {
        let entry = entry.map_err(|e| FixerError::Io(std::io::Error::other(e)))?;
        let path = entry.path();
        if path.is_file() {
            let rel_path = path.strip_prefix(folder).map_err(|e| {
                FixerError::Archive(format!("Failed to compute relative path: {e}"))
            })?;
            let name_in_zip = rel_path.to_string_lossy().replace('\\', "/");
            // Skip OS metadata files
            if name_in_zip.ends_with(".DS_Store")
                || name_in_zip.ends_with("Thumbs.db")
                || name_in_zip.ends_with("desktop.ini")
            {
                continue;
            }
            zip.start_file(name_in_zip, options)
                .map_err(|e| FixerError::Archive(format!("Zip start_file failed: {e}")))?;
            let mut file_content = fs_err::File::open(path)?;
            std::io::copy(&mut file_content, &mut zip)?;
        }
    }
    zip.finish()
        .map_err(|e| FixerError::Archive(format!("Zip finish failed: {e}")))?;
    Ok(())
}

/// Unpack a .fantome zip archive into a destination directory.
pub fn unpack_fantome_archive(archive_path: &Path, destination_dir: &Path) -> FixerResult<()> {
    fs_err::create_dir_all(destination_dir)?;
    let file = fs_err::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| FixerError::Archive(format!("Failed to open zip archive: {e}")))?;
    archive
        .extract(destination_dir)
        .map_err(|e| FixerError::Archive(format!("Failed to extract zip archive: {e}")))?;
    Ok(())
}

/// Convert a .modpkg binary package into a temporary or destination .fantome archive.
pub fn convert_modpkg_to_fantome(modpkg_path: &Path, fantome_path: &Path) -> FixerResult<()> {
    let temp_dir = tempfile::TempDir::new()?;
    let project_dir = temp_dir.path().join("project");
    fs_err::create_dir_all(&project_dir)?;

    let project_utf8: camino::Utf8PathBuf = project_dir
        .try_into()
        .map_err(|_| FixerError::Archive("Project path is not valid UTF-8".to_string()))?;

    let file = fs_err::File::open(modpkg_path)?;
    ltk_mod_project::ProjectImporter::new(&project_utf8)
        .import(ltk_mod_project::modpkg::ModpkgImporter::new(file))
        .map_err(|e| FixerError::Archive(format!("Modpkg import failed: {e}")))?;

    let output_file = fs_err::File::create(fantome_path)?;
    ltk_mod_project::ProjectPacker::from_dir(&project_utf8)
        .map_err(|e| FixerError::Archive(format!("Project packer failed: {e}")))?
        .pack(ltk_mod_project::fantome::FantomeFormat::new(output_file))
        .map_err(|e| FixerError::Archive(format!("Fantome pack failed: {e}")))?;

    Ok(())
}

/// Convert a .fantome archive into a .modpkg binary package.
pub fn convert_fantome_to_modpkg(fantome_path: &Path, modpkg_path: &Path) -> FixerResult<()> {
    let temp_dir = tempfile::TempDir::new()?;
    let project_dir = temp_dir.path().join("project");
    fs_err::create_dir_all(&project_dir)?;

    let project_utf8: camino::Utf8PathBuf = project_dir
        .try_into()
        .map_err(|_| FixerError::Archive("Project path is not valid UTF-8".to_string()))?;

    let file = fs_err::File::open(fantome_path)?;
    ltk_mod_project::ProjectImporter::new(&project_utf8)
        .import(ltk_mod_project::fantome::FantomeImporter::new(file))
        .map_err(|e| FixerError::Archive(format!("Fantome import failed: {e}")))?;

    let output_file = fs_err::File::create(modpkg_path)?;
    ltk_mod_project::ProjectPacker::from_dir(&project_utf8)
        .map_err(|e| FixerError::Archive(format!("Project packer failed: {e}")))?
        .pack(ltk_mod_project::modpkg::ModpkgFormat::new(output_file))
        .map_err(|e| FixerError::Archive(format!("Modpkg pack failed: {e}")))?;

    Ok(())
}
