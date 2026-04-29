# Security Policy

## Our Promise

We don't send anything to outside servers we or anyone else owns beyond your own AI
subscription. This service does not establish any new outside API calls unless you the
user direct it to do so. It is completely contained within your computer after you install it.

## Supported Versions

The latest minor version receives security updates.

## Reporting Security Issues

Please open a [GitHub Issue](https://github.com/AIWander/ops/issues) or email josephwander@gmail.com.

---

## Command Blocklist

`ops` enforces a 4-tier safety system on the `powershell` and `session_run` tools.
All other tools (file I/O, breadcrumbs, reminders, etc.) are unrestricted.

**Tier numbering:** 4 is strictest (always blocked), 1 is loosest (allowed by default).

### Tier 4 - Always blocked (catastrophic)

Commands that delete system files, format drives, corrupt the registry, destroy backups,
disable security tools, or execute arbitrary downloaded payloads.
Blocked unconditionally regardless of any flag.
Examples: `Remove-Item C:\`, `format C:`, `vssadmin delete shadows`, `mshta https://...`.

### Tier 3 - Require `allow_destructive: true`

Commands with irreversible local consequences: drive reformat, account deletion,
bulk system-path deletion, driver removal.
Pass `allow_destructive: true` in the tool call to proceed.

### Tier 2 - Require `confirm: true`

Commands that modify system configuration: firewall rules, service start/stop,
registry writes outside HKCU, scheduled task creation.
Pass `confirm: true` in the tool call to proceed.

### Tier 1 - Unrestricted

Everything else runs without restriction.

Full pattern list: `src/security/blocklist.rs`.
