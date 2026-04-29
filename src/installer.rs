//! Installer subcommands.
//!
//! When the Inno Setup wizard finishes copying files it runs
//! `ops.exe register --binary <path>` and on uninstall it runs
//! `ops.exe unregister`. This module implements both.
//!
//! These subcommands intentionally live in the same binary as the MCP
//! server (instead of a separate helper exe) so the installer ships
//! exactly one file into `%LOCALAPPDATA%\Ops\`.
//!
//! Behaviour overview:
//!
//! - On `register`: the path to ops.exe is **always** copied to the
//!   clipboard, so the user has it ready to paste into any MCP-compatible
//!   client (Claude Desktop, Claude Code, Codex CLI, Gemini CLI, future
//!   clients). This is the universal-fallback channel.
//! - If `%APPDATA%\Claude\` already exists (Claude Desktop has been
//!   launched at least once), we additionally back up and update
//!   `claude_desktop_config.json` so Claude Desktop sees the server on
//!   next start.
//! - If `%APPDATA%\Claude\` does NOT exist, we deliberately do **not**
//!   create it. The user might be a Claude Code / Codex / Gemini user
//!   who never installs the desktop app, and creating a phantom config
//!   directory in Roaming AppData is rude. The clipboard path is enough
//!   for them.
//!
//! What we do not assume: where `Claude.exe` itself is installed. The
//! config path `%APPDATA%\Claude\claude_desktop_config.json` is fixed by
//! Claude Desktop independent of where its binary lives.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde_json::{Map, Value};

/// Entry point called from `main()` when argv looks like a subcommand.
/// Returns Ok(true) if a subcommand was handled (caller should exit),
/// Ok(false) if argv didn't match (caller should fall through to the
/// MCP stdio loop), Err on failure.
pub fn dispatch(argv: &[String]) -> Result<bool> {
    let cmd = match argv.get(1).map(String::as_str) {
        Some("register") => "register",
        Some("unregister") => "unregister",
        _ => return Ok(false),
    };

    match cmd {
        "register" => {
            let bin = parse_flag(argv, "--binary").ok_or_else(|| {
                anyhow!("`register` requires --binary <path-to-ops.exe>")
            })?;
            let bin_path = PathBuf::from(bin);
            if !bin_path.exists() {
                return Err(anyhow!("binary does not exist at {}", bin_path.display()));
            }
            register(&bin_path)?;
        }
        "unregister" => {
            unregister()?;
        }
        _ => unreachable!(),
    }
    Ok(true)
}

fn parse_flag(argv: &[String], name: &str) -> Option<String> {
    let mut iter = argv.iter();
    while let Some(a) = iter.next() {
        if a == name {
            return iter.next().cloned();
        }
        if let Some(rest) = a.strip_prefix(&format!("{name}=")) {
            return Some(rest.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

fn claude_config_path() -> Result<PathBuf> {
    // Per-user roaming AppData. On Windows: C:\Users\<u>\AppData\Roaming
    let appdata = dirs::config_dir().ok_or_else(|| anyhow!("could not resolve %APPDATA%"))?;
    Ok(appdata.join("Claude").join("claude_desktop_config.json"))
}

/// Whether Claude Desktop appears to have been launched on this account.
/// We use the existence of the Claude config directory as the signal -
/// not the presence of Claude.exe, because Claude.exe install location
/// varies per machine while the config dir does not.
fn claude_dir_exists(config_path: &Path) -> bool {
    config_path
        .parent()
        .map(|p| p.exists())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Backup + JSON I/O
// ---------------------------------------------------------------------------

fn backup_config(config_path: &Path) -> Result<Option<PathBuf>> {
    if !config_path.exists() {
        return Ok(None);
    }
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup = config_path.with_file_name(format!(
        "claude_desktop_config.{}.bak.json",
        stamp
    ));
    std::fs::copy(config_path, &backup)
        .with_context(|| format!("backing up {}", config_path.display()))?;
    Ok(Some(backup))
}

fn load_or_init(config_path: &Path) -> Result<Value> {
    if !config_path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let raw = std::fs::read_to_string(config_path)
        .with_context(|| format!("reading {}", config_path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", config_path.display()))
}

fn write_pretty(config_path: &Path, root: &Value) -> Result<()> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let pretty = serde_json::to_string_pretty(root)?;
    std::fs::write(config_path, pretty)
        .with_context(|| format!("writing {}", config_path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

/// Best-effort clipboard write. Failures are non-fatal - a missing
/// clipboard is only a friction problem, not a correctness problem, and
/// we still print the path to stdout as a fallback.
fn copy_to_clipboard(value: &str) -> bool {
    use arboard::Clipboard;
    match Clipboard::new() {
        Ok(mut cb) => cb.set_text(value.to_string()).is_ok(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// register / unregister
// ---------------------------------------------------------------------------

fn register(ops_exe: &Path) -> Result<()> {
    let ops_exe_str = ops_exe.to_string_lossy().into_owned();
    let config_path = claude_config_path()?;

    // Always copy the install path to the clipboard. This is the universal
    // fallback channel: works for Claude Desktop, Claude Code, Codex CLI,
    // Gemini CLI, anything that takes a stdio MCP server command.
    let clipboard_ok = copy_to_clipboard(&ops_exe_str);
    println!("ops binary: {}", ops_exe_str);
    if clipboard_ok {
        println!("Path copied to clipboard. Paste it into your MCP client config if needed.");
    }

    if !claude_dir_exists(&config_path) {
        // Don't create a phantom Claude config directory. The user might
        // not be a Claude Desktop user at all - Claude Code / Codex /
        // Gemini CLI users only need the clipboard path.
        println!(
            "Claude Desktop not detected (no {} directory).",
            config_path.parent().map(|p| p.display().to_string()).unwrap_or_default()
        );
        println!("Skipping claude_desktop_config.json edit.");
        println!("If you install Claude Desktop later, paste the clipboard path into:");
        println!("  {}", config_path.display());
        return Ok(());
    }

    // Claude Desktop is installed - update its config.
    let backup = backup_config(&config_path)?;

    let mut root = load_or_init(&config_path)?;
    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("claude_desktop_config.json must be a JSON object at the root"))?;

    let servers = root_obj
        .entry("mcpServers".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let servers_obj = servers
        .as_object_mut()
        .ok_or_else(|| anyhow!("`mcpServers` must be a JSON object"))?;

    let mut entry = Map::new();
    entry.insert("command".to_string(), Value::String(ops_exe_str));
    servers_obj.insert("ops".to_string(), Value::Object(entry));

    write_pretty(&config_path, &root)?;

    println!("Registered `ops` in {}", config_path.display());
    if let Some(p) = backup {
        println!("Previous config backed up to {}", p.display());
    }
    Ok(())
}

fn unregister() -> Result<()> {
    let config_path = claude_config_path()?;
    if !config_path.exists() {
        // Idempotent: nothing to do.
        return Ok(());
    }
    let backup = backup_config(&config_path)?;

    let mut root = load_or_init(&config_path)?;
    let root_obj = match root.as_object_mut() {
        Some(o) => o,
        None => return Ok(()),
    };

    if let Some(Value::Object(servers)) = root_obj.get_mut("mcpServers") {
        servers.shift_remove("ops");
    }

    write_pretty(&config_path, &root)?;

    println!("Unregistered `ops` from {}", config_path.display());
    if let Some(p) = backup {
        println!("Previous config backed up to {}", p.display());
    }
    Ok(())
}
