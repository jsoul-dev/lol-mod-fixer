# LoL Mod Fixer (`lol-mod-fixer`)

A portable, standalone native Rust CLI tool for League of Legends mod diagnosis, repair, and automatic name beautification, built on the official [LeagueToolkit/ltk-manager](https://github.com/LeagueToolkit/ltk-manager) engine with full [Rose Mod Manager](https://github.com/the-rose-project/rose) integration.

`lol-mod-fixer` inspects mod packages headlessly using LTK Manager's real health and problems engine, reports detected property mismatches and outdated types, and repairs them using LTK Manager's actual repair and export pipeline without needing the Tauri desktop GUI.

---

## Key Features

- **Direct Upstream Engine Reuse**: Integrates `ltk-manager-base`, `ltk-manager-assets`, `ltk-manager-library`, `ltk-manager-problems`, and `ltk-manager-workshop` directly as first-party crate dependencies.
- **Single Portable Executable (Zero Dependencies)**: Compiles with statically linked C runtime (`+crt-static`), meaning it runs out-of-the-box on any Windows 10/11 computer without requiring Visual C++ Redistributable, .NET, Python, or Node.js.
- **Automatic Mod & Folder Beautification (ON by Default)**:
  - Formats all mod and folder names into clean, client-friendly titles:
    $$\text{<Mod Skin Name>} \quad \text{<Champion Name>} \quad \text{v<Version>}$$
  - Examples:
    - `Sonic_Rammus-1.0.0` $\rightarrow$ `Sonic Rammus v1.0`
    - `Rammus_Sonic-1.0.0` $\rightarrow$ `Sonic Rammus v1.0` *(always skin name first, champion second)*
    - `tank-volibear` $\rightarrow$ `Tank Volibear v1.0` *(automatic title casing)*
    - `Angel-v1.0.0` $\rightarrow$ `Angel Volibear v1.0` *(auto-detects champion from skin ID `106000`)*
    - `Chun_Li_Garen-2.0` $\rightarrow$ `Chun Li Garen v2.0`
    - `Ansem_Malzahar-Main` $\rightarrow$ `Ansem Malzahar v1.1.2` *(resolves true version from `info.json`)*
    - `Zacian_Hecarim-1.1.0` $\rightarrow$ `Zacian Hecarim v1.1`
    - `Shadow_The_Hedgehog__Ekko_-1.1.3` $\rightarrow$ `Shadow The Hedgehog Ekko v1.1.3`
  - Fully idempotent: already-beautified mods remain untouched.
  - Can be disabled at any time with `--no-beautify`.
- **Embedded Offline League Champion & Skin Database**:
  - Bundles 9,303 official skin IDs from `Alban1911/LeagueSkins`.
  - Accurately identifies champions using parent folder Skin IDs (`106000` $\rightarrow$ Volibear, `33000` $\rightarrow$ Rammus), internal WAD client archives (`WAD/Ekko.wad.client`), or folder name tokens completely offline.
- **Rose Mod Manager Integration**:
  - **Extracted Mod Folder Support**: Directly supports `<mod_dir>/META/info.json` and `<mod_dir>/WAD/*.wad.client` structures.
  - **Manifest Synchronization**: Automatically updates Rose's `rose_mod_targets.json` with the newly repaired `folderHash` and `wadHashes` using Rose's exact hashing algorithm, preserving target skin IDs and display names.
  - **Automatic NTFS Permissions Unlocking**: Detects restricted/hooked Rose folders (`os error 5: Access is denied`) and automatically resets NTFS access control lists (ACLs) when elevated.
- **Double-Click / Zero-Argument Workflow**: Dropping `lol-mod-fixer.exe` into a folder with mods and double-clicking automatically scans and repairs the directory, pausing on completion so the console window remains visible.
- **Vibrant ANSI Console Output**: Color-coded status badges (`HEALTHY`, `REPAIRABLE`, `UNREPAIRABLE`, `BROKEN`) with native Windows VT100 console support.
- **Machine-Readable JSON Mode**: `--json` output writes pure, uncolored JSON to `stdout` with progress and diagnostics directed to `stderr`.

---

## Installation & Builds

### Pre-Built Binaries
The `dist/` directory provides two release variants:

1. **`dist/lol-mod-fixer-static.exe`** *(Recommended)*:
   - Statically links the C runtime (`+crt-static`).
   - Completely standalone: runs on any Windows 10/11 PC with **zero dependencies**.
2. **`dist/lol-mod-fixer-dynamic.exe`**:
   - Standard dynamic MSVC build linking against `vcruntime140.dll`.

### Building from Source

```bash
# Clone the repository with submodules
git clone --recurse-submodules https://github.com/your-username/lol-mod-fixer.git
cd lol-mod-fixer

# Build static release binary (configured in .cargo/config.toml)
cargo build --release
```

The output executable is created at:
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
- Scans all mod archives in the current folder (recursively by default).
- Beautifies folder names and syncs `rose_mod_targets.json`.
- Diagnoses and repairs repairable archives safely in-place.
- Automatically pauses at the end (`Press Enter to exit...`) when run interactively.

### 2. Administrator Launcher Script (`run-fixer-as-admin.bat`)
When running inside recent Rose installations that apply restrictive folder permissions:
- Right-click `run-fixer-as-admin.bat` $\rightarrow$ **Run as administrator**.
- Automatically prompts for UAC elevation, unlocks restricted NTFS permissions, repairs all mods, beautifies folder names, and synchronizes `rose_mod_targets.json`.
- Forwards any CLI arguments passed to it (e.g. `run-fixer-as-admin.bat --no-beautify`).

### 3. Health Check Mode (`check`)
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

### 4. Repair Mode (`repair`)
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

### 5. Automatic Mode (`auto`)
Checks health, prints diagnostics, repairs when possible, and verifies the repaired output:
```bash
lol-mod-fixer auto "C:\Mods\Ahri.fantome"
```

### 6. Disabling Beautification (`--no-beautify`)
If you or your users prefer to keep the original, raw mod folder names untouched:
```bash
# Run with beautification disabled
lol-mod-fixer.exe --no-beautify

# Or via explicit repair
lol-mod-fixer.exe repair "C:\Users\Admin\AppData\Local\Rose\mods\skins" --no-beautify
```
*(You can also create a Windows shortcut to `lol-mod-fixer.exe` and add ` --no-beautify` to the Target field).*

### 7. Configuration (`config`)
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
| **ModPkg** | `.modpkg` | ✅ Detected | ❌ Unrepairable | By design in LTK Manager, `.modpkg` is read straight out of its archive without an unpacked representation. Converting/repairing `.modpkg` is not supported upstream. |
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
