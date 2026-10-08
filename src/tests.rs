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

#[test]
fn test_extracted_fantome_folder_inspection_and_repair() {
    let temp = TempDir::new().unwrap();
    let mod_dir = temp.path().join("Tank-1.0.0-not-fixed");
    let meta_dir = mod_dir.join("META");
    let wad_dir = mod_dir.join("WAD");
    fs_err::create_dir_all(&meta_dir).unwrap();
    fs_err::create_dir_all(&wad_dir).unwrap();

    let info_content = r#"{
  "Name": "Tank Volibear",
  "Author": "Frog",
  "Version": "1.0.0",
  "Description": "Test mod"
}"#;
    fs_err::write(meta_dir.join("info.json"), info_content).unwrap();

    // Stale BIN in WAD
    let stale_data = ltk_manager_assets::test_util::build_packed_wad(&[(
        "data/characters/volibear/skins/skin0.bin",
        &ltk_manager_assets::test_util::bin_bytes(&stale_bin()),
    )]);
    fs_err::write(wad_dir.join("Volibear.wad.client"), stale_data).unwrap();

    // Check detection
    assert_eq!(ModFormat::detect(&mod_dir), ModFormat::FantomeFolder);

    let config = dummy_config();

    // Health check on the extracted folder
    let report = check_mod_health(&mod_dir, &config).unwrap();
    assert_eq!(report.format, ModFormat::FantomeFolder);
    assert_eq!(report.status, ModHealthStatus::Repairable);
    assert!(report.repairable_count > 0);

    // Repair in-place
    let result = repair_mod_archive(&mod_dir, None, &config, false, false).unwrap();
    assert!(result.is_repaired());

    // Verify folder was repaired in-place
    assert!(meta_dir.join("info.json").exists());
    assert!(wad_dir.join("Volibear.wad.client").exists());

    // Health check on repaired folder must now be healthy!
    let post_report = check_mod_health(&mod_dir, &config).unwrap();
    assert_eq!(post_report.status, ModHealthStatus::Healthy);
    assert_eq!(post_report.format, ModFormat::FantomeFolder);
}

#[test]
fn test_scan_directory_with_extracted_fantome_folders() {
    let temp = TempDir::new().unwrap();
    let skins_dir = temp.path().join("skins");
    let skin_id_dir = skins_dir.join("34000");
    let mod_folder = skin_id_dir.join("Emilia_Anivia-1.0.0");
    fs_err::create_dir_all(mod_folder.join("META")).unwrap();
    fs_err::create_dir_all(mod_folder.join("WAD")).unwrap();
    fs_err::write(mod_folder.join("META").join("info.json"), b"{}").unwrap();
    fs_err::write(mod_folder.join("WAD").join("Anivia.wad.client"), b"test").unwrap();

    // Add another loose fantome archive in skins
    let loose_fantome = skins_dir.join("LooseSkin.fantome");
    make_packed_bin_fantome_zip(
        &loose_fantome,
        "LooseSkin",
        &healthy_bin(),
        zip::CompressionMethod::Stored,
    );

    // Scan skin_id_dir (non-recursive)
    let found_in_id = scan_directory(&skin_id_dir, false).unwrap();
    assert_eq!(found_in_id.len(), 1);
    assert_eq!(found_in_id[0], mod_folder);

    // Scan skins_dir (recursive)
    let found_in_skins = scan_directory(&skins_dir, true).unwrap();
    assert_eq!(found_in_skins.len(), 2);
    assert!(found_in_skins.contains(&mod_folder));
    assert!(found_in_skins.contains(&loose_fantome));
}

