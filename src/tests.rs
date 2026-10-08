//! Comprehensive tests for lol-mod-fixer.

use std::path::PathBuf;
use tempfile::TempDir;

use ltk_manager_assets::test_util::{
    build_packed_wad, healthy_bin, make_packed_bin_fantome_zip, stale_bin,
};
use ltk_manager_base::config::Config as LtkConfig;

use crate::formats::ModFormat;
use crate::health::{ModHealthStatus, check_mod_health};
use crate::output::{JsonCheckOutput, JsonCheckSummary, JsonRepairOutput, JsonRepairSummary};
use crate::repair::{RepairResult, repair_mod_archive};
use crate::scanner::scan_directory;

fn dummy_config() -> LtkConfig {
    LtkConfig::default()
}

#[test]
fn test_formats_detection() {
    let temp = TempDir::new().unwrap();

    let fantome_file = temp.path().join("Ahri.fantome");
    make_packed_bin_fantome_zip(
        &fantome_file,
        "Ahri",
        &healthy_bin(),
        zip::CompressionMethod::Stored,
    );
    assert_eq!(ModFormat::detect(&fantome_file), ModFormat::Fantome);

    let modpkg_file = temp.path().join("Lux.modpkg");
    fs_err::write(&modpkg_file, b"test-modpkg-content").unwrap();
    assert_eq!(ModFormat::detect(&modpkg_file), ModFormat::Modpkg);

    let wad_file = temp.path().join("Aatrox.wad.client");
    let packed = build_packed_wad(&[]);
    fs_err::write(&wad_file, packed).unwrap();
    assert_eq!(ModFormat::detect(&wad_file), ModFormat::WadClient);

    let txt_file = temp.path().join("notes.txt");
    fs_err::write(&txt_file, b"notes").unwrap();
    assert_eq!(ModFormat::detect(&txt_file), ModFormat::Unsupported);
}

#[test]
fn test_healthy_mod_detection() {
    let temp = TempDir::new().unwrap();
    let mod_path = temp.path().join("HealthySkin.fantome");
    make_packed_bin_fantome_zip(
        &mod_path,
        "HealthySkin",
        &healthy_bin(),
        zip::CompressionMethod::Stored,
    );

    let config = dummy_config();
    let report = check_mod_health(&mod_path, &config).unwrap();

    assert_eq!(report.status, ModHealthStatus::Healthy);
    assert_eq!(report.format, ModFormat::Fantome);
    assert_eq!(report.repairable_count, 0);
}

#[test]
fn test_repairable_mod_detection() {
    let temp = TempDir::new().unwrap();
    let mod_path = temp.path().join("StaleSkin.fantome");
    make_packed_bin_fantome_zip(
        &mod_path,
        "StaleSkin",
        &stale_bin(),
        zip::CompressionMethod::Stored,
    );

    let config = dummy_config();
    let report = check_mod_health(&mod_path, &config).unwrap();

    assert_eq!(report.status, ModHealthStatus::Repairable);
    assert_eq!(report.format, ModFormat::Fantome);
    assert!(report.repairable_count > 0);
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.rule_id == "bin/property-type" && p.repairable)
    );
}

#[test]
fn test_repair_execution_and_post_verification() {
    let temp = TempDir::new().unwrap();
    let mod_path = temp.path().join("RepairMe.fantome");
    make_packed_bin_fantome_zip(
        &mod_path,
        "RepairMe",
        &stale_bin(),
        zip::CompressionMethod::Stored,
    );

    let config = dummy_config();
    let result = repair_mod_archive(&mod_path, None, &config, false, false).unwrap();

    assert!(result.is_repaired());
    if let RepairResult::Repaired {
        fixes_applied,
        verified,
        ..
    } = result
    {
        assert!(fixes_applied > 0);
        assert!(verified);
    } else {
        panic!("Expected RepairResult::Repaired");
    }

    // Run health check again on the repaired archive
    let post_report = check_mod_health(&mod_path, &config).unwrap();
    assert_eq!(post_report.status, ModHealthStatus::Healthy);
    assert_eq!(post_report.repairable_count, 0);
}

