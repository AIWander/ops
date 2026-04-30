# Changelog

## v0.3.1 - 2026-04-29

### Changed

- **Breadcrumb shape aligned with manager-universal v1.0+ dual-source reader.** Path resolution and JSONL field set updated so that ops breadcrumbs are consumed by manager-universal's dashboard alongside any other source.
  - Path resolution: 4-tier priority - `OPS_BREADCRUMBS_DIR` (matches manager-universal) -> `OPS_BREADCRUMB_PATH` (legacy, kept) -> `%LOCALAPPDATA%\CPC\ops-data\logs` (default matches manager-universal) -> exe-relative fallback.
  - Each breadcrumb gets a stable `id` field generated at `start()` time as `bc_{unix_ts}_{slug}`. Required by manager-universal's dedup key.
  - Each JSONL entry now carries a `status` field: `active` / `complete` / `aborted`. Required by manager-universal's active-filter.
  - Existing breadcrumb consumers continue to work; new fields are additive.

## v0.3.0 - 2026-04-29

### Added
- New `bash` tool — execute commands via Git Bash, mirrors `powershell` semantics with `allow_destructive` and `confirm` flags. Falls back to PATH if Git Bash not at standard location; honors `OPS_BASH_PATH` env override.
- Extended command blocklist with Unix-shaped patterns: dd-to-device, fork bombs, `rm -rf` of root or system dirs, `curl | sh`, `chmod 777` on system paths, mkfs/shred on raw devices, systemctl/service/iptables/ufw/crontab/visudo/package-manager guards.

### Changed
- Tool count: 68 → 69.

## v0.2.1 - 2026-04-29

### Changed
- Upgraded `rusqlite` to 0.39 (bundled SQLite update + API adaptations).
- Bumped GitHub Actions versions to v5 (checkout, upload-artifact, download-artifact) — addresses upcoming Node.js 20 deprecation.

### Fixed
- v0.2.0 tag pointed at the initial commit (pre-clippy fixes); v0.2.1 tag points at HEAD with all fixes applied.

## v0.2.0 - 2026-04-28 - First public release

Initial public release of `ops`, the Windows operations MCP server for Claude Desktop.
Originated as an internal consolidation of the `research-mcp` crate from the CPC stack.

### Highlights

- **68 tools across 14 categories:** file I/O, transforms, persistent sessions,
  utilities, archives, build/deploy, health, breadcrumbs, reminders, dead-drops,
  recovery, install identity, shell, and misc.
- **4-tier command blocklist** on `powershell` and `session_run`: catastrophic
  patterns blocked unconditionally; destructive commands require explicit
  `allow_destructive: true`; service/firewall/registry changes require `confirm: true`;
  everything else runs freely.
- **Portable path resolution** - default state at `%LOCALAPPDATA%\Ops\`,
  overridable via env vars. No Google Drive or hardcoded paths.
- **Companion installer** (`install-ops-x64.exe`) - backs up Claude Desktop config,
  adds the ops entry only, surfaces backup path for easy revert.

### Known gaps for v0.2.x

- ARM64 installer not yet built (manual config edit required for ARM64 users)
- Some helper modules retained as compile dependencies still emit dead-code warnings;
  will be pruned in v0.2.1
