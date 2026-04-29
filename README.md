# ops MCP Server

[![CI](https://github.com/AIWander/ops/actions/workflows/ci.yml/badge.svg)](https://github.com/AIWander/ops/actions/workflows/ci.yml)

**Windows operations MCP server: file I/O, persistent sessions, build/deploy, breadcrumb tracking, reminders, and dead-drop coordination. The `powershell` and `session_run` tools enforce a 4-tier safety blocklist for destructive commands.**

Version 0.3.0 Â· Apache 2.0 Â· [GitHub](https://github.com/AIWander/ops)

**Part of [CPC](https://github.com/AIWander) (Copy Paste Compute)** - a multi-agent AI orchestration platform. Related repos: [local](https://github.com/AIWander/local) Â· [manager](https://github.com/AIWander/manager) Â· [hands](https://github.com/AIWander/hands) Â· [workflow](https://github.com/AIWander/workflow) Â· [cpc-paths](https://github.com/AIWander/cpc-paths) Â· [cpc-breadcrumbs](https://github.com/AIWander/cpc-breadcrumbs)

---

## What's New in v0.3.0

### v0.3.0 highlights

| Feature | Detail |
|---------|--------|
| `bash` tool | Execute commands via Git Bash â€” mirrors `powershell` with `allow_destructive` and `confirm` flags |
| Extended blocklist | Unix-shaped T4/T3/T2 patterns: dd-to-device, fork bombs, rm -rf root, curl\|sh, chmod 777, mkfs/shred, systemctl, iptables, apt |
| Path resolution | `OPS_BASH_PATH` env override â†’ standard Git Bash locations â†’ PATH fallback |
| 69 tools total | +1 from v0.2.1 |

---

## Installation

Each release ships an Inno Setup wizard. The recommended path is download → double-click → click Next.

| File | Platform | Use |
|------|----------|-----|
| `install-ops-x64.exe` | Windows x64 | **Recommended** - GUI installer |
| `install-ops-arm64.exe` | Windows ARM64 | **Recommended** - GUI installer |
| `ops-x64.exe` | Windows x64 | Standalone server binary, for manual install |
| `ops-arm64.exe` | Windows ARM64 | Standalone server binary, for manual install |

### Recommended: download then double-click

1. Download `install-ops-x64.exe` (or `install-ops-arm64.exe`) from the [latest release](https://github.com/AIWander/ops/releases/latest).
2. Double-click it.
3. Click through the wizard - the default install location (`%LOCALAPPDATA%\Ops`) does not require admin.

The wizard always copies the path to `ops.exe` to your clipboard. Paste it into any MCP client config:

- **Claude Desktop** - if installed, the wizard auto-registers `ops` in `claude_desktop_config.json` for you. Restart Claude Desktop to pick it up.
- **Claude Code** - paste into `~/.claude/mcp.json` under `mcpServers.ops.command`.
- **Codex CLI / Gemini CLI / any other stdio-MCP client** - paste into the equivalent field in that client's config.

If Claude Desktop is **not** installed, the wizard intentionally skips the config edit (no phantom config directories are created) and just leaves the path on the clipboard for you.

The wizard also:
- Backs up `claude_desktop_config.json` with a timestamp before touching it (only when Claude Desktop is installed)
- Adds (or updates) only the `ops` entry in `mcpServers` - touches nothing else
- Registers itself in Add/Remove Programs, so uninstall is one click. The uninstaller backs up the config again and removes the `ops` entry cleanly.

The installer does not need to know where Claude Desktop itself is installed. It only writes to `%APPDATA%\Claude\claude_desktop_config.json`, which is the per-user config path Claude Desktop hardcodes regardless of where the .exe lives.

### Manual install (no installer)

1. Download `ops-x64.exe` (or `ops-arm64.exe`) and rename it to `ops.exe`.
2. Place it somewhere permanent, e.g. `%LOCALAPPDATA%\Ops\ops.exe`.
3. Edit `%APPDATA%\Claude\claude_desktop_config.json` and add (replace `YourName` with your Windows username):

   ``json
   {
     "mcpServers": {
       "ops": {
         "command": "C:\\Users\\YourName\\AppData\\Local\\Ops\\ops.exe"
       }
     }
   }
   ``

4. Restart Claude Desktop.

### Prerequisites

- Windows 10/11 (x64 or ARM64)
- Claude Desktop, Claude Code, or any other MCP-compatible client

### Build from Source

Building the wizard requires [Inno Setup 6.x](https://jrsoftware.org/isdl.php) on top of the Rust toolchain. The same `ops.exe` is the MCP server *and* the post-install helper that the wizard calls (via the `register` / `unregister` subcommands), so there is only one binary to build.

``powershell
git clone https://github.com/AIWander/ops.git
cd ops
cargo build --release

# Then compile the GUI installer:
ISCC.exe /DOpsBinary="`C:\Windows\system32\target\release\ops.exe" `
         /DArch=x64 /DAppVersion=0.3.0 /DOutputDir="`C:\Windows\system32\dist" `
         installer\ops.iss
``

The server binary appears at `target/release/ops.exe`; the wizard at `dist/install-ops-x64.exe`. Requires Rust stable - nightly is not required.
---

## Configuration

| Environment Variable | Default | Purpose |
|---|---|---|
| `OPS_BREADCRUMB_PATH` | `%LOCALAPPDATA%\Ops\breadcrumbs\` | Breadcrumb storage directory |
| `OPS_SCRIPTS_DIR` | `%LOCALAPPDATA%\Ops\scripts\` | Scripts directory |
| `OPS_STATE_FILE` | `%LOCALAPPDATA%\Ops\state.json` | State file path |
| `RUST_MCP_DIR` | (none) | Override workspace root for git/build tools |

All env vars are optional. Defaults work without any configuration. See `claude_desktop_config.example.json` for a ready-to-paste config block.

---

## Tool Inventory

### File I/O

| Tool | Description |
|------|-------------|
| `read_file` | Read file with smart options: search for pattern, get specific lines, or auto-truncate large files |
| `write_file` | Write file, return confirmation only |
| `append_file` | Append to file, return confirmation only |
| `list_dir` | List directory contents as tree |
| `tail_file` | Return last N lines of a file plus current byte offset |
| `search_file` | Search files by name or content |

### Transforms

| Tool | Description |
|------|-------------|
| `transform_grep` | Search files for pattern, return matching lines with context |
| `transform_extract_lines` | Extract specific line range from file |
| `transform_diff_file` | Compare two files, return diff |
| `transform_find_replace` | Find/replace in file |
| `transform_json_format` | Pretty-print JSON with proper indentation |
| `transform_hash_file` | Compute file checksum (SHA256 via PowerShell) |
| `transform_file_stats` | Get file/directory stats without reading content |

### Sessions

| Tool | Description |
|------|-------------|
| `session_create` | Create a persistent shell session |
| `session_run` | Run command in persistent session |
| `session_cd` | Change directory in session |
| `session_set_env` | Set environment variable in session |
| `session_get_env` | Get environment variable(s) from session |
| `session_list` | List all active sessions with their state |
| `session_destroy` | Destroy a session and kill its PowerShell process |

### Breadcrumbs

| Tool | Description |
|------|-------------|
| `breadcrumb_start` | Start tracked operation with planned steps |
| `breadcrumb_step` | Log step completion, auto-advances to next |
| `breadcrumb_complete` | Mark operation complete, trigger extraction review |
| `breadcrumb_abort` | Abort current operation with reason |
| `breadcrumb_status` | Get current operation status and progress |
| `breadcrumb_backup` | Snapshot breadcrumb state before irreversible ops |

### Reminders

| Tool | Description |
|------|-------------|
| `reminder_add` | Create reminder with natural language time parsing |
| `reminder_list` | List reminders with optional filter |
| `reminder_complete` | Mark reminder completed |
| `reminder_delete` | Permanently remove a reminder |
| `reminder_check_due` | Return all reminders that are due now or overdue |
| `reminder_add_recurring` | Add recurring reminder (daily/weekly/monthly) |
| `reminder_add_scheduled` | Create Windows Task Scheduler reminder |
| `reminder_list_scheduled` | List Windows Task Scheduler CPC reminders |
| `reminder_delete_scheduled` | Delete Windows Task Scheduler reminder by name |
| `system_time_check` | Check elapsed time and re-surface reminders if 3+ hours passed |

### Health

| Tool | Description |
|------|-------------|
| `system_health_check` | Check server health and update dashboard |
| `system_health_report` | Get current health dashboard |
| `server_health` | Check which MCP servers are alive |

### Config

| Tool | Description |
|------|-------------|
| `config_backup` | Backup claude_desktop_config.json with a timestamp before editing |
| `config_validate` | Validate claude_desktop_config.json: parse JSON and check structure |
| `mcp_rebuild` | Rebuild an MCP server with backup |

### Build/Deploy

| Tool | Description |
|------|-------------|
| `deploy_preflight` | Pre-kill safety checks before deploying/rebuilding an MCP server |
| `deploy_smoke_test` | Validate MCP server binaries before packaging |
| `git_rollback` | Rollback rust-mcp repo to a previous commit |

### Archives

| Tool | Description |
|------|-------------|
| `archive_create` | Create zip/tar/tar.gz archive |
| `archive_extract` | Extract zip/tar/tar.gz archive |
| `md2docx` | Convert Markdown to DOCX via pandoc |

### Shell

| Tool | Description |
|------|-------------|
| `powershell` | Execute PowerShell (4-tier blocklist enforced) |

### Cross-AI

| Tool | Description |
|------|-------------|
| `dead_drop_leave` | Leave message in dead drop for other AI agents to find at boot |
| `dead_drop_check` | Check dead drop for unread messages |
| `dead_drop_clear` | Mark dead drop messages as read |
| `bag_tag` | Tag items into the in-memory bag for later retrieval |
| `bag_read` | Read current bag contents |
| `bag_clear` | Clear the bag |

### Recovery

| Tool | Description |
|------|-------------|
| `checkpoint_save` | Save working memory state (survives context compaction) |
| `checkpoint_load` | Load last checkpoint |
| `checkpoint_clear` | Clear checkpoint after task completion |

### Utility

| Tool | Description |
|------|-------------|
| `clipboard_read` | Read from Windows clipboard |
| `clipboard_write` | Write to Windows clipboard |
| `notify` | Show a Windows toast notification |
| `kill_process` | Kill process by PID |
| `list_process` | List processes, optionally filtered by name |
| `port_check` | Test TCP connectivity to a host:port |
| `sqlite_query` | Execute a read-only SQL query against a SQLite database |
| `system_info` | Get OS, CPU, RAM, disk info |

### Misc

| Tool | Description |
|------|-------------|
| `status` | Check system or topic status |
| `tool_fallback` | Look up fallback tool when primary is unavailable |

68 tools across 14 categories.

---

## Safety: Command Blocklist

`ops` enforces a 4-tier safety system on `powershell` and `session_run`. All other tools (file I/O, breadcrumbs, reminders, archives, etc.) are unrestricted.

**Tier numbering:** 4 is strictest (always blocked), 1 is loosest (allowed by default).

| Tier | Trigger | Required flag |
|------|---------|---------------|
| 4 - Catastrophic | `Remove-Item C:\`, shadow copy deletion, boot config destruction, LOLBin execution, etc. | **Always blocked** |
| 3 - Destructive | Drive format, account deletion, bulk system-path deletion | `allow_destructive: true` |
| 2 - System config | Firewall rules, service management, registry writes outside HKCU | `confirm: true` |
| 1 - Everything else | All other commands | Unrestricted |

Full pattern list: `src/security/blocklist.rs`. See [SECURITY.md](SECURITY.md) for tier descriptions.

---

## Failure Modes

`ops` is a thin layer over real OS operations, so failures map directly to what the OS would tell you:

- **Blocked command** - `powershell` and `session_run` return an explicit blocklist-tier error with the matched pattern. Adjust the call or pass the appropriate flag.
- **Command not found / non-zero exit** - tools surface the real exit code and captured stderr. Read the error rather than retrying blindly.
- **State directory not writable** - occurs if `%LOCALAPPDATA%\Ops\` is missing or permissions are wrong. Run `doctor.ps1` to diagnose.
- **Long-running process hangs** - use `session_*` for commands that need interactive state; `powershell` is best for short one-shots with a hard timeout.

---

## Compatible With

`ops` is designed to work standalone - one binary, pointed at by one MCP client, and you have shell + filesystem + breadcrumbs + reminders. Pair it with other CPC servers for broader capabilities:

- [local](https://github.com/AIWander/local) - if you need a public, stable server that ships with hooks and a dashboard
- [manager](https://github.com/AIWander/manager) - multi-backend orchestration on top of ops's execution tools
- [hands](https://github.com/AIWander/hands) - when a script needs to reach into a browser or Windows UI layer
- [workflow](https://github.com/AIWander/workflow) - when scripts call APIs you've graduated from browser discovery to stored HTTP patterns

Host clients: Claude Desktop (`claude_desktop_config.json`), Claude Code (`~/.claude/mcp.json`), OpenAI Codex CLI, or Gemini CLI.

---

## Contributing

Issues welcome; PRs considered but this is primarily maintained as part of the CPC stack.
See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Apache License 2.0 - see [LICENSE](LICENSE).

Copyright 2026 Joseph Wander.

---

## Contact

- **GitHub:** [github.com/AIWander](https://github.com/AIWander/)
- **Email:** josephwander@gmail.com
- **Issues:** [github.com/AIWander/ops/issues](https://github.com/AIWander/ops/issues)