#[test]
fn test_unchanged_mod_not_rewritten() {
    let temp = TempDir::new().unwrap();
    let mod_path = temp.path().join("CleanSkin.fantome");
    make_packed_bin_fantome_zip(
        &mod_path,
        "CleanSkin",
        &healthy_bin(),
        zip::CompressionMethod::Stored,
    );

    let before_bytes = fs_err::read(&mod_path).unwrap();

    let config = dummy_config();
    let result = repair_mod_archive(&mod_path, None, &config, false, false).unwrap();

    assert!(result.is_unchanged());
    let after_bytes = fs_err::read(&mod_path).unwrap();
    assert_eq!(before_bytes, after_bytes);
}

#[test]
fn test_dry_run_leaves_original_untouched() {
    let temp = TempDir::new().unwrap();
    let mod_path = temp.path().join("DryRunSkin.fantome");
    make_packed_bin_fantome_zip(
        &mod_path,
        "DryRunSkin",
        &stale_bin(),
        zip::CompressionMethod::Stored,
    );

    let before_bytes = fs_err::read(&mod_path).unwrap();

    let config = dummy_config();
    let result = repair_mod_archive(&mod_path, None, &config, false, true).unwrap();

    assert!(result.is_repaired());
    let after_bytes = fs_err::read(&mod_path).unwrap();
    assert_eq!(before_bytes, after_bytes); // File must be untouched
}

#[test]
fn test_backup_mode_preserves_original() {
    let temp = TempDir::new().unwrap();
    let mod_path = temp.path().join("BackupSkin.fantome");
    make_packed_bin_fantome_zip(
        &mod_path,
        "BackupSkin",
        &stale_bin(),
        zip::CompressionMethod::Stored,
    );

    let original_bytes = fs_err::read(&mod_path).unwrap();

    let config = dummy_config();
    let result = repair_mod_archive(&mod_path, None, &config, true, false).unwrap();
    assert!(result.is_repaired());

    let backup_path = temp.path().join("BackupSkin.fantome.bak");
    assert!(backup_path.exists());
    let backup_bytes = fs_err::read(&backup_path).unwrap();
    assert_eq!(original_bytes, backup_bytes);
}

#[test]
fn test_output_path_with_spaces_and_unicode() {
    let temp = TempDir::new().unwrap();
    let mod_path = temp.path().join("UnicodeSource.fantome");
    make_packed_bin_fantome_zip(
        &mod_path,
        "UnicodeSource",
        &stale_bin(),
        zip::CompressionMethod::Stored,
    );

    let out_dir = temp.path().join("Fixed Mods 目標目錄");
    let out_file = out_dir.join("Repaired Skin 最終版.fantome");

    let config = dummy_config();
    let result = repair_mod_archive(&mod_path, Some(&out_file), &config, false, false).unwrap();
    assert!(result.is_repaired());
    assert!(out_file.exists());

    // Original file must remain untouched
    let orig_report = check_mod_health(&mod_path, &config).unwrap();
    assert_eq!(orig_report.status, ModHealthStatus::Repairable);

    // Repaired output file must be healthy
    let out_report = check_mod_health(&out_file, &config).unwrap();
    assert_eq!(out_report.status, ModHealthStatus::Healthy);
}

#[test]
fn test_wad_client_behavior_and_limitation() {
    let temp = TempDir::new().unwrap();
    let wad_path = temp.path().join("Aatrox.wad.client");
    let packed = build_packed_wad(&[]);
    fs_err::write(&wad_path, packed).unwrap();

    let config = dummy_config();
    let report = check_mod_health(&wad_path, &config).unwrap();

    assert_eq!(report.status, ModHealthStatus::Unsupported);
    assert_eq!(report.format, ModFormat::WadClient);
    assert!(
        report
            .reason
            .as_ref()
            .unwrap()
            .contains("packed .wad.client")
    );

    let repair_result = repair_mod_archive(&wad_path, None, &config, false, false).unwrap();
    assert!(repair_result.is_unsupported());
}