#[test]
fn test_rose_manifest_synchronization() {
    let temp = TempDir::new().unwrap();
    let champ_dir = temp.path().join("34000");
    fs_err::create_dir_all(&champ_dir).unwrap();

    let mod_dir = champ_dir.join("Emilia_Anivia-1.0.0");
    let meta_dir = mod_dir.join("META");
    let wad_dir = mod_dir.join("WAD");
    fs_err::create_dir_all(&meta_dir).unwrap();
    fs_err::create_dir_all(&wad_dir).unwrap();

    fs_err::write(meta_dir.join("info.json"), b"{\"Name\": \"Anivia\"}").unwrap();
    fs_err::write(wad_dir.join("Anivia.wad.client"), b"old-wad-bytes-12345").unwrap();

    // Create an initial Rose targets manifest with old hashes
    let manifest_path = champ_dir.join("rose_mod_targets.json");
    let initial_manifest = serde_json::json!({
        "version": 1,
        "championId": 34,
        "targets": [],
        "mods": {
            "old_hash_key_111": {
                "name": "Emilia_Anivia-1.0.0",
                "folderHash": "old_hash_key_111",
                "wadHashes": {
                    "WAD/Anivia.wad.client": "old_wad_hash_999"
                },
                "targets": [34000],
                "displayName": "Emilia Anivia"
            }
        }
    });
    fs_err::write(
        &manifest_path,
        serde_json::to_string_pretty(&initial_manifest).unwrap(),
    )
    .unwrap();

    // Compute Rose hashes directly
    let (folder_hash, wad_hashes) = crate::rose::compute_rose_hashes(&mod_dir).unwrap();
    assert!(!folder_hash.is_empty());
    assert_eq!(wad_hashes.len(), 1);
    assert!(wad_hashes.contains_key("WAD/Anivia.wad.client"));

    // Modify WAD content to simulate repair
    fs_err::write(
        wad_dir.join("Anivia.wad.client"),
        b"repaired-wad-bytes-99999",
    )
    .unwrap();

    // Update Rose manifest
    let updated = crate::rose::update_rose_manifest_if_present(&mod_dir).unwrap();
    assert!(updated);

    // Read back manifest and verify
    let read_back: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&manifest_path).unwrap()).unwrap();

    let mods = read_back.get("mods").unwrap().as_object().unwrap();
    // Old key must be gone
    assert!(!mods.contains_key("old_hash_key_111"));

    // New folder hash key must exist
    let (new_folder_hash, new_wad_hashes) = crate::rose::compute_rose_hashes(&mod_dir).unwrap();
    assert!(mods.contains_key(&new_folder_hash));

    let entry = mods.get(&new_folder_hash).unwrap();
    assert_eq!(
        entry.get("name").unwrap().as_str().unwrap(),
        "Emilia_Anivia-1.0.0"
    );
    assert_eq!(
        entry.get("folderHash").unwrap().as_str().unwrap(),
        new_folder_hash
    );
    assert_eq!(
        entry.get("displayName").unwrap().as_str().unwrap(),
        "Emilia Anivia"
    );
    assert_eq!(
        entry.get("targets").unwrap().as_array().unwrap()[0]
            .as_u64()
            .unwrap(),
        34000
    );

    let stored_wads = entry.get("wadHashes").unwrap().as_object().unwrap();
    assert_eq!(
        stored_wads
            .get("WAD/Anivia.wad.client")
            .unwrap()
            .as_str()
            .unwrap(),
        new_wad_hashes.get("WAD/Anivia.wad.client").unwrap()
    );
}

