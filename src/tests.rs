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

#[test]
fn test_beautify_all_user_examples() {
    use crate::beautify::compute_beautified_title;

    // Example 1: Sonic_Rammus-1.0.0 -> Sonic Rammus v1.0
    let res1 = compute_beautified_title("Sonic_Rammus-1.0.0", Some("33000"), None, None, &[]);
    assert_eq!(res1, "Sonic Rammus v1.0");

    // Example 2: Rammus_Sonic-1.0.0 -> Sonic Rammus v1.0
    let res2 = compute_beautified_title("Rammus_Sonic-1.0.0", Some("33000"), None, None, &[]);
    assert_eq!(res2, "Sonic Rammus v1.0");

    // Example 3: Chun_Li_Garen-2.0 -> Chun Li Garen v2.0
    let res3 = compute_beautified_title("Chun_Li_Garen-2.0", Some("86000"), None, None, &[]);
    assert_eq!(res3, "Chun Li Garen v2.0");

    // Example 4: Ansem_Malzahar-Main -> Ansem Malzahar v1.1.2
    let res4 = compute_beautified_title(
        "Ansem_Malzahar-Main",
        Some("90000"),
        Some("Malzahar - Ansem"),
        Some("1.1.2"),
        &[],
    );
    assert_eq!(res4, "Ansem Malzahar v1.1.2");

    // Example 5: Eto_Shyvana-main -> Eto Shyvana v1.0
    let res5 = compute_beautified_title(
        "Eto_Shyvana-main",
        Some("102000"),
        Some("Eto Shyvana"),
        Some("1.0.0"),
        &[],
    );
    assert_eq!(res5, "Eto Shyvana v1.0");

    // Example 6: Angel-v1.0.0 -> Angel Volibear v1.0
    let res6 = compute_beautified_title("Angel-v1.0.0", Some("106000"), None, None, &[]);
    assert_eq!(res6, "Angel Volibear v1.0");

    // Example 7: tank-volibear -> Tank Volibear v1.0 (with capitalization check)
    let res7 = compute_beautified_title(
        "tank-volibear",
        Some("106000"),
        Some("tank volibear"),
        Some("1.0.0"),
        &[],
    );
    assert_eq!(res7, "Tank Volibear v1.0");

    // Example 8: Zacian_Hecarim-1.1.0 -> Zacian Hecarim v1.1
    let res8 = compute_beautified_title("Zacian_Hecarim-1.1.0", Some("120000"), None, None, &[]);
    assert_eq!(res8, "Zacian Hecarim v1.1");

    // Example 9: Shadow_The_Hedgehog__Ekko_-1.1.3 -> Shadow The Hedgehog Ekko v1.1.3
    let res9 = compute_beautified_title(
        "Shadow_The_Hedgehog__Ekko_-1.1.3",
        Some("245000"),
        None,
        None,
        &[],
    );
    assert_eq!(res9, "Shadow The Hedgehog Ekko v1.1.3");

    // Example 10: Vi Deku (champion first) -> Deku Vi v1.0
    let res10 = compute_beautified_title("Vi Deku", Some("254000"), None, None, &[]);
    assert_eq!(res10, "Deku Vi v1.0");
    let res10_inferred = compute_beautified_title("Vi Deku", None, None, None, &[]);
    assert_eq!(res10_inferred, "Deku Vi v1.0");

    // Example 11: Kai'Sa duplicate / apostrophe stripping
    let res11 = compute_beautified_title("Zero Two Kaisa Kai'Sa", Some("145000"), None, None, &[]);
    assert_eq!(res11, "Zero Two Kai'Sa v1.0");

    // Example 12: Dr. Mundo with Dr. in skin name
    let res12 = compute_beautified_title("Broly Dr. Dr. Mundo", Some("36000"), None, None, &[]);
    assert_eq!(res12, "Broly Dr. Mundo v1.0");

    // Example 13: Rek'Sai triple repetition
    let res13 = compute_beautified_title("Warden Rek'Sai Rek'Sai Rek'Sai-1.4", Some("421000"), None, None, &[]);
    assert_eq!(res13, "Warden Rek'Sai v1.4");

    // Example 14: SAO SINON -> SAO Sinon Caitlyn v1.0 (acronym kept, word title-cased)
    let res14 = compute_beautified_title("SAO SINON Caitlyn", Some("51000"), None, None, &[]);
    assert_eq!(res14, "SAO Sinon Caitlyn v1.0");

    // Example 15: Semi-dupe Caitlyn nickname (Cait)
    let res15 = compute_beautified_title("Usopp Cait", Some("51000"), None, None, &[]);
    assert_eq!(res15, "Usopp Caitlyn v1.0");

    // Example 16: Semi-dupe Aphelios nickname (Aphe)
    let res16 = compute_beautified_title("Angel Aphe", Some("523000"), None, None, &[]);
    assert_eq!(res16, "Angel Aphelios v1.0");

    // Example 17: Semi-dupe Lee Sin (Leesin)
    let res17 = compute_beautified_title("Yuji Leesin", Some("64000"), None, None, &[]);
    assert_eq!(res17, "Yuji Lee Sin v1.0");

    // Example 18: Kog'Maw repetition
    let res18 = compute_beautified_title("Demon Kog'Maw Kog'Maw-1.5", Some("96000"), None, None, &[]);
    assert_eq!(res18, "Demon Kog'Maw v1.5");

    // Example 19: UUID folder name with clean info.json name
    let res19 = compute_beautified_title(
        "B1554516-7fed-462d-Bd77-91da895aa3bd-ziggs",
        Some("115000"),
        Some("Mad Scientist"),
        Some("1.0.0"),
        &[],
    );
    assert_eq!(res19, "Mad Scientist Ziggs v1.0");

    // Idempotency: Running beautification on already-beautified titles MUST remain unchanged
    let all_beautified = [
        (&res1, "33000"),
        (&res2, "33000"),
        (&res3, "86000"),
        (&res4, "90000"),
        (&res5, "102000"),
        (&res6, "106000"),
        (&res7, "106000"),
        (&res8, "120000"),
        (&res9, "245000"),
        (&res10, "254000"),
        (&res11, "145000"),
        (&res12, "36000"),
        (&res13, "421000"),
        (&res14, "51000"),
        (&res15, "51000"),
        (&res16, "523000"),
        (&res17, "64000"),
        (&res18, "96000"),
        (&res19, "115000"),
    ];
    for (title, parent_id) in all_beautified {
        let idempotent = compute_beautified_title(title, Some(parent_id), None, None, &[]);
        assert_eq!(&idempotent, title, "Beautification must be strictly idempotent");
    }
}

