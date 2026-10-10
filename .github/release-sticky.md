### Quick Start (How to Use with Rose)

1. Press `Win + R`, paste `%LOCALAPPDATA%\Rose\mods\skins`, and press **Enter**.
2. Copy **`lol-mod-fixer.exe`** into that `skins` folder.
3. Right-click **`lol-mod-fixer.exe`** -> **Run as administrator** (or double-click directly).
4. The tool will automatically:
   - Recursively scan and repair all installed skins in-place.
   - Automatically unlock any restricted Rose folder permissions.
   - Auto-beautify folder names into `<Mod Skin Name> <Champion> v<Version>`.
   - Safely quarantine unrepairable or corrupted mods into `.broken/` and update `rose_mod_targets.json`.
   - Update Rose's `rose_mod_targets.json` manifests so Rose detects the updated names immediately.
   - Generate `skin_mappings.txt` and `skin_mappings.json` for easy ID lookup.
   - Clean up empty directories and orphan manifests.
   - Pause with a clean summary so you can review the results before exiting.

---

### Release Asset (Zero Dependencies)

| Asset | Description |
| :--- | :--- |
| **`lol-mod-fixer.exe`** | **Statically linked C runtime (`/MT`)**.<br>Completely standalone with **zero external dependencies**. Runs out-of-the-box on any clean Windows 10/11 machine without requiring Microsoft Visual C++ Redistributable (`vcruntime140.dll`), .NET, Python, Node.js, or batch wrappers. |
