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