#[test]
fn test_beautify_and_sync_folder_in_rose() {
    let temp = TempDir::new().unwrap();
    let champ_dir = temp.path().join("106000");
    fs_err::create_dir_all(&champ_dir).unwrap();

    let raw_mod_dir = champ_dir.join("tank-volibear");
    let meta_dir = raw_mod_dir.join("META");
    let wad_dir = raw_mod_dir.join("WAD");
    fs_err::create_dir_all(&meta_dir).unwrap();
    fs_err::create_dir_all(&wad_dir).unwrap();

    fs_err::write(
        meta_dir.join("info.json"),
        b"{\"Name\": \"tank volibear\", \"Version\": \"1.0.0\"}",
    )
    .unwrap();
    fs_err::write(wad_dir.join("Volibear.wad.client"), b"volibear-wad-content-1").unwrap();

    // Initial manifest
    let manifest_path = champ_dir.join("rose_mod_targets.json");
    let (initial_folder_hash, initial_wad_hashes) =
        crate::rose::compute_rose_hashes(&raw_mod_dir).unwrap();
    let initial_manifest = serde_json::json!({
        "version": 1,
        "championId": 106,
        "targets": [],
        "mods": {
            &initial_folder_hash: {
                "name": "tank-volibear",
                "folderHash": &initial_folder_hash,
                "wadHashes": initial_wad_hashes,
                "targets": [106000]
            }
        }
    });
    fs_err::write(
        &manifest_path,
        serde_json::to_string_pretty(&initial_manifest).unwrap(),
    )
    .unwrap();

    // Execute beautification on the mod directory
    let new_path = crate::beautify::beautify_and_sync_folder(&raw_mod_dir).unwrap();
    assert!(new_path.is_some());
    let beautified_path = new_path.unwrap();
    assert_eq!(
        beautified_path.file_name().unwrap().to_str().unwrap(),
        "Tank Volibear v1.0"
    );
    assert!(!raw_mod_dir.exists(), "Old folder must be renamed");
    assert!(beautified_path.exists(), "New beautified folder must exist");

    // Verify rose manifest updated with new name and hashes
    let read_manifest: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&manifest_path).unwrap()).unwrap();
    let mods = read_manifest.get("mods").unwrap().as_object().unwrap();
    assert_eq!(mods.len(), 1);

    let (expected_new_hash, _) =
        crate::rose::compute_rose_hashes(&beautified_path).unwrap();
    let entry = mods.get(&expected_new_hash).expect("Must be keyed under new folder hash");
    assert_eq!(entry.get("name").unwrap().as_str().unwrap(), "Tank Volibear v1.0");
    assert_eq!(entry.get("folderHash").unwrap().as_str().unwrap(), expected_new_hash);
}

#[test]
fn test_cleanup_empty_folders_and_orphan_manifests() {
    let temp = TempDir::new().unwrap();
    let skins_dir = temp.path();

    // 1. Orphan target folders containing only rose_mod_targets.json
    let orphan1 = skins_dir.join("105000");
    fs_err::create_dir_all(&orphan1).unwrap();
    fs_err::write(orphan1.join("rose_mod_targets.json"), b"{\"mods\":{}}").unwrap();

    let orphan2 = skins_dir.join("13000");
    fs_err::create_dir_all(&orphan2).unwrap();
    fs_err::write(orphan2.join("rose_mod_targets.json"), b"{\"mods\":{}}").unwrap();

    // 2. Active target folder with an active mod and an empty mod subfolder
    let active_target = skins_dir.join("106000");
    let active_mod = active_target.join("Angel Volibear v1.0");
    fs_err::create_dir_all(&active_mod).unwrap();
    fs_err::write(active_mod.join("test.txt"), b"some content").unwrap();
    fs_err::write(active_target.join("rose_mod_targets.json"), b"{\"mods\":{\"test\":{}}}").unwrap();

    let empty_mod = active_target.join("Empty Mod");
    fs_err::create_dir_all(&empty_mod).unwrap();

    // 3. Mod with non-empty META and non-empty META/hashes (must NEVER be deleted)
    let non_empty_mod = active_target.join("Emilia Anivia v1.0");
    let non_empty_hashes = non_empty_mod.join("META").join("hashes");
    fs_err::create_dir_all(&non_empty_hashes).unwrap();
    fs_err::write(non_empty_mod.join("META").join("info.json"), b"{\"Name\":\"Emilia\"}").unwrap();
    fs_err::write(non_empty_hashes.join("game.harvested.hashes.txt"), b"some hashes").unwrap();

    // 4. Mod with non-empty META, but EMPTY META/hashes (empty hashes folder is safe to delete)
    let garen_mod = active_target.join("Chun Li Garen v2.0");
    let empty_hashes = garen_mod.join("META").join("hashes");
    fs_err::create_dir_all(&empty_hashes).unwrap();
    fs_err::write(garen_mod.join("META").join("info.json"), b"{\"Name\":\"Chun Li\"}").unwrap();

    // 5. Empty Hematite-Fixed folder
    let hematite_dir = skins_dir.join("sub").join("Hematite-Fixed");
    fs_err::create_dir_all(&hematite_dir).unwrap();

    // Run cleanup
    let report = crate::cleanup::cleanup_empty_rose_folders(skins_dir).unwrap();

    assert_eq!(report.deleted_targets, 2);
    assert_eq!(report.deleted_mods, 1);
    assert_eq!(report.deleted_hematite_fixed, 1);
    assert_eq!(report.deleted_manifest_files, 2);
    assert_eq!(report.deleted_empty_subdirs, 1);

    // Verify orphan folders and empty mod folders are gone
    assert!(!orphan1.exists(), "Orphan target 105000 must be deleted");
    assert!(!orphan2.exists(), "Orphan target 13000 must be deleted");
    assert!(!empty_mod.exists(), "Empty mod folder must be deleted");
    assert!(!hematite_dir.exists(), "Empty Hematite-Fixed must be deleted");

    // CRITICAL CHECKS:
    // 1. Folders with files inside are NEVER deleted:
    assert!(non_empty_mod.join("META").exists(), "META with files must NEVER be deleted");
    assert!(non_empty_hashes.exists(), "META/hashes with files must NEVER be deleted");
    assert!(non_empty_hashes.join("game.harvested.hashes.txt").exists(), "Hash files preserved");

    // 2. Empty META/hashes is deleted, but its parent META (containing info.json) is PRESERVED:
    assert!(!empty_hashes.exists(), "Empty META/hashes folder must be deleted");
    assert!(garen_mod.join("META").exists(), "META containing info.json must be preserved");
    assert!(garen_mod.join("META").join("info.json").exists(), "info.json preserved");

    // Verify active target folder and mod are preserved
    assert!(active_target.exists(), "Active target folder must be preserved");
    assert!(active_mod.exists(), "Active mod folder must be preserved");
    assert!(active_target.join("rose_mod_targets.json").exists(), "Manifest in active target preserved");
}


