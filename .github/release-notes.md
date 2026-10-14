## What's New in v0.2.7

* **Rose Category Manifest Precision & Stray Manifest Pruning**:
  - Fixed category manifest scanner to register all mod subfolders regardless of individual sub-file ACL restrictions, accurately registering all map mods (`Hell Rift`, `Shurima Rift`, `Spirit Blossom Rift`, `Winter Rift`).
  - Completely prevents generating empty `rose_category_mods.json` manifests in empty category folders (e.g. `fonts`, `announcers`, `loading_screen`, `ui`).
  - Automatically detects and prunes any stray or empty `rose_category_mods.json` files from unpopulated category directories.
  - Consolidated startup migration output into a single unified summary.

## What's New in v0.2.6

* **Rose Category Mods Support & Simultaneous Synchronization**:
  - Full discovery, migration, and reconciliation for non-skin Rose categories (`maps`, `loading_screen`, `announcers`, `fonts`, `others`, `sfx`, `ui`, `vfx`, `voiceover`, `hud`, `audio`, and custom category folders).
  - Automatically migrates and extracts unextracted `.fantome`, `.zip`, and `.modpkg` archives placed directly inside category folders into the modern Rose layout (`<Mod Name>/META/info.json` + `WAD/...`).
  - Simultaneously synchronizes both `rose_category_mods.json` and `rose_mod_targets.json` concurrently in a single run when scanning from `%LOCALAPPDATA%\Rose\mods`.
  - Atomic, idempotent manifest writes prevent file corruption if interrupted.
  - Automatically updates `rose_category_mods.json` when category mods are beautified, deleted, or quarantined to `.broken/`.

