# LoL Mod Fixer (`lol-mod-fixer`)

[![CI](https://github.com/jsoul-dev/lol-mod-fixer/actions/workflows/ci.yml/badge.svg)](https://github.com/jsoul-dev/lol-mod-fixer/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/jsoul-dev/lol-mod-fixer?style=flat-square&color=blue)](https://github.com/jsoul-dev/lol-mod-fixer/releases/latest)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078D4?style=flat-square&logo=windows)](https://github.com/jsoul-dev/lol-mod-fixer/releases/latest)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue?style=flat-square)](LICENSE)

A portable, standalone native Rust CLI tool for League of Legends mod diagnosis, repair, automatic name beautification, and legacy structure migration, built on the official [LeagueToolkit/ltk-manager](https://github.com/LeagueToolkit/ltk-manager) engine with full [Rose](https://github.com/Alban1911/Rose) integration.

`lol-mod-fixer` inspects mod packages headlessly using LTK Manager's real health and problems engine, reports detected property mismatches and outdated types, and repairs them using LTK Manager's actual repair and export pipeline without needing the Tauri desktop GUI.

---

## Key Features

- **Direct Upstream Engine Reuse**: Integrates `ltk-manager-base`, `ltk-manager-assets`, `ltk-manager-library`, `ltk-manager-problems`, and `ltk-manager-workshop` directly as first-party crate dependencies.
- **Single Portable Executable (Zero Dependencies)**: Compiles with statically linked C runtime (`+crt-static`), meaning it runs out-of-the-box on any Windows 10/11 computer without requiring Visual C++ Redistributable, .NET, Python, or Node.js.
- **Automatic Mod & Folder Beautification (ON by Default)**:
  - Formats all mod and folder names into clean, client-friendly titles:
    `<Mod Skin Name> <Champion Name> v<Version>`
  - Examples:
    - `Sonic_Rammus-1.0.0` → `Sonic Rammus v1.0`
    - `Rammus_Sonic-1.0.0` → `Sonic Rammus v1.0` *(always skin name first, champion second)*
    - `tank-volibear` → `Tank Volibear v1.0` *(automatic title casing)*
    - `Angel-v1.0.0` → `Angel Volibear v1.0` *(auto-detects champion from skin ID `106000`)*
    - `Chun_Li_Garen-2.0` → `Chun Li Garen v2.0`
    - `Ansem_Malzahar-Main` → `Ansem Malzahar v1.1.2` *(resolves true version from `info.json`)*
    - `Zacian_Hecarim-1.1.0` → `Zacian Hecarim v1.1`
    - `Shadow_The_Hedgehog__Ekko_-1.1.3` → `Shadow The Hedgehog Ekko v1.1.3`
  - Fully idempotent: already-beautified mods remain untouched.
  - Can be disabled at any time with `--no-beautify`.
- **Automatic Empty Folder & Orphan Manifest Cleanup (ON by Default)**:
  - Automatically cleans empty mod subdirectories and unused target folders.
  - Automatically deletes orphan target directories containing only `rose_mod_targets.json` / `rose_wad_targets.json` when all mods have been uninstalled.
  - Recursively searches for and removes empty `Hematite-Fixed` folders.
  - Can be disabled at any time with `--no-cleanup`.
- **Automatic Skin ID Mapping Generator (`skin_mappings.txt` & `skin_mappings.json`) (ON by Default)**:
  - Generates human-readable (`skin_mappings.txt`) and machine-readable (`skin_mappings.json`) mapping files directly beside the skin folders in Rose's skins directory.
  - Cross-references numeric skin folder IDs (e.g. `106000`, `33000`) with champion names (`Volibear`, `Rammus`), skin names, and lists all installed mods.
  - Automatically synchronizes when folders are deleted or newly imported.
  - Can be disabled at any time with `--no-mapping`.
- **Automatic Quarantine of Corrupted & Unrepairable Mods (ON by Default)**:
  - When a mod archive or folder cannot be repaired (e.g. CRC checksum corruption, missing files, or unresolvable rule errors), it is safely moved into a hidden `.broken/` quarantine folder.
  - Automatically updates `rose_mod_targets.json` and skin mappings (`skin_mappings.txt` and `.json`) to remove all references to broken mods, ensuring your skin directories contain **only healthy, working mods**.
  - Prevents game crashes and accidental loading in Rose without permanently destroying your files.
  - Can be left in-place with `--no-quarantine` or permanently deleted with `--delete-unrepairable`.
- **Embedded Offline League Champion & Skin Database**:
  - Bundles 9,303 official skin IDs from `Alban1911/LeagueSkins`.
  - Accurately identifies champions using parent folder Skin IDs (`106000` → Volibear, `33000` → Rammus), internal WAD client archives (`WAD/Ekko.wad.client`), or folder name tokens completely offline.
- **Rose Mod Manager Integration**:
  - **Legacy & Raw Archive Auto-Migration (ON by Default)**: Automatically detects outdated or raw archives (`.fantome`, `.zip`, and `.modpkg`) placed directly inside numeric skin target folders without being extracted. Safely extracts archives (converting `.modpkg` binary packages via LTK project unpacking into clean Rose layout), flattens redundant wrapper folders, validates files, unlinks original archives, and builds `rose_mod_targets.json` with exact `folderHash` and `wadHashes` so mods inject properly in Rose client and Party Mode. Already updated structures are left completely untouched. (Disable with `--no-migrate`).
  - **Extracted Mod Folder Support**: Directly supports `<mod_dir>/META/info.json` and `<mod_dir>/WAD/*.wad.client` structures.
  - **Manifest Synchronization**: Automatically updates Rose's `rose_mod_targets.json` with the newly repaired `folderHash` and `wadHashes` using Rose's exact hashing algorithm, preserving target skin IDs and display names.
  - **Automatic NTFS Permissions Unlocking**: Detects restricted/hooked Rose folders (`os error 5: Access is denied`) and automatically resets NTFS access control lists (ACLs) when elevated.
- **Double-Click / Zero-Argument Workflow**: Dropping `lol-mod-fixer.exe` into a folder with mods and double-clicking automatically migrates legacy archives, cleans empty folders, scans and repairs the directory, synchronizes skin ID mappings, and pauses on completion so the console window remains visible.

- **Vibrant ANSI Console Output**: Color-coded status badges (`HEALTHY`, `REPAIRABLE`, `UNREPAIRABLE`, `BROKEN`) with native Windows VT100 console support.
- **Machine-Readable JSON Mode**: `--json` output writes pure, uncolored JSON to `stdout` with progress and diagnostics directed to `stderr`.


---

## Installation & Builds

### Pre-Built Binaries: Which One Should You Download?

The [latest release](https://github.com/jsoul-dev/lol-mod-fixer/releases/latest) provides two executable variants:

1. **`lol-mod-fixer-static.exe`** *(Recommended for 99% of Users)*:
   - Compiles with statically linked C runtime (`/MT` via `+crt-static`).
   - **Completely standalone**: runs out of the box on any clean Windows 10/11 system with **zero dependencies**—no Microsoft Visual C++ Redistributable (`vcruntime140.dll`), .NET, Python, or external runtimes required.
   - You will never encounter *"The code execution cannot proceed because VCRUNTIME140.dll was not found"*.
2. **`lol-mod-fixer-dynamic.exe`**:
   - Standard MSVC dynamic runtime build (`/MD`), which links against the system's `vcruntime140.dll`.
   - Requires Microsoft Visual C++ 2015–2022 Redistributable installed on Windows.
   - Only choose this if you specifically manage shared Visual C++ runtimes system-wide.
   - *Note: Both variants contain identical features, performance, and repair logic (the ~5 KB difference between them is solely the embedded CRT glue code).*

### Building from Source

```bash
# Clone the repository with submodules
git clone --recurse-submodules https://github.com/jsoul-dev/lol-mod-fixer.git
cd lol-mod-fixer

# Build static release binary (configured in .cargo/config.toml)
cargo build --release
```

The output executable is created at:
```text
target/release/lol-mod-fixer.exe
```

### Updating with Upstream LeagueToolkit Releases
When League of Legends patches and `LeagueToolkit/ltk-manager` ships updated repair rules:
```bash
# Pull the latest upstream LTK engine into the submodule
git submodule update --remote vendor/ltk-manager

# Rebuild with new repair rules
cargo build --release
```
*(An automated GitHub Actions workflow [`.github/workflows/update-upstream.yml`](.github/workflows/update-upstream.yml) also checks for upstream updates every Wednesday after League patches and automatically tests and opens a PR).*

---

## Usage

### 1. Default Mode (Zero Arguments / Double-Click)
Run the executable directly or double-click `lol-mod-fixer.exe` in Windows Explorer:
```bash
lol-mod-fixer.exe
```
- Scans all mod archives in the current folder (recursively by default).
- Beautifies folder names and syncs `rose_mod_targets.json`.
- Diagnoses and repairs repairable archives safely in-place.
- Automatically pauses at the end (`Press Enter to exit...`) when run interactively.

### 2. Administrator Launcher Script (`scripts/run-fixer-as-admin.bat`)
When running inside recent Rose installations that apply restrictive folder permissions:
- Right-click `scripts/run-fixer-as-admin.bat` (or the one included in release packages) → **Run as administrator**.
- Automatically prompts for UAC elevation, unlocks restricted NTFS permissions, repairs all mods, beautifies folder names, and synchronizes `rose_mod_targets.json`.
- Forwards any CLI arguments passed to it (e.g. `run-fixer-as-admin.bat --no-beautify`).

### 3. How to Use with Rose (Fix Already-Imported Skins)

Rose extracts and organizes all installed custom skins inside the user's Local AppData directory:
```text
%LOCALAPPDATA%\Rose\mods\skins
(e.g., C:\Users\<YourUsername>\AppData\Local\Rose\mods\skins)
```

To repair, clean, and organize all skins already imported into Rose:

#### Method A: Drop & Double-Click (Recommended)
1. Press `Win + R`, paste `%LOCALAPPDATA%\Rose\mods\skins`, and press **Enter**.
2. Copy `lol-mod-fixer-static.exe` and `run-fixer-as-admin.bat` into that `skins` folder.
3. Right-click `run-fixer-as-admin.bat` → **Run as administrator** (or double-click `lol-mod-fixer-static.exe` directly).
4. `lol-mod-fixer` will automatically:
   - Recursively scan all champion skin folders (e.g., `106000`, `33000`, `34000`).
   - Repair broken BIN properties, outdated sound bank IDs, and format errors in-place.
   - Auto-beautify mod folder names into `<Mod Skin Name> <Champion> v<Version>`.
   - Update Rose's `rose_mod_targets.json` manifests so Rose detects the updated mod names instantly.
   - Generate `skin_mappings.txt` and `skin_mappings.json` for easy reference.
   - Clean up any empty folders and orphan manifests.
   - Pause with a clean summary so you can review the results.

#### Method B: Run via Command Line / PowerShell (From Anywhere)
You can also run the fixer against Rose's skins directory directly from any terminal using the `%LOCALAPPDATA%` environment variable:

```powershell
# Repair and beautify all imported skins in Rose:
lol-mod-fixer.exe repair "$env:LOCALAPPDATA\Rose\mods\skins" -r

# Or repair without renaming folders (keep original folder names):
lol-mod-fixer.exe repair "$env:LOCALAPPDATA\Rose\mods\skins" -r --no-beautify

# Perform a safe diagnostic check without modifying any files:
lol-mod-fixer.exe check "$env:LOCALAPPDATA\Rose\mods\skins" -r
```

### 4. Health Check Mode (`check`)
Inspect a single archive or directory without modifying any files:
```bash
# Check a single mod archive
lol-mod-fixer check "C:\Mods\Ahri.fantome"

# Check an extracted Fantome mod folder
lol-mod-fixer check "C:\Users\Admin\AppData\Local\Rose\mods\skins\34000\Emilia_Anivia-1.0.0"

# Recursively scan all Rose skins folders
lol-mod-fixer check "C:\Users\Admin\AppData\Local\Rose\mods\skins" -r

# Verbose check with individual problem rule IDs
lol-mod-fixer check "C:\Mods\Ahri.fantome" -v

# Machine-readable JSON output
lol-mod-fixer check "C:\Mods\Ahri.fantome" --json
```

### 5. Repair Mode (`repair`)
Repair a mod archive, extracted mod folder, or all mods in a directory:
```bash
# Repair an archive in-place
lol-mod-fixer repair "C:\Mods\Ahri.fantome"

# Repair an extracted mod folder in-place
lol-mod-fixer repair "C:\Users\Admin\AppData\Local\Rose\mods\skins\34000\Emilia_Anivia-1.0.0"

# Repair and output to a separate destination (preserves original)
lol-mod-fixer repair "C:\Mods\Ahri.fantome" "C:\Mods\fixed\Ahri.fantome"

# Repair with backup (.bak) of original file/folder
lol-mod-fixer repair "C:\Mods\Ahri.fantome" --backup

# Dry-run: report what would be repaired without writing files
lol-mod-fixer repair "C:\Mods" --dry-run
```

### 6. Automatic Mode (`auto`)
Checks health, prints diagnostics, repairs when possible, and verifies the repaired output:
```bash
lol-mod-fixer auto "C:\Mods\Ahri.fantome"
```

### 7. Disabling Beautification (`--no-beautify`)
If you or your users prefer to keep the original, raw mod folder names untouched:
```bash
# Run with beautification disabled
lol-mod-fixer.exe --no-beautify

# Or via explicit repair
lol-mod-fixer.exe repair "C:\Users\Admin\AppData\Local\Rose\mods\skins" --no-beautify
```
*(You can also create a Windows shortcut to `lol-mod-fixer.exe` and add ` --no-beautify` to the Target field).*

### 8. Configuration (`config`)
Manage persistent tool settings stored in `%APPDATA%\LeagueToolkit\LoLModFixer\config\config.json`:
```bash
# View current configuration and detected League paths
lol-mod-fixer config show

# Set persistent League of Legends path
lol-mod-fixer config set-league "C:\Riot Games\League of Legends"

# Unset League path (fallback to auto-detection)
lol-mod-fixer config unset-league

# Configure exit pause behavior
lol-mod-fixer config set-pause true
lol-mod-fixer config set-pause false
```

---

## CLI Options Reference

| Flag / Option | Description |
|---|---|
| `[TARGET]` | Target path (file or folder). Defaults to the executable's directory if omitted. |
| `-d, --dir <DIR>` | Explicit directory to scan. |
| `-l, --league <DIR>` | Path to League of Legends installation root or Game directory. |
| `-r, --recursive` | Recursively scan subdirectories for mod archives. |
| `-v, --verbose` | Verbose output showing individual rule IDs and descriptions. |
| `--dry-run` | Scan and diagnose problems without writing or modifying files. |
| `--backup` | Create a `.bak` backup before replacing any repaired archive. |
| `--no-beautify` | Disable automatic beautification of mod and folder names. |
| `--no-cleanup` | Disable automatic cleanup of empty folders and orphan Rose manifests. |
| `--no-mapping` | Disable automatic generation of skin folder ID mappings (`skin_mappings.txt` and `.json`). |
| `--no-migrate` | Disable automatic migration of outdated Rose mod structures (.fantome/.zip archives). |
| `--no-quarantine` | Disable automatic quarantine of unrepairable and corrupted mods to `.broken/` folder. |
| `--delete-unrepairable` | Permanently delete unrepairable and corrupted mods instead of quarantining them to `.broken/`. |
| `--elevate` | Request Administrator elevation via UAC to unlock restricted mod folders. |
| `--pause` | Force interactive pause prompt (`Press Enter to exit...`) at the end. |
| `--no-pause` | Disable interactive pause prompt at the end (useful for scripts/CI). |
| `--sync-hashtables` | Synchronize or update local hashtables from mimir before starting. |
| `--json` | Output results in JSON format on stdout (diagnostics sent to stderr). |

---

## Format Capabilities & Limitations

| Format | Extension / Structure | Health Check | Repair Support | Technical Rationale |
|---|---|---|---|---|
| **Fantome Archive** | `.fantome`, `.zip` | ✅ Yes | ✅ Yes | Full support via `ltk-manager-library` and `ltk_fantome`. Outdated BIN property types, outdated references, and audio bank IDs are detected and repaired. |
| **Fantome Folder** | `<dir>/META/info.json`<br>`<dir>/WAD/*.wad.client` | ✅ Yes | ✅ Yes | Direct support for extracted mods and Rose directories. Inspected, repaired, and beautified in-place with atomic rollback safety and `rose_mod_targets.json` synchronization. |
| **ModPkg** | `.modpkg` | ✅ Detected / Auto-Migrated | ✅ Repaired upon extraction | Standalone `.modpkg` is an immutable package in LTK Manager. However, `lol-mod-fixer`'s migration engine automatically converts raw `.modpkg` archives into clean Rose extracted layouts (`META/` + `WAD/`), repairing and hashing them in the process. |
| **Client WAD** | `.wad.client`, `.wad` | ✅ Detected | ❌ Unsupported | Standalone WAD archives contain only 64-bit xxHash hashes in their table of contents. Custom author filenames cannot be recovered without mod project metadata. |

---

## Machine-Readable JSON Schema

When passing `--json`, `lol-mod-fixer` emits valid JSON to `stdout`:

```json
{
  "mode": "check",
  "directory": "C:\\Users\\Admin\\AppData\\Local\\Rose\\mods\\skins",
  "summary": {
    "scanned": 9,
    "healthy": 9,
    "repairable": 0,
    "unrepairable": 0,
    "broken": 0,
    "unsupported": 0
  },
  "mods": [
    {
      "path": "C:\\Users\\Admin\\AppData\\Local\\Rose\\mods\\skins\\33000\\Sonic Rammus v1.0",
      "file_name": "Sonic Rammus v1.0",
      "format": "fantome_folder",
      "status": "healthy",
      "problem_count": 0,
      "repairable_count": 0,
      "unrepairable_count": 0,
      "problems": [],
      "reason": null
    }
  ]
}
```

---

## Exit Codes

- `0`: Success (all mods scanned, healthy mods left untouched, repairs successfully applied and verified).
- `1`: One or more mods had unrepairable issues, repair failures, or check found repairable issues in `check` mode.
- `2`: Initialization error (e.g. invalid arguments or unreadable config).

---

## License

GPL-3.0-or-later (aligned with `LeagueToolkit/ltk-manager`).