#[test]
fn test_generate_skin_mappings_sync() {
    let temp = TempDir::new().unwrap();
    let skins_dir = temp.path();

    // Setup folder 106000 (Volibear) with 2 mods
    let voli_dir = skins_dir.join("106000");
    let mod1 = voli_dir.join("Angel Volibear v1.0");
    let mod2 = voli_dir.join("Tank Volibear v1.0");
    fs_err::create_dir_all(&mod1).unwrap();
    fs_err::create_dir_all(&mod2).unwrap();
    fs_err::write(mod1.join("a.txt"), b"a").unwrap();
    fs_err::write(mod2.join("b.txt"), b"b").unwrap();

    // Setup folder 33000 (Rammus) with 1 mod
    let rammus_dir = skins_dir.join("33000");
    let mod3 = rammus_dir.join("Sonic Rammus v1.0");
    fs_err::create_dir_all(&mod3).unwrap();
    fs_err::write(mod3.join("c.txt"), b"c").unwrap();

    // Generate mappings
    let summary = crate::mapping::generate_skin_mappings(skins_dir).unwrap().expect("Summary returned");
    assert_eq!(summary.total_folders, 2);
    assert_eq!(summary.total_mods, 3);

    let txt_path = skins_dir.join(crate::mapping::MAPPING_TXT_FILENAME);
    let json_path = skins_dir.join(crate::mapping::MAPPING_JSON_FILENAME);

    assert!(txt_path.exists());
    assert!(json_path.exists());

    let txt_content = fs_err::read_to_string(&txt_path).unwrap();
    assert!(txt_content.contains("Rammus"));
    assert!(txt_content.contains("Volibear"));
    assert!(txt_content.contains("Sonic Rammus v1.0"));
    assert!(txt_content.contains("Angel Volibear v1.0"));
    assert!(txt_content.contains("Tank Volibear v1.0"));

    let json_content = fs_err::read_to_string(&json_path).unwrap();
    let manifest: crate::mapping::MappingFileManifest = serde_json::from_str(&json_content).unwrap();
    assert_eq!(manifest.total_folders, 2);
    assert_eq!(manifest.total_mods, 3);
    assert_eq!(manifest.mappings[0].folder, "33000");
    assert_eq!(manifest.mappings[0].champion, "Rammus");
    assert_eq!(manifest.mappings[0].is_base_skin, true);
    assert_eq!(manifest.mappings[0].mods, vec!["Sonic Rammus v1.0"]);

    assert_eq!(manifest.mappings[1].folder, "106000");
    assert_eq!(manifest.mappings[1].champion, "Volibear");
    assert_eq!(manifest.mappings[1].is_base_skin, true);
    assert_eq!(manifest.mappings[1].mods, vec!["Angel Volibear v1.0", "Tank Volibear v1.0"]);

    // Test synchronization: Remove folder 33000
    fs_err::remove_dir_all(&rammus_dir).unwrap();

    // Re-run mapping generation
    let summary2 = crate::mapping::generate_skin_mappings(skins_dir).unwrap().expect("Summary returned");
    assert_eq!(summary2.total_folders, 1);
    assert_eq!(summary2.total_mods, 2);

    let txt_content2 = fs_err::read_to_string(&txt_path).unwrap();
    assert!(!txt_content2.contains("Rammus"), "Deleted champion Rammus must be removed from txt");
    assert!(!txt_content2.contains("33000"), "Deleted folder 33000 must be removed from txt");
    assert!(txt_content2.contains("Volibear"));

    let json_content2 = fs_err::read_to_string(&json_path).unwrap();
    let manifest2: crate::mapping::MappingFileManifest = serde_json::from_str(&json_content2).unwrap();
    assert_eq!(manifest2.total_folders, 1);
    assert_eq!(manifest2.mappings[0].folder, "106000");
}