* **Production Optimized Release Profile**:
  - Maximum compilation optimization (`opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `panic = "abort"`, and binary symbol stripping) yielding minimal binary size and instantaneous CLI startup.

## What's New in v0.2.5

* **Native `.modpkg` Title Beautification & In-Place Renaming**:
  - Full support for inspecting `.modpkg` packages in-memory via `ltk_modpkg::Modpkg` to extract WAD names and project metadata without disk extraction.
  - Automatically renames and beautifies standalone `.modpkg` files into canonical `<Skin Name> <Champion Name> v<Version>.modpkg` formatting in both single-file mode and full directory scans.
  - Corrupted or invalid `.modpkg` files safely bypass renaming to ensure accurate health checks and quarantine handling.

* **Fuzzy & Prefix Champion Token Deduplication**:
  - Implemented prefix and Levenshtein edit distance ($\le 1$) matching to recognize truncated or slightly misspelled champion names in mod titles (e.g. `Hecari` for `Hecarim`, `Nasu` for `Nasus`, `Aatro` for `Aatrox`).
  - Automatically strips duplicate champion occurrences (e.g. `Abyss Cosmic Hecari Hecari v0.1` $\rightarrow$ `Abyss Cosmic Hecarim v0.1` and `Abyss Cosmic Hecari Hecarim v0.1` $\rightarrow$ `Abyss Cosmic Hecarim v0.1`) without duplicating champion tokens in the final title.

* **Chained & Stacked Extension Sanitization**:
  - Added recursive archive extension stripping to clean stacked or chained extensions (e.g. `Mod v1.0.fantome.fantome`, `Mod.zip.fantome`, `Mod.modpkg.modpkg`) down to a single clean archive extension.

## What's New in v0.2.4

* **Native `.modpkg` Health Inspection & In-Place Repair**:
  - Full support for inspecting `.modpkg` archives using LTK's native `ModpkgImporter`.
  - Healthy `.modpkg` archives are accurately diagnosed as `HEALTHY / UNCHANGED` and left completely untouched (preventing false quarantine to `.broken/`).
  - Repairable `.modpkg` archives are safely repaired and repacked back into valid `.modpkg` files in-place via `ModpkgFormat`.
  - Corrupted `.modpkg` archives fail gracefully with precise error reporting instead of generic unsupported messages.

* **Multi-Rose & Nested Directory Auto-Discovery**:
  - Deep recursive discovery finds all Rose installations across complex directory structures (e.g. `wooshh/skins`, `pepperpotts/skins22`, `mods/skins`).
  - Seamlessly handles mixed directories containing loose archive files, standard folders, and multiple Rose skin directories side-by-side.
  - Automatically synchronizes `rose_mod_targets.json` across all discovered Rose skin roots.

* **Advanced Name Beautification & Title Sanitization**:
  - Enhanced title beautification for standalone archive files.
  - Automatic stripping of `default` and `base` keywords from skin titles.
  - Proper formatting for champion names wrapped in parentheses (e.g. `The Evoker (Yorick)` -> `The Evoker Yorick v1.0`).

* **Clean Relative Quarantine Paths**:
  - Quarantined files are now organized under clean, shortened relative subpaths within their local `.broken/` directory.

## What's New in v0.2.3

* **Skin Mappings Isolation**: Restricted `skin_mappings.txt` and `skin_mappings.json` generation strictly to the Rose `skins` root directory (`...\Rose\mods\skins`). When running the fixer against an individual numeric skin folder (e.g. `800000`, `950000`), it will no longer generate unwanted mapping files inside that folder.
* **Stray File Auto-Cleanup**: Automatically detects and deletes any stray or misplaced `skin_mappings.txt` / `skin_mappings.json` files previously generated inside individual numeric target folders.
* **Strict Numeric Skin Validation**: Updated mapping collector so it strictly considers valid numeric skin IDs, preventing mod subfolders (such as `Gojo Mel v1.0`) from ever being mistaken for skin folders.
* **Direct Latest Download Links**: Added permanent download badges and links in `README.md` pointing directly to the latest binary.

## What's New in v0.2.2

* **Pure ASCII Console Output**:
  - Replaced all Unicode checkmarks (`✓`), bullets (`•`), arrows (`→`), and failure crosses (`✗`) with standard ASCII tags (`[OK]`, `[+]`, `[-]`, `[X]`, `->`).
  - Completely eliminates broken question mark boxes (`?` / tofu) and raster font decoding issues across legacy Windows `cmd.exe`, PowerShell, and conhost terminals.
* **Instant Startup & Incremental Manifest Synchronization**:
  - Replaced redundant full-library SHA-256 disk re-hashing on startup with smart incremental synchronization.
  - Untouched mods reuse existing verified hashes directly from `rose_mod_targets.json` without reading gigabytes of files from disk.
  - Newly pasted or manually added mods are incrementally detected, hashed, and appended to the manifest.
  - Deleted mod folders are automatically pruned from `rose_mod_targets.json`.
  - Fixes the 30–60+ second silent freeze on startup on large mod libraries and systems running Windows Defender.
* **Immediate Real-Time Terminal Progress Feedback**:
  - Added real-time progress indicators immediately after the banner (`-> Checking Rose mod structure and manifests...`, `-> Scanning directory for mods...`, `[+] Found X mod(s). Initializing engine...`).
  - The console never sits silent or idle with a blinking cursor.
* **Optimized Release Binary Footprint**:
  - Enabled Thin LTO (`lto = "thin"`), `codegen-units = 1`, and binary symbol stripping (`strip = true`).
  - Shrunk binary size from 25.2 MB down to **19.7 MB** while retaining peak runtime speed and stability across all Windows 10/11 environments.

## What's New in v0.2.1

* **Instant Startup & Incremental Manifest Synchronization**:
  - Replaced redundant, unbuffered full-library SHA-256 disk re-hashing on startup with smart incremental synchronization.
  - Untouched mods reuse existing verified hashes directly from `rose_mod_targets.json` without reading gigabytes of files from disk.
  - Newly pasted or manually added mods are incrementally detected, hashed, and appended to the manifest.
  - Deleted mod folders are automatically pruned from `rose_mod_targets.json`.
  - Fixes the 30–60+ second silent freeze on startup on large mod libraries and systems running Windows Defender.
* **Immediate Real-Time Terminal Progress Feedback**:
  - Added real-time progress indicators immediately after the banner (`Checking Rose mod structure and manifests...`, `Scanning directory for mods...`, `Found X mod(s). Initializing engine...`).
  - The console never sits silent or idle with a blinking cursor.
* **Optimized Release Binary Footprint**:
  - Enabled Thin LTO (`lto = "thin"`), `codegen-units = 1`, and binary symbol stripping (`strip = true`).
  - Shrunk binary size from 25.2 MB down to **19.7 MB** while retaining peak runtime speed and stability across all Windows 10/11 environments.

## What's New in v0.2.0

* **Unified Standalone Binary**:
  - Dropped separate `-static` and `-dynamic` executables.
  - Now ships as a single zero-dependency standalone binary named **`lol-mod-fixer.exe`** with statically linked C runtime (`/MT`). Works everywhere on Windows 10/11 out-of-the-box without needing batch wrappers or external scripts.
* **Bulletproof Non-Mod File Protection**:
  - Replaced incomplete file extension blacklists with a strict allowlist. Non-mod files (such as Word documents `.docx`, Excel `.xlsx`, `.pdf`, `.mp3`, etc.) are completely ignored and protected from accidental inspection or quarantine.
  - Verified zip sniffing (`is_fantome_zip`) ensures generic non-mod zip archives are never misidentified as Fantome mods.
* **Intelligent File Lock & Permission Detection**:
  - Automatically identifies locked or restricted files and folders (`Access is denied`, `os error 5`/`32`, sharing violations) and displays clear instructions in scan alerts, post-repair summaries, and error messages prompting users to run as Administrator.
* **Loose RAW/DATA Asset Mod Support**:
  - Extracted mod folders and loose zip packages with `RAW/ASSETS` or `RAW/DATA` structures (texture overrides, NSFW mods, unpackaged zips) are fully detected, repaired, and beautified with proper champion names and versions.
* **Automatic Quarantine (`.broken/`)**:
  - Unrepairable or corrupted mods are safely isolated to `.broken/` and unlinked from `rose_mod_targets.json` and skin mappings so active skin directories contain only healthy, working mods.
* **Multi-Mod Pasting & Reconciliation**:
  - Automatically reconciles and synchronizes `rose_mod_targets.json` when multiple mods are manually copied or merged into existing skin target directories.
* **Deep System Cleanup**:
  - Purges empty directories and orphan target manifests even when containing hidden, read-only Windows system files like `desktop.ini`.
* **Dynamic Console Banner**:
  - Console header displays current version dynamically and automatically matches the underline border width.
* **Instant Startup**:
  - Eliminated process scanning overhead for zero-delay scans.

## What's New in v0.1.0

* **Headless LTK Engine**: Uses LTK Manager's real health and problems engine to inspect and repair BIN property mismatches, outdated references, and audio bank issues without requiring the Tauri desktop GUI.
* **Full Mod Format Support**:
  - Extracted mod folders (`META/` + `WAD/`).
  - Loose **RAW/DATA asset mods** (e.g. custom texture overrides, unpackaged zips with `RAW/ASSETS` or `RAW/DATA` trees) with full version extraction and champion detection.
  - `.fantome` and `.zip` archives.
  - Raw `.modpkg` archives (auto-migrated and unpacked into Rose's modern layout).
* **Automatic Name Beautification**: Formats mod folders cleanly into `<Mod Skin Name> <Champion> v<Version>` (e.g. `Sonic Rammus v1.0`, `Chun Li Garen v2.0`). Disable anytime with `--no-beautify`.
* **Automatic Quarantine Isolation (`.broken/`)**: Moves unrepairable or broken mods into a hidden `.broken/` folder, removing them from Rose manifests and mappings so your active skin directories contain only healthy, working mods.
* **Multi-Mod Pasting & Reconciliation**: Seamlessly recalculates and synchronizes `rose_mod_targets.json` when multiple mods are copied or merged into existing skin target directories.
* **Deep System Cleanup**: Purges empty directories and orphan target manifests even when containing hidden, read-only Windows system files like `desktop.ini`.
* **Skin ID Mapping Generator**: Cross-references numeric skin folders (`106000`, `33000`) with champion names and installed mods into `skin_mappings.txt` and `skin_mappings.json`.
* **Zero-Delay Instant Startup**: Zero background process polling overhead; scans and reports immediately.
* **Double-Click Zero-Argument Mode**: Simply drop into any folder and double-click. Automatically pauses at the end so the console window remains open for review.