#[test]
fn test_modpkg_behavior_and_limitation() {
    let temp = TempDir::new().unwrap();
    let modpkg_path = temp.path().join("Legacy.modpkg");
    fs_err::write(&modpkg_path, b"test-modpkg").unwrap();

    let config = dummy_config();
    let report = check_mod_health(&modpkg_path, &config).unwrap();

    assert_eq!(report.status, ModHealthStatus::Unrepairable);
    assert_eq!(report.format, ModFormat::Modpkg);
    assert!(
        report
            .reason
            .as_ref()
            .unwrap()
            .contains(".modpkg is read straight out of its archive")
    );

    let repair_result = repair_mod_archive(&modpkg_path, None, &config, false, false).unwrap();
    assert!(repair_result.is_unrepairable());
}

#[test]
fn test_invalid_path_reporting() {
    let config = dummy_config();
    let missing_path = PathBuf::from("C:\\definitely_missing_path_123456789.fantome");
    let report = check_mod_health(&missing_path, &config).unwrap();

    assert_eq!(report.status, ModHealthStatus::Broken);
    assert!(report.reason.as_ref().unwrap().contains("does not exist"));
}

#[test]
fn test_directory_scan_filtering() {
    let temp = TempDir::new().unwrap();

    // Create supported mods
    let m1 = temp.path().join("Skin1.fantome");
    let m2 = temp.path().join("Skin2.modpkg");
    let m3 = temp.path().join("Skin3.wad.client");
    make_packed_bin_fantome_zip(&m1, "Skin1", &healthy_bin(), zip::CompressionMethod::Stored);
    fs_err::write(&m2, b"pkg").unwrap();
    fs_err::write(&m3, b"wad").unwrap();

    // Create ignored files
    let ig1 = temp.path().join("Skin1.fantome.bak");
    let ig2 = temp.path().join(".Skin1.tmp");
    let ig3 = temp.path().join("readme.txt");
    let ig4 = temp.path().join("app.exe");
    fs_err::write(&ig1, b"bak").unwrap();
    fs_err::write(&ig2, b"tmp").unwrap();
    fs_err::write(&ig3, b"txt").unwrap();
    fs_err::write(&ig4, b"exe").unwrap();

    // Subdirectory
    let sub = temp.path().join("subfolder");
    fs_err::create_dir_all(&sub).unwrap();
    let sub_mod = sub.join("SubSkin.fantome");
    make_packed_bin_fantome_zip(
        &sub_mod,
        "SubSkin",
        &healthy_bin(),
        zip::CompressionMethod::Stored,
    );

    // Non-recursive scan
    let non_rec = scan_directory(temp.path(), false).unwrap();
    assert_eq!(non_rec.len(), 3);
    assert!(non_rec.contains(&m1));
    assert!(non_rec.contains(&m2));
    assert!(non_rec.contains(&m3));

    // Recursive scan
    let rec = scan_directory(temp.path(), true).unwrap();
    assert_eq!(rec.len(), 4);
    assert!(rec.contains(&sub_mod));
}

#[test]
fn test_json_output_validity() {
    let check_out = JsonCheckOutput {
        mode: "check",
        directory: Some("D:\\Mods".to_string()),
        summary: JsonCheckSummary {
            scanned: 2,
            healthy: 1,
            repairable: 1,
            unrepairable: 0,
            broken: 0,
            unsupported: 0,
        },
        mods: vec![],
    };
    let json_str = serde_json::to_string(&check_out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed["mode"], "check");
    assert_eq!(parsed["summary"]["scanned"], 2);

    let repair_out = JsonRepairOutput {
        mode: "repair",
        directory: Some("D:\\Mods".to_string()),
        summary: JsonRepairSummary {
            scanned: 2,
            healthy: 1,
            repaired: 1,
            unrepairable: 0,
            failed: 0,
            unsupported: 0,
        },
        mods: vec![],
    };
    let json_str2 = serde_json::to_string(&repair_out).unwrap();
    let parsed2: serde_json::Value = serde_json::from_str(&json_str2).unwrap();
    assert_eq!(parsed2["mode"], "repair");
    assert_eq!(parsed2["summary"]["repaired"], 1);
}