#[test]
fn test_migrate_outdated_rose_directory() {
    use std::io::Write;
    let temp = TempDir::new().unwrap();
    let skins_dir = temp.path();

    fn create_mock_zip(path: &std::path::Path, files: &[(&str, &[u8])]) {
        let file = fs_err::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
    }

    // Target 24000 (Jax): Contains raw Blood King Jax.fantome with standard structure
    let target_24000 = skins_dir.join("24000");
    fs_err::create_dir_all(&target_24000).unwrap();
    let jax_archive = target_24000.join("Blood King Jax.fantome");
    create_mock_zip(
        &jax_archive,
        &[
            ("META/info.json", b"{\"Name\":\"Blood King Jax\"}"),
            ("WAD/Jax.wad.client", b"wad-bytes-jax"),
        ],
    );

    // Target 86000 (Garen): Contains raw Escanor Garen.zip with single-root wrapper
    let target_86000 = skins_dir.join("86000");
    fs_err::create_dir_all(&target_86000).unwrap();
    let garen_archive = target_86000.join("Escanor Garen.zip");
    create_mock_zip(
        &garen_archive,
        &[
            ("Escanor Garen/META/info.json", b"{\"Name\":\"Escanor Garen\"}"),
            ("Escanor Garen/WAD/Garen.wad.client", b"wad-bytes-garen"),
        ],
    );

    // Target 106000 (Volibear): ALREADY modern extracted folder (must NOT be touched)
    let target_106000 = skins_dir.join("106000");
    let voli_mod = target_106000.join("Angel Volibear v1.0");
    fs_err::create_dir_all(voli_mod.join("META")).unwrap();
    fs_err::write(voli_mod.join("META").join("info.json"), b"{\"Name\":\"Angel Volibear\"}").unwrap();
    fs_err::write(target_106000.join("rose_mod_targets.json"), b"{\"version\":1,\"championId\":106,\"targets\":[106000],\"mods\":{}}").unwrap();

    // Verify detection
    assert!(crate::migrate::is_rose_skins_directory(skins_dir));

    // Run migration
    let report = crate::migrate::migrate_outdated_rose_directory(skins_dir).unwrap();
    assert_eq!(report.extracted_archives, 2);
    assert_eq!(report.manifests_rebuilt, 3);

    // 1. Check Jax extraction
    assert!(!jax_archive.exists(), "Raw Jax archive must be deleted");
    let jax_extracted = target_24000.join("Blood King Jax");
    assert!(jax_extracted.is_dir());
    assert!(jax_extracted.join("META").join("info.json").exists());
    assert!(jax_extracted.join("WAD").join("Jax.wad.client").exists());

    let jax_manifest_path = target_24000.join("rose_mod_targets.json");
    assert!(jax_manifest_path.exists());
    let jax_manifest: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&jax_manifest_path).unwrap()).unwrap();
    assert_eq!(jax_manifest["championId"], 24);
    assert_eq!(jax_manifest["targets"], serde_json::json!([24000]));
    let jax_mods = jax_manifest["mods"].as_object().unwrap();
    assert_eq!(jax_mods.len(), 1);
    let jax_entry = jax_mods.values().next().unwrap();
    assert_eq!(jax_entry["name"], "Blood King Jax");

    // 2. Check Garen extraction and single-root flattening
    assert!(!garen_archive.exists(), "Raw Garen archive must be deleted");
    let garen_extracted = target_86000.join("Escanor Garen");
    assert!(garen_extracted.is_dir());
    // Direct child must be META and WAD, NOT an extra Escanor Garen wrapper
    assert!(garen_extracted.join("META").join("info.json").exists());
    assert!(garen_extracted.join("WAD").join("Garen.wad.client").exists());
    assert!(!garen_extracted.join("Escanor Garen").exists(), "Wrapper folder must be flattened");

    let garen_manifest_path = target_86000.join("rose_mod_targets.json");
    assert!(garen_manifest_path.exists());
    let garen_manifest: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&garen_manifest_path).unwrap()).unwrap();
    assert_eq!(garen_manifest["championId"], 86);
    assert_eq!(garen_manifest["targets"], serde_json::json!([86000]));

    // 3. Check Volibear (already modern) remained untouched
    assert!(voli_mod.exists());

    // 4. Idempotency test: Re-running migration on the now-modern structure extracts 0 archives and rebuilds 0 manifests
    let report2 = crate::migrate::migrate_outdated_rose_directory(skins_dir).unwrap();
    assert_eq!(report2.extracted_archives, 0);
    assert_eq!(report2.manifests_rebuilt, 0);
}

#[test]
fn test_modpkg_migration_and_extraction() {
    let modpkg_source = std::path::Path::new(
        r"C:\Users\Admin\Downloads\Scripts\Modpkg-to-Fantome\Escanor Nasus.modpkg",
    );
    if !modpkg_source.is_file() {
        // Skip on environments without the fixture
        return;
    }

    let temp = TempDir::new().unwrap();
    let skins_dir = temp.path().join("skins");
    let target_75000 = skins_dir.join("75000");
    fs_err::create_dir_all(&target_75000).unwrap();

    let modpkg_dest = target_75000.join("Escanor Nasus.modpkg");
    fs_err::copy(modpkg_source, &modpkg_dest).unwrap();

    // Verify detection
    assert!(crate::migrate::is_rose_skins_directory(&skins_dir));

    // Migrate
    let report = crate::migrate::migrate_outdated_rose_directory(&skins_dir).unwrap();
    assert_eq!(report.extracted_archives, 1);
    assert_eq!(report.manifests_rebuilt, 1);

    // Verify .modpkg is deleted
    assert!(!modpkg_dest.exists(), "Original .modpkg file must be unlinked");

    // Verify extracted structure
    let extracted = target_75000.join("Escanor Nasus");
    assert!(extracted.is_dir());
    assert!(extracted.join("META").join("info.json").exists());
    assert!(extracted.join("WAD").exists());

    // Verify manifest
    let manifest_path = target_75000.join("rose_mod_targets.json");
    assert!(manifest_path.exists());
    let manifest: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["championId"], 75);
    assert_eq!(manifest["targets"], serde_json::json!([75000]));
    let mods = manifest["mods"].as_object().unwrap();
    assert_eq!(mods.len(), 1);
}

