# LoL Mod Fixer (`lol-mod-fixer`)

A portable, standalone native Rust CLI tool for League of Legends mod diagnosis and repair, built on the official [LeagueToolkit/ltk-manager](https://github.com/LeagueToolkit/ltk-manager) engine.

`lol-mod-fixer` inspects mod packages headlessly using LTK Manager's real health and problems engine, reports detected property mismatches and outdated types, and repairs them using LTK Manager's actual repair and export pipeline without needing the Tauri desktop GUI.

---

## Features

- **Direct Upstream Engine Reuse**: Integrates `ltk-manager-base`, `ltk-manager-assets`, `ltk-manager-library`, `ltk-manager-problems`, and `ltk-manager-workshop` directly as first-party crate dependencies.
- **Single Portable Executable**: Compiles into a completely standalone Windows binary (`lol-mod-fixer.exe`) with no extra runtime dependencies.
- **Double-Click / Zero-Argument Workflow**: Dropping `lol-mod-fixer.exe` into a folder with mods and double-clicking automatically scans and repairs the directory, pausing on completion so the console window remains visible.
- **Health Check Mode (`check`)**: Analyzes archives for broken BIN properties, outdated metadata, and invalid types without modifying any files.
- **Safe Repair Pipeline (`repair` / `auto`)**:
  - Operates in isolated temporary workspaces.
  - Applies LTK Manager's `repair_mod` rule passes.
  - Automatically verifies output archives before touching target files.
  - Replaces files atomically with rollback protection and optional `.bak` backups (`--backup`).
- **Machine-Readable JSON**: `--json` output writes pure JSON to `stdout` with progress and diagnostics directed to `stderr`.
- **Honest Format Detection**:
  - `.fantome` / Fantome zip: Fully supported for inspection and repair.
  - **Extracted Fantome Folder**: Fully supported! Folders structured as `<mod_dir>/META/info.json` and `<mod_dir>/WAD/*.wad.client` (standard for Rose mod manager and unpacked mods) are inspected and repaired directly in-place or to an output directory.
  - `.modpkg`: Detected and reported as unrepairable (LTK Manager reads `.modpkg` directly with no unpacked or editable form).
  - Standalone `.wad.client`: Detected and reported as unsupported (standalone packed WADs store only xxHash64 hashes without custom author filenames; custom paths cannot be recovered without mod metadata).
- **Auto-Discovery of League of Legends**: Automatically locates League installs via `RiotClientInstalls.json` or standard paths, with support for `--league` and persistent configuration.

---

## Installation & Build

### Prerequisites
- Rust 1.85+ (Edition 2024 support)
- Git with submodule support

### Build from Source
```bash
# Clone the repository with submodules
git clone --recurse-submodules https://github.com/your-username/lol-mod-fixer.git
cd lol-mod-fixer

# Build optimized release binary
cargo build --release
```

The compiled standalone executable is located at:
```text
target/release/lol-mod-fixer.exe
```

---

## Usage

### 1. Default Mode (Zero Arguments / Double-Click)
Run the executable directly or double-click `lol-mod-fixer.exe` in Windows Explorer:
```bash
lol-mod-fixer.exe
```
- Scans all mod archives in the folder where `lol-mod-fixer.exe` is located.
- Diagnoses and repairs repairable archives safely in-place.
- Automatically pauses at the end (`Press Enter to exit...`) when run interactively.

### 2. Health Check (`check`)
Inspect a single archive or directory without modifying any files:
```bash
# Check a single mod archive
lol-mod-fixer check "C:\Mods\Ahri.fantome"

# Check an extracted Fantome mod folder
lol-mod-fixer check "C:\Users\Admin\AppData\Local\Rose\mods\skins\34000\Emilia_Anivia-1.0.0"

# Check all mods in a folder (including extracted mod folders)
lol-mod-fixer check "C:\Mods"

# Recursively scan a folder containing subfolders of mods (e.g. Rose skin directories)
lol-mod-fixer check "C:\Users\Admin\AppData\Local\Rose\mods\skins" -r

# Verbose check with individual problem descriptions
lol-mod-fixer check "C:\Mods\Ahri.fantome" -v

# Machine-readable JSON output
lol-mod-fixer check "C:\Mods\Ahri.fantome" --json
```

Example output:
```text
League Mod Fixer (LTK Repair Engine)
=====================================

[1/1] Ahri.fantome
      Format: Fantome
      Status: REPAIRABLE
      Problems found: 14 (14 repairable)
        [REPAIRABLE] A meta property at a type the game no longer reads (bin/property-type)

--------------------------------
Check complete

Mods scanned:       1
Healthy:            0
Repairable:         1
Unrepairable:       0
Broken:             0
Unsupported:        0
```

### 3. Repair Mode (`repair`)
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

### 4. Automatic Mode (`auto`)
Checks health, prints diagnostics, repairs when possible, and verifies the repaired output:
```bash
lol-mod-fixer auto "C:\Mods\Ahri.fantome"
```

### 5. Configuration (`config`)
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

## Format Capabilities & Limitations

| Format | Extension / Structure | Health Check | Repair Support | Technical Rationale |
|---|---|---|---|---|
| **Fantome Archive** | `.fantome`, `.zip` | ✅ Yes | ✅ Yes | Full support via `ltk-manager-library` and `ltk_fantome`. Outdated BIN property types and references are detected and repaired. |
| **Fantome Folder** | `<dir>/META/info.json`<br>`<dir>/WAD/*.wad.client` | ✅ Yes | ✅ Yes | Direct support for extracted mods and Rose directories. Analyzed and repaired in-place or to target folder with atomic rollback safety. |
| **ModPkg** | `.modpkg` | ✅ Detected | ❌ Unrepairable | By design in LTK Manager, `.modpkg` is read straight out of its archive without an unpacked representation. Converting/repairing `.modpkg` is not supported upstream. |
| **Client WAD** | `.wad.client`, `.wad` | ✅ Detected | ❌ Unsupported | Standalone WAD archives contain only 64-bit xxHash hashes in their table of contents. Custom author filenames cannot be recovered without mod project metadata. |

---

## Machine-Readable JSON Schema

When passing `--json`, `lol-mod-fixer` emits valid JSON to `stdout`:

### Check Mode (`check --json`)
```json
{
  "mode": "check",
  "directory": "C:\\Mods",
  "summary": {
    "scanned": 1,
    "healthy": 0,
    "repairable": 1,
    "unrepairable": 0,
    "broken": 0,
    "unsupported": 0
  },
  "mods": [
    {
      "path": "C:\\Mods\\Ahri.fantome",
      "file_name": "Ahri.fantome",
      "format": "fantome",
      "status": "repairable",
      "problem_count": 14,
      "repairable_count": 14,
      "unrepairable_count": 0,
      "problems": [
        {
          "rule_id": "bin/property-type",
          "severity": "Fatal",
          "description": "A meta property at a type the game no longer reads",
          "file": "base:Ahri.wad.client/1234567890abcdef",
          "repairable": true
        }
      ],
      "reason": null
    }
  ]
}
```

### Repair Mode (`repair --json`)
```json
{
  "mode": "repair",
  "directory": null,
  "summary": {
    "scanned": 1,
    "healthy": 0,
    "repaired": 1,
    "unrepairable": 0,
    "failed": 0,
    "unsupported": 0
  },
  "mods": [
    {
      "path": "C:\\Mods\\Ahri.fantome",
      "format": "fantome",
      "status": "repaired",
      "modified": true,
      "output_path": "C:\\Mods\\Ahri.fantome",
      "fixes_applied": 14
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
