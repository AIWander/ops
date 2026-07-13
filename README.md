# ops

**One MCP server. Give any AI local files, shells, and persistent sessions.**

ops is a STDIO MCP server that gives any AI client a real, local execution layer on Windows: read and write files, run shell commands, manage persistent sessions, track multi-step operations, install whatever environments your AI needs (Python, ffmpeg, Node, you name it) — all running directly on your computer. Wire ops into Claude Desktop, Cowork, Claude Code, Codex CLI, Gemini CLI, LM Studio, or anything else that speaks STDIO MCP, and your AI gets the kind of hands-on local-agent abilities that closed products like Codex ship as their headline feature. Same idea, but you bring your own AI and your own machine.

[![CI](https://github.com/AIWander/ops/actions/workflows/ci.yml/badge.svg)](https://github.com/AIWander/ops/actions/workflows/ci.yml)

Version 0.3.1 · Apache 2.0 · [GitHub](https://github.com/AIWander/ops)

**Part of [CPC](https://github.com/AIWander) (Copy Paste Compute)** — a multi-agent AI orchestration platform. Companion repos: [AI-Hands](https://github.com/AIWander/AI-Hands) · [workflow](https://github.com/AIWander/workflow) · [manager-universal](https://github.com/AIWander/manager-universal) (manager/dashboard Beta and coming soon) · [Voice-Command](https://github.com/AIWander/Voice-Command)

---

## 🔒 Stays on your computer

ops runs locally. File reads, shell commands, sessions, breadcrumbs, reminders — all of it happens on your machine. **ops itself never reaches out to the internet on its own.** Your AI may reach out (Claude pings Anthropic, ChatGPT pings OpenAI, etc.), but ops adds zero outbound traffic of its own. If your AI uses ops to *download* something (e.g. `winget install python`), that's the AI's call, executed locally through ops's shell tools.

---

## What you can do with it

Once your AI has ops, it can:

- **Read and write files** on your machine — anywhere you have permission
- **Run shell commands** in PowerShell or Bash (via Git Bash) with a 4-tier safety blocklist
- **Hold persistent shell sessions** that remember `cwd`, env vars, and state across calls
- **Install whatever environments it needs** — `winget install python`, `pip install`, `npm install`, `cargo install`, you name it. If a script needs Python and Python isn't there, your AI can fetch it.
- **Track multi-step operations** with breadcrumbs that survive context compaction
- **Set reminders** with natural-language time parsing, including Windows Task Scheduler entries
- **Manage MCP servers** — backup configs, rebuild binaries, validate setup
- **Coordinate with other AI sessions** via dead-drop messaging and bag-tagging
- **Notify you** with toast notifications, read/write the clipboard, kill processes, query SQLite databases

68 tools total across file I/O, transforms, shell, sessions, archives, breadcrumbs, reminders, recovery, build/deploy, MCP server management, and cross-AI coordination. Full inventory below.

---

## Works with

ops speaks STDIO MCP, so it plugs into any AI client that does:

- **Claude** (chat) — Claude Desktop and the web app
- **Cowork** — Claude's desktop agent
- **Claude Code** — the CLI coding agent
- **Codex CLI** — OpenAI / GPT
- **Gemini CLI** — Google
- **LM Studio** — for running local models (Llama, Qwen, Mistral, etc.)
- **Anything else that calls STDIO MCP servers** — the protocol is the only requirement

---

## Install — three paths, in order of effort

### 1. Easiest: one-click installer (Windows x64)

If you're on **Windows x64** and don't want to think about it:

1. Download both `install-ops-x64.exe` and `ops-x64.exe` from the [latest release](https://github.com/AIWander/ops/releases/latest).
2. Drop both files in the same folder.
3. Double-click `install-ops-x64.exe` (or run `install-ops-x64.exe --binary ops-x64.exe` from a terminal).
4. Pick **Program Files** (admin install) or **LocalAppData** (no admin needed).
5. Restart your AI client.

The installer:
- Backs up `claude_desktop_config.json` with a timestamp before touching it
- Copies `ops.exe` to the location you picked
- Adds (or updates) only the `ops` entry in your MCP config — touches nothing else
- Prints the backup path so you can revert if anything looks wrong

After restarting, toggle the `ops` connector **on** in **Settings → Connectors** and you're done.

### 2. Have your AI install it for you

If you have **Claude Desktop**, **Cowork**, **Claude Code**, **Codex CLI**, or **Gemini CLI** open and your AI has any kind of file-write or shell access, copy this and paste it to your AI:

> `https://github.com/AIWander/ops` — Can you install this MCP server for me. Grab the right binary for my computer (x64 or ARM64) from the latest release, drop it somewhere sensible, wire it into my MCP client's config (back up the existing config first), and tell me when everything's ready and I need to restart.

Your AI will fetch the right binary, edit the config (with a timestamped backup), and tell you when to restart.

> If your AI doesn't yet have a way to read/write files or run shell commands, use the one-click installer above — that's the bootstrap path.

### 3. Programmer / manual

**ARM64 Windows** (no installer yet — manual is the only path on ARM64):

1. Download `ops-aarch64.exe` from the [latest release](https://github.com/AIWander/ops/releases/latest), rename to `ops.exe`.
2. Drop it somewhere permanent, e.g. `%LOCALAPPDATA%\Ops\ops.exe`.
3. Edit `%APPDATA%\Claude\claude_desktop_config.json` and add:
   ```json
   {
     "mcpServers": {
       "ops": {
         "command": "C:\\Users\\YourName\\AppData\\Local\\Ops\\ops.exe"
       }
     }
   }
   ```
4. Restart Claude Desktop.

**Build from source** (any platform with Rust toolchain):

```bash
git clone https://github.com/AIWander/ops.git
cd ops
cargo build --release
```

Binary lands at `target/release/ops.exe`. Stable Rust toolchain — no nightly required.

### Prerequisites

- Windows 10 or 11 (x64 or ARM64)
- A Claude Desktop or other MCP-compatible client

---

## Configuration

| Environment Variable | Default | Purpose |
|---|---|---|
| `OPS_BREADCRUMB_PATH` | `%LOCALAPPDATA%\Ops\breadcrumbs\` | Breadcrumb storage |
| `OPS_SCRIPTS_DIR` | `%LOCALAPPDATA%\Ops\scripts\` | Scripts directory |
| `OPS_STATE_FILE` | `%LOCALAPPDATA%\Ops\state.json` | State file |
| `RUST_MCP_DIR` | (none) | Override workspace root for git/build tools |
| `OPS_BASH_PATH` | auto-discovered | Override Git Bash binary path |

Defaults work without configuration. See `claude_desktop_config.example.json` for a ready-to-paste config block.

---

## Safety: the blocklist

ops's `powershell`, `bash`, and `session_run` tools enforce a 4-tier safety system. All other tools (file I/O, breadcrumbs, reminders, archives, etc.) are unrestricted.

**Tier 4 is strictest (always blocked); Tier 1 is allowed by default.**

| Tier | Trigger | Required flag |
|------|---------|---------------|
| 4 — Catastrophic | `Remove-Item C:\`, shadow copy deletion, fork bombs, `dd` to raw devices, `curl \| sh` from network, BitLocker disable, etc. | **Always blocked** |
| 3 — Destructive | Drive format, account deletion, bulk system-path deletion, `mkfs`/`shred` | `allow_destructive: true` |
| 2 — System config | Firewall rules, service management, registry writes outside HKCU, scheduled task changes | `confirm: true` |
| 1 — Everything else | All other commands | Unrestricted |

Full pattern list: [`src/security/blocklist.rs`](src/security/blocklist.rs). Tier descriptions: [SECURITY.md](SECURITY.md).

---

## What's New in v0.3.1

- Aligned breadcrumb shape with manager-universal's dual-source reader — frontends can now render breadcrumbs from ops and manager interchangeably.

### v0.3.0 highlights

| Feature | Detail |
|---------|--------|
| `bash` tool | Execute commands via Git Bash — mirrors `powershell` with `allow_destructive` and `confirm` flags |
| Extended blocklist | Unix-shaped T4/T3/T2 patterns: dd-to-device, fork bombs, rm -rf root, curl\|sh, chmod 777, mkfs/shred, systemctl, iptables, apt |
| Path resolution | `OPS_BASH_PATH` env override → standard Git Bash locations → PATH fallback |
| 69 tools total | +1 from v0.2.1 |

---

## Tool reference

<details>
<summary><strong>Click to expand the full tool inventory (68 tools across 14 categories)</strong></summary>

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
| `config_backup` | Backup `claude_desktop_config.json` with a timestamp before editing |
| `config_validate` | Validate `claude_desktop_config.json`: parse JSON and check structure |
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
| `bash` | Execute commands via Git Bash (4-tier blocklist enforced) |

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

</details>

---

## Failure modes

ops is a thin layer over real OS operations, so failures map directly to what the OS would tell you:

- **Blocked command** — `powershell`, `bash`, and `session_run` return an explicit blocklist-tier error with the matched pattern. Adjust the call or pass the appropriate flag.
- **Command not found / non-zero exit** — tools surface the real exit code and captured stderr. Read the error rather than retrying blindly.
- **State directory not writable** — usually means `%LOCALAPPDATA%\Ops\` is missing or has wrong permissions. Run `doctor.ps1` to diagnose.
- **Long-running process hangs** — use `session_*` for commands that need interactive state; `powershell` is best for short one-shots with a hard timeout.

---

## Pairs nicely with the rest of CPC

ops works standalone — one binary, one MCP client, full shell + filesystem + breadcrumbs + reminders. Pair it with these for broader capabilities:

- **[AI-Hands](https://github.com/AIWander/AI-Hands)** — browser automation, Windows UI control, vision/OCR
- **[workflow](https://github.com/AIWander/workflow)** — API discovery and replay, credential vault, scheduled flows
- **[manager-universal](https://github.com/AIWander/manager-universal)** — manager/dashboard Beta and coming soon; current repository is a test surface
- **[Voice-Command](https://github.com/AIWander/Voice-Command)** — voice-control any AI that has ops; pairs cleanly because ops gives the AI hands and Voice-Command gives it ears and a voice

---

## Contributing

Issues welcome; PRs considered, but this is primarily maintained as part of the CPC stack. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Apache License 2.0 — see [LICENSE](LICENSE). Copyright 2026 Joseph Wander.

---

## Contact

- **GitHub:** [github.com/AIWander](https://github.com/AIWander/)
- **Email:** contact@aiwander.ai
- **Issues:** [github.com/AIWander/ops/issues](https://github.com/AIWander/ops/issues)