#[test]
fn test_corrupted_archive_detected_as_broken() {
    let temp = TempDir::new().unwrap();
    let corrupt_file = temp.path().join("draven-shaco-thrower_1.0.0.fantome");

    // Write invalid/corrupt zip content with broken entry
    let mut bad_bytes = vec![0x50, 0x4B, 0x03, 0x04]; // PK\x03\x04
    bad_bytes.extend_from_slice(&[0u8; 100]); // truncated/invalid header
    fs_err::write(&corrupt_file, &bad_bytes).unwrap();

    let config = dummy_config();
    let report = check_mod_health(&corrupt_file, &config).unwrap();
    assert_eq!(report.status, ModHealthStatus::Broken);
    assert!(report.reason.is_some());
}

#[test]
fn test_delete_unrepairable_and_update_manifests() {
    let temp = TempDir::new().unwrap();
    let skins_dir = temp.path().join("skins");
    let target_dir = skins_dir.join("119000"); // Draven
    fs_err::create_dir_all(&target_dir).unwrap();

    // 1. Create a corrupted/broken archive
    let corrupt_file = target_dir.join("draven-shaco-thrower_1.0.0.fantome");
    fs_err::write(&corrupt_file, b"corrupted-non-zip-data").unwrap();

    // 2. Create a healthy mod folder alongside it
    let healthy_archive = temp.path().join("Gladiator Draven.fantome");
    make_packed_bin_fantome_zip(
        &healthy_archive,
        "Gladiator Draven",
        &healthy_bin(),
        zip::CompressionMethod::Stored,
    );
    let healthy_mod = target_dir.join("Gladiator Draven v1.0");
    crate::formats::unpack_fantome_archive(&healthy_archive, &healthy_mod).unwrap();

    // Build initial Rose manifest
    let _ = crate::migrate::rebuild_target_manifest(&target_dir, 119000).unwrap();
    assert!(target_dir.join("rose_mod_targets.json").exists());

    // Execute repair with delete_unrepairable: true
    let printer = crate::output::Printer::new(false, false);
    let _ = crate::engine::execute_repair(
        Some(skins_dir.clone()),
        None,
        None,
        false,
        false,
        true,
        true,
        true,
        true,
        false,
        false, // quarantine
        true,  // delete_unrepairable
        &printer,
    ).unwrap();

    // Corrupted file should be deleted from disk
    assert!(!corrupt_file.exists(), "Corrupted archive must be deleted");

    // Healthy mod should still exist
    assert!(healthy_mod.exists(), "Healthy mod must be preserved");

    // Manifest must be preserved and contain only healthy mod
    let manifest: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(target_dir.join("rose_mod_targets.json")).unwrap()).unwrap();
    let mods = manifest["mods"].as_object().unwrap();
    assert_eq!(mods.len(), 1);

    // Mappings must only contain Gladiator Draven
    let summary = crate::mapping::generate_skin_mappings(&skins_dir).unwrap().unwrap();
    assert_eq!(summary.total_mods, 1);
}

#[test]
fn test_quarantine_unrepairable_and_update_manifests() {
    let temp = TempDir::new().unwrap();
    let skins_dir = temp.path().join("skins");
    let target_dir = skins_dir.join("119000"); // Draven
    fs_err::create_dir_all(&target_dir).unwrap();

    // 1. Create a corrupted/broken archive
    let corrupt_file = target_dir.join("draven-shaco-thrower_1.0.0.fantome");
    fs_err::write(&corrupt_file, b"corrupted-invalid-checksum-zip").unwrap();

    // 2. Create a healthy mod folder alongside it
    let healthy_archive = temp.path().join("Gladiator Draven.fantome");
    make_packed_bin_fantome_zip(
        &healthy_archive,
        "Gladiator Draven",
        &healthy_bin(),
        zip::CompressionMethod::Stored,
    );
    let healthy_mod = target_dir.join("Gladiator Draven v1.0");
    crate::formats::unpack_fantome_archive(&healthy_archive, &healthy_mod).unwrap();

    // Build initial Rose manifest
    let _ = crate::migrate::rebuild_target_manifest(&target_dir, 119000).unwrap();
    assert!(target_dir.join("rose_mod_targets.json").exists());

    // Execute repair with quarantine: true, delete_unrepairable: false (default behavior)
    let printer = crate::output::Printer::new(false, false);
    let _ = crate::engine::execute_repair(
        Some(skins_dir.clone()),
        None,
        None,
        false,
        false,
        true,
        true,
        true,
        true,
        false,
        true,  // quarantine
        false, // delete_unrepairable
        &printer,
    ).unwrap();

    // Corrupted file must NO LONGER be in target_dir (119000)
    assert!(!corrupt_file.exists(), "Corrupted archive must be removed from target directory");

    // Corrupted file must be safely preserved in .broken/119000/
    let broken_file = skins_dir
        .join(".broken")
        .join("119000")
        .join("draven-shaco-thrower_1.0.0.fantome");
    assert!(broken_file.exists(), "Corrupted archive must be safely quarantined into .broken/");

    // Healthy mod should still exist in 119000
    assert!(healthy_mod.exists(), "Healthy mod must be preserved");

    // Manifest must contain only healthy mod
    let manifest: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(target_dir.join("rose_mod_targets.json")).unwrap()).unwrap();
    let mods = manifest["mods"].as_object().unwrap();
    assert_eq!(mods.len(), 1);

    // Mappings must only contain Gladiator Draven and completely ignore .broken
    let summary = crate::mapping::generate_skin_mappings(&skins_dir).unwrap().unwrap();
    assert_eq!(summary.total_mods, 1);

    // Rescan must not scan .broken directory
    let rescan = crate::scanner::scan_directory(&skins_dir, true).unwrap();
    assert_eq!(rescan.len(), 1);
    assert_eq!(rescan[0], healthy_mod);
}