#[test]
fn test_rose_multiple_mods_on_same_skin_id() {
    let temp = TempDir::new().unwrap();
    let champ_dir = temp.path().join("106000");
    fs_err::create_dir_all(&champ_dir).unwrap();

    // Mod 1: tank-volibear
    let mod1_dir = champ_dir.join("tank-volibear");
    fs_err::create_dir_all(mod1_dir.join("META")).unwrap();
    fs_err::create_dir_all(mod1_dir.join("WAD")).unwrap();
    fs_err::write(mod1_dir.join("META/info.json"), b"{\"Name\": \"Volibear\"}").unwrap();
    fs_err::write(
        mod1_dir.join("WAD/Volibear.wad.client"),
        b"volibear-wad-initial",
    )
    .unwrap();

    // Mod 2: Angel-v1.0.0
    let mod2_dir = champ_dir.join("Angel-v1.0.0");
    fs_err::create_dir_all(mod2_dir.join("META")).unwrap();
    fs_err::create_dir_all(mod2_dir.join("WAD")).unwrap();
    fs_err::write(mod2_dir.join("META/info.json"), b"{\"Name\": \"Angel\"}").unwrap();
    fs_err::write(
        mod2_dir.join("WAD/Volibear.wad.client"),
        b"angel-wad-initial",
    )
    .unwrap();

    let (mod1_init_folder_hash, mod1_init_wads) =
        crate::rose::compute_rose_hashes(&mod1_dir).unwrap();
    let (mod2_init_folder_hash, mod2_init_wads) =
        crate::rose::compute_rose_hashes(&mod2_dir).unwrap();

    // Create Rose manifest with both mods
    let manifest_path = champ_dir.join("rose_mod_targets.json");
    let initial_manifest = serde_json::json!({
        "version": 1,
        "championId": 106,
        "targets": [],
        "mods": {
            &mod1_init_folder_hash: {
                "name": "tank-volibear",
                "folderHash": &mod1_init_folder_hash,
                "wadHashes": mod1_init_wads,
                "targets": [106000]
            },
            &mod2_init_folder_hash: {
                "name": "Angel-v1.0.0",
                "folderHash": &mod2_init_folder_hash,
                "wadHashes": mod2_init_wads,
                "targets": [106000]
            }
        }
    });
    fs_err::write(
        &manifest_path,
        serde_json::to_string_pretty(&initial_manifest).unwrap(),
    )
    .unwrap();

    // Simulate repair of Mod 2 (Angel) only
    fs_err::write(
        mod2_dir.join("WAD/Volibear.wad.client"),
        b"angel-wad-repaired-456",
    )
    .unwrap();
    let updated2 = crate::rose::update_rose_manifest_if_present(&mod2_dir).unwrap();
    assert!(updated2);

    // Verify manifest: Mod 2 is updated, Mod 1 is preserved untouched!
    let read1: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&manifest_path).unwrap()).unwrap();
    let mods1 = read1.get("mods").unwrap().as_object().unwrap();
    assert_eq!(mods1.len(), 2, "Both mods must still exist in manifest");
    assert!(
        mods1.contains_key(&mod1_init_folder_hash),
        "Mod 1 must be untouched"
    );

    let (mod2_new_hash, _) = crate::rose::compute_rose_hashes(&mod2_dir).unwrap();
    assert!(
        mods1.contains_key(&mod2_new_hash),
        "Mod 2 must have new key"
    );

    // Now simulate repair of Mod 1 (Volibear) as well
    fs_err::write(
        mod1_dir.join("WAD/Volibear.wad.client"),
        b"volibear-wad-repaired-789",
    )
    .unwrap();
    let updated1 = crate::rose::update_rose_manifest_if_present(&mod1_dir).unwrap();
    assert!(updated1);

    // Verify manifest: BOTH mods now have their new hashes and targets intact
    let read2: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&manifest_path).unwrap()).unwrap();
    let mods2 = read2.get("mods").unwrap().as_object().unwrap();
    assert_eq!(mods2.len(), 2, "Both mods must still exist in manifest");

    let (mod1_new_hash, _) = crate::rose::compute_rose_hashes(&mod1_dir).unwrap();
    assert!(mods2.contains_key(&mod1_new_hash));
    assert!(mods2.contains_key(&mod2_new_hash));

    let m1_entry = mods2.get(&mod1_new_hash).unwrap();
    assert_eq!(
        m1_entry.get("name").unwrap().as_str().unwrap(),
        "tank-volibear"
    );
    assert_eq!(
        m1_entry.get("targets").unwrap().as_array().unwrap()[0]
            .as_u64()
            .unwrap(),
        106000
    );

    let m2_entry = mods2.get(&mod2_new_hash).unwrap();
    assert_eq!(
        m2_entry.get("name").unwrap().as_str().unwrap(),
        "Angel-v1.0.0"
    );
    assert_eq!(
        m2_entry.get("targets").unwrap().as_array().unwrap()[0]
            .as_u64()
            .unwrap(),
        106000
    );
}
