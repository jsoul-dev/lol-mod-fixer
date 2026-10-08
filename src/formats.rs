//! Mod archive format classification and detection.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModFormat {
    /// Fantome zip archive (.fantome or .zip)
    Fantome,
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
            Self::Fantome => "Fantome",
            Self::Modpkg => "ModPkg",
            Self::WadClient => "WadClient",
            Self::Unsupported => "Unsupported",
        }
    }

    /// Whether this format can be repaired by LTK Manager's repair engine.
    pub fn is_repairable_format(&self) -> bool {
        matches!(self, Self::Fantome)
    }

    /// Detect format from path and file magic.
    pub fn detect(path: &Path) -> Self {
        let path_str = path.to_string_lossy().to_ascii_lowercase();

        if path_str.ends_with(".wad.client") || path_str.ends_with(".wad") {
            return Self::WadClient;
        }

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext.to_ascii_lowercase().as_str() {
                "fantome" => return Self::Fantome,
                "modpkg" => return Self::Modpkg,
                "zip" if is_fantome_zip(path) => return Self::Fantome,
                _ => {}
            }
        }

        // Sniff first few bytes if file exists
        if let Ok(mut file) = File::open(path) {
            let mut magic = [0u8; 4];
            if file.read_exact(&mut magic).is_ok() {
                // Zip signature PK\x03\x04
                if magic == [0x50, 0x4B, 0x03, 0x04] {
                    return Self::Fantome;
                }
                // WAD signature "RW4\x00" - "RW4\x03"
                if magic[0..3] == [0x52, 0x57, 0x34] {
                    return Self::WadClient;
                }
            }
        }

        Self::Unsupported
    }
}

fn is_fantome_zip(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };

    // Check if zip contains META/info.json or WAD/ or RAW/
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_ascii_uppercase();
            if name.starts_with("META/INFO.JSON")
                || name.starts_with("WAD/")
                || name.starts_with("RAW/")
            {
                return true;
            }
        }
    }
    false
}