#[test]
fn test_cli_subcommands_accept_pause_flags() {
    use clap::Parser;
    use crate::cli::{Cli, Commands};

    // Test repair --pause
    let args1 = ["lol-mod-fixer", "repair", "--pause"];
    let cli1 = Cli::try_parse_from(args1).expect("repair --pause should be valid");
    match cli1.command {
        Some(Commands::Repair(args)) => assert!(args.pause),
        _ => panic!("Expected Commands::Repair"),
    }

    // Test check --pause
    let args2 = ["lol-mod-fixer", "check", "--pause"];
    let cli2 = Cli::try_parse_from(args2).expect("check --pause should be valid");
    match cli2.command {
        Some(Commands::Check(args)) => assert!(args.pause),
        _ => panic!("Expected Commands::Check"),
    }

    // Test auto --pause
    let args3 = ["lol-mod-fixer", "auto", "--pause"];
    let cli3 = Cli::try_parse_from(args3).expect("auto --pause should be valid");
    match cli3.command {
        Some(Commands::Auto(args)) => assert!(args.pause),
        _ => panic!("Expected Commands::Auto"),
    }

    // Test repair --no-pause
    let args4 = ["lol-mod-fixer", "repair", "--no-pause"];
    let cli4 = Cli::try_parse_from(args4).expect("repair --no-pause should be valid");
    match cli4.command {
        Some(Commands::Repair(args)) => assert!(args.no_pause),
        _ => panic!("Expected Commands::Repair"),
    }
}

#[test]
fn test_cleanup_empty_folders_with_desktop_ini() {
    let temp = tempfile::tempdir().unwrap();
    let skins_dir = temp.path().join("skins");
    fs_err::create_dir_all(&skins_dir).unwrap();

    // 1. Target directory 82000 containing only desktop.ini
    let target_82000 = skins_dir.join("82000");
    fs_err::create_dir_all(&target_82000).unwrap();
    fs_err::write(target_82000.join("desktop.ini"), "[.ShellClassInfo]\r\nIconResource=icon.ico,0").unwrap();

    // 2. Target directory 106000 containing a valid mod folder
    let target_106000 = skins_dir.join("106000");
    let valid_mod = target_106000.join("Angel Volibear v1.0");
    fs_err::create_dir_all(valid_mod.join("META")).unwrap();
    fs_err::write(valid_mod.join("META").join("info.json"), "{}").unwrap();
    fs_err::create_dir_all(valid_mod.join("WAD")).unwrap();
    fs_err::write(valid_mod.join("WAD").join("volibear.wad.client"), "dummy").unwrap();

    // 3. Mod directory containing only desktop.ini inside 106000
    let junk_mod = target_106000.join("Empty Junk Mod");
    fs_err::create_dir_all(&junk_mod).unwrap();
    fs_err::write(junk_mod.join("desktop.ini"), "junk").unwrap();

    let report = crate::cleanup::cleanup_empty_rose_folders(&skins_dir).unwrap();

    // 82000 should be deleted completely
    assert!(!target_82000.exists(), "Target 82000 containing only desktop.ini must be purged");
    assert_eq!(report.deleted_targets, 1, "Should report 1 deleted target");

    // Junk mod inside 106000 should be deleted
    assert!(!junk_mod.exists(), "Empty mod folder with desktop.ini must be purged");
    assert_eq!(report.deleted_mods, 1, "Should report 1 deleted empty mod");

    // Valid mod should remain untouched
    assert!(valid_mod.exists(), "Valid mod folder must be preserved");
}

#[test]
fn test_rose_manifest_sync_when_mods_pasted() {
    let temp = tempfile::tempdir().unwrap();
    let skins_dir = temp.path().join("skins");
    let target_102000 = skins_dir.join("102000");
    fs_err::create_dir_all(&target_102000).unwrap();

    // Create Mod 1: Kaido Shyvana v1.0
    let mod1 = target_102000.join("Kaido Shyvana v1.0");
    fs_err::create_dir_all(mod1.join("META")).unwrap();
    fs_err::write(mod1.join("META").join("info.json"), "{}").unwrap();
    fs_err::create_dir_all(mod1.join("WAD")).unwrap();
    fs_err::write(mod1.join("WAD").join("shyvana.wad.client"), "wad1").unwrap();

    // Build initial manifest for just Mod 1
    crate::migrate::rebuild_target_manifest(&target_102000, 102000).unwrap();

    // Verify initial manifest has 1 mod
    let manifest_path = target_102000.join("rose_mod_targets.json");
    let manifest_content = fs_err::read_to_string(&manifest_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&manifest_content).unwrap();
    assert_eq!(parsed["mods"].as_object().unwrap().len(), 1);

    // Simulate user copying/pasting 2 more mods from another directory into 102000
    let mod2 = target_102000.join("Blossom Lizard Shyvana");
    fs_err::create_dir_all(mod2.join("META")).unwrap();
    fs_err::write(mod2.join("META").join("info.json"), "{}").unwrap();
    fs_err::create_dir_all(mod2.join("WAD")).unwrap();
    fs_err::write(mod2.join("WAD").join("shyvana.wad.client"), "wad2").unwrap();

    let mod3 = target_102000.join("Wild Rift Shyvana");
    fs_err::create_dir_all(mod3.join("META")).unwrap();
    fs_err::write(mod3.join("META").join("info.json"), "{}").unwrap();
    fs_err::create_dir_all(mod3.join("WAD")).unwrap();
    fs_err::write(mod3.join("WAD").join("shyvana.wad.client"), "wad3").unwrap();

    // Now run sync_rose_target_manifests
    let synced = crate::migrate::sync_rose_target_manifests(&skins_dir).unwrap();
    assert_eq!(synced, 1, "Should detect out-of-sync manifest and rebuild it");

    // Read updated manifest
    let updated_content = fs_err::read_to_string(&manifest_path).unwrap();
    let updated: serde_json::Value = serde_json::from_str(&updated_content).unwrap();
    let mods = updated["mods"].as_object().unwrap();

    // Must now contain all 3 mods!
    assert_eq!(mods.len(), 3, "Manifest must now contain all 3 mods");
    let mod_names: Vec<&str> = mods.values().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(mod_names.contains(&"Kaido Shyvana v1.0"));
    assert!(mod_names.contains(&"Blossom Lizard Shyvana"));
    assert!(mod_names.contains(&"Wild Rift Shyvana"));

    // Running sync again should do nothing because it's already in sync
    let synced_again = crate::migrate::sync_rose_target_manifests(&skins_dir).unwrap();
    assert_eq!(synced_again, 0, "Second sync must be a no-op since manifest is up to date");
}

#[test]
fn test_incremental_rose_manifest_preserves_hashes_and_prunes() {
    let temp = tempfile::tempdir().unwrap();
    let target_102000 = temp.path().join("102000");
    fs_err::create_dir_all(&target_102000).unwrap();

    // Create Mod 1
    let mod1 = target_102000.join("Mod Alpha");
    fs_err::create_dir_all(mod1.join("META")).unwrap();
    fs_err::write(mod1.join("META").join("info.json"), "{}").unwrap();
    fs_err::create_dir_all(mod1.join("WAD")).unwrap();
    fs_err::write(mod1.join("WAD").join("alpha.wad.client"), "content_alpha").unwrap();

    // Initial build
    let rebuilt = crate::migrate::rebuild_target_manifest(&target_102000, 102000).unwrap();
    assert!(rebuilt);

    let manifest_path = target_102000.join("rose_mod_targets.json");
    let initial_json: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&manifest_path).unwrap()).unwrap();
    let mod1_entry = initial_json["mods"]
        .as_object()
        .unwrap()
        .values()
        .find(|v| v["name"] == "Mod Alpha")
        .unwrap()
        .clone();
    let initial_mod1_hash = mod1_entry["folderHash"].as_str().unwrap().to_string();

    // Rebuild with no changes -> must return Ok(false) immediately
    let no_change = crate::migrate::rebuild_target_manifest(&target_102000, 102000).unwrap();
    assert!(!no_change, "Rebuild with zero changes must return false (no-op)");

    // Paste Mod 2 and Mod 3
    let mod2 = target_102000.join("Mod Beta");
    fs_err::create_dir_all(mod2.join("META")).unwrap();
    fs_err::write(mod2.join("META").join("info.json"), "{}").unwrap();
    fs_err::create_dir_all(mod2.join("WAD")).unwrap();
    fs_err::write(mod2.join("WAD").join("beta.wad.client"), "content_beta").unwrap();

    let mod3 = target_102000.join("Mod Gamma");
    fs_err::create_dir_all(mod3.join("META")).unwrap();
    fs_err::write(mod3.join("META").join("info.json"), "{}").unwrap();
    fs_err::create_dir_all(mod3.join("WAD")).unwrap();
    fs_err::write(mod3.join("WAD").join("gamma.wad.client"), "content_gamma").unwrap();

    // Rebuild manifest -> must incrementally append new mods
    let rebuilt_new = crate::migrate::rebuild_target_manifest(&target_102000, 102000).unwrap();
    assert!(rebuilt_new, "Rebuild with new mods must update manifest");

    let updated_json: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&manifest_path).unwrap()).unwrap();
    let updated_mods = updated_json["mods"].as_object().unwrap();
    assert_eq!(updated_mods.len(), 3);

    // Verify Mod 1 preserved its exact original hash without recalculation
    let preserved_mod1 = updated_mods
        .values()
        .find(|v| v["name"] == "Mod Alpha")
        .unwrap();
    assert_eq!(
        preserved_mod1["folderHash"].as_str().unwrap(),
        initial_mod1_hash,
        "Mod Alpha's hash must be preserved incrementally"
    );

    // Running again without changes must be false
    assert!(!crate::migrate::rebuild_target_manifest(&target_102000, 102000).unwrap());

    // Delete Mod 2 from disk -> Rebuild must prune it
    fs_err::remove_dir_all(&mod2).unwrap();
    let pruned = crate::migrate::rebuild_target_manifest(&target_102000, 102000).unwrap();
    assert!(pruned, "Deleting a mod from disk must trigger manifest update");

    let pruned_json: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&manifest_path).unwrap()).unwrap();
    let pruned_mods = pruned_json["mods"].as_object().unwrap();
    assert_eq!(pruned_mods.len(), 2);
    let names: Vec<&str> = pruned_mods.values().map(|v| v["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"Mod Alpha"));
    assert!(names.contains(&"Mod Gamma"));
    assert!(!names.contains(&"Mod Beta"));
}

#[test]
fn test_raw_fantome_folder_detection_and_beautification() {
    let temp = TempDir::new().unwrap();
    let skin_dir = temp.path().join("102000");
    fs_err::create_dir_all(&skin_dir).unwrap();

    // Create Blossom Lizard Shyvana with META and RAW
    let mod_dir = skin_dir.join("Blossom Lizard Shyvana");
    fs_err::create_dir_all(mod_dir.join("META")).unwrap();
    fs_err::write(
        mod_dir.join("META").join("info.json"),
        r#"{ "Name": "Shyvana", "Author": "Abdomera", "Version": "1.0" }"#,
    )
    .unwrap();
    fs_err::create_dir_all(mod_dir.join("RAW").join("ASSETS").join("Characters").join("Shyvana")).unwrap();
    fs_err::write(
        mod_dir.join("RAW").join("ASSETS").join("Characters").join("Shyvana").join("test.dds"),
        "dummy dds bytes",
    )
    .unwrap();

    // Verify format detection recognizes it as FantomeFolder
    assert!(crate::formats::is_fantome_folder(&mod_dir));
    assert_eq!(crate::formats::ModFormat::detect(&mod_dir), crate::formats::ModFormat::FantomeFolder);

    // Run beautify_and_sync_folder
    let new_path = crate::beautify::beautify_and_sync_folder(&mod_dir).unwrap();
    assert!(new_path.is_some());
    let beautified_path = new_path.unwrap();
    assert_eq!(
        beautified_path.file_name().unwrap().to_string_lossy(),
        "Blossom Lizard Shyvana v1.0"
    );
    assert!(beautified_path.exists());
    assert!(!mod_dir.exists());

    // Also test Fenrir Warwick
    let ww_skin_dir = temp.path().join("19000");
    fs_err::create_dir_all(&ww_skin_dir).unwrap();
    let ww_mod = ww_skin_dir.join("Fenrir Warwick");
    fs_err::create_dir_all(ww_mod.join("META")).unwrap();
    fs_err::write(
        ww_mod.join("META").join("info.json"),
        r#"{ "Name": "Warwick Fenrir", "Author": "Abdomera", "Version": "1.0" }"#,
    )
    .unwrap();
    fs_err::create_dir_all(ww_mod.join("RAW")).unwrap();
    fs_err::write(ww_mod.join("RAW").join("data.bin"), "data").unwrap();

    assert!(crate::formats::is_fantome_folder(&ww_mod));
    let ww_new = crate::beautify::beautify_and_sync_folder(&ww_mod).unwrap().unwrap();
    assert_eq!(
        ww_new.file_name().unwrap().to_string_lossy(),
        "Fenrir Warwick v1.0"
    );

    // Also test Nude Morgana
    let morg_skin_dir = temp.path().join("25000");
    fs_err::create_dir_all(&morg_skin_dir).unwrap();
    let morg_mod = morg_skin_dir.join("Nude Morgana");
    fs_err::create_dir_all(morg_mod.join("META")).unwrap();
    fs_err::write(
        morg_mod.join("META").join("info.json"),
        r#"{ "Name": "Base Morgana Female", "Author": "Onothera", "Version": "1.0" }"#,
    )
    .unwrap();
    fs_err::create_dir_all(morg_mod.join("RAW")).unwrap();
    fs_err::write(morg_mod.join("RAW").join("data.bin"), "data").unwrap();

    assert!(crate::formats::is_fantome_folder(&morg_mod));
    let morg_new = crate::beautify::beautify_and_sync_folder(&morg_mod).unwrap().unwrap();
    assert_eq!(
        morg_new.file_name().unwrap().to_string_lossy(),
        "Nude Morgana v1.0"
    );
}

#[test]
fn test_non_mod_files_and_documents_ignored_and_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let docs_dir = temp.path().join("Documents");
    fs_err::create_dir_all(&docs_dir).unwrap();

    // 1. Create a simulated docx file (which is a zip internally)
    let docx_path = docs_dir.join("Endorsement-for-Defense.docx");
    {
        let file = fs_err::File::create(&docx_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("word/document.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, b"<xml>Thesis defense</xml>").unwrap();
        zip.finish().unwrap();
    }

    // 2. Create generic other non-mod files
    let pdf_path = docs_dir.join("Guide.pdf");
    fs_err::write(&pdf_path, b"%PDF-1.4 simulated pdf").unwrap();

    let generic_zip = docs_dir.join("homework.zip");
    {
        let file = fs_err::File::create(&generic_zip).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("notes.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, b"Study notes").unwrap();
        zip.finish().unwrap();
    }

    // Verify format detection explicitly rejects all of them
    assert_eq!(
        crate::formats::ModFormat::detect(&docx_path),
        crate::formats::ModFormat::Unsupported
    );
    assert_eq!(
        crate::formats::ModFormat::detect(&pdf_path),
        crate::formats::ModFormat::Unsupported
    );
    assert_eq!(
        crate::formats::ModFormat::detect(&generic_zip),
        crate::formats::ModFormat::Unsupported
    );

    // Verify candidate file detection rejects all of them
    assert!(!crate::scanner::is_candidate_file(&docx_path));
    assert!(!crate::scanner::is_candidate_file(&pdf_path));
    assert!(!crate::scanner::is_candidate_file(&generic_zip));

    // Verify scanner finds 0 mods in this directory
    let found = crate::scanner::scan_directory(&docs_dir, true).unwrap();
    assert!(found.is_empty(), "Expected 0 candidates, found: {:?}", found);
}

#[test]
fn test_lock_or_permission_error_detection_and_tracking() {
    use crate::permissions::is_lock_or_permission_error;
    use crate::scanner::{get_restricted_count, record_restricted_file, reset_restricted_count};

    assert!(is_lock_or_permission_error("os error 5: Access is denied"));
    assert!(is_lock_or_permission_error("Failed to open file: Access is denied (os error 5)"));
    assert!(is_lock_or_permission_error("os error 32: The process cannot access the file because it is being used by another process"));
    assert!(is_lock_or_permission_error("Permission denied (os error 13)"));
    assert!(is_lock_or_permission_error("sharing violation"));
    assert!(is_lock_or_permission_error("File is locked"));
    assert!(!is_lock_or_permission_error("File not found (os error 2)"));
    assert!(!is_lock_or_permission_error("Corrupted zip archive: bad CRC"));

    reset_restricted_count();
    assert_eq!(get_restricted_count(), 0);
    record_restricted_file();
    record_restricted_file();
    assert_eq!(get_restricted_count(), 2);
    reset_restricted_count();
    assert_eq!(get_restricted_count(), 0);
}






