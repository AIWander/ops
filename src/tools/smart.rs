//! Smart execution routing — ported/adapted from local crate smart.rs.
//! Auto-picks best execution/read path. Adapted for ops: routes command
//! execution through the persistent `sessions` module or a direct PowerShell
//! invocation, and reads through the `xforms` module. The Operating-file TOC
//! routing from the local donor is omitted (ops has no toc module).
//! stdio MCP: never prints to stdout.

use super::{sessions, xforms};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn recovery_dir() -> PathBuf {
    crate::config::get_config().data_dir.join("smart")
}

fn fallbacks_path() -> PathBuf {
    recovery_dir().join("error_fallbacks.json")
}

fn error_log_path() -> PathBuf {
    recovery_dir().join("error_patterns.jsonl")
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct ErrorPattern {
    trigger: String,
    symptom: String,
    fallback: String,
    success_rate: f64,
    occurrences: u32,
}

fn load_fallbacks() -> HashMap<String, ErrorPattern> {
    if let Ok(content) = fs::read_to_string(fallbacks_path()) {
        if let Ok(patterns) = serde_json::from_str(&content) {
            return patterns;
        }
    }
    // Default patterns
    let mut patterns = HashMap::new();
    patterns.insert(
        "path_spaces_cmd".into(),
        ErrorPattern {
            trigger: "raw_run".into(),
            symptom: "syntax is incorrect".into(),
            fallback: "powershell".into(),
            success_rate: 0.95,
            occurrences: 0,
        },
    );
    patterns.insert(
        "timeout_raw".into(),
        ErrorPattern {
            trigger: "raw_run".into(),
            symptom: "timeout".into(),
            fallback: "powershell".into(),
            success_rate: 0.80,
            occurrences: 0,
        },
    );
    patterns
}

fn save_fallbacks(patterns: &HashMap<String, ErrorPattern>) {
    let _ = fs::create_dir_all(recovery_dir());
    if let Ok(content) = serde_json::to_string_pretty(&patterns) {
        let _ = fs::write(fallbacks_path(), content);
    }
}

fn log_error_attempt(tool: &str, error: &str, fallback: Option<&str>, success: Option<bool>) {
    let entry = json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "tool": tool,
        "error_message": error,
        "fallback_tried": fallback,
        "fallback_success": success
    });
    let _ = fs::create_dir_all(recovery_dir());
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(error_log_path())
    {
        let _ = writeln!(file, "{}", entry);
    }
}

fn find_fallback(
    route: &str,
    error_msg: &str,
    patterns: &HashMap<String, ErrorPattern>,
) -> Option<(String, String)> {
    let error_lower = error_msg.to_lowercase();
    for (id, pattern) in patterns {
        if pattern.trigger.to_lowercase() == route.to_lowercase()
            && error_lower.contains(&pattern.symptom.to_lowercase())
        {
            return Some((id.clone(), pattern.fallback.clone()));
        }
    }
    None
}

fn is_error_result(result: &Value) -> Option<String> {
    if let Some(err) = result.get("error").and_then(|v| v.as_str()) {
        return Some(err.to_string());
    }
    if result.get("success") == Some(&json!(false)) {
        if let Some(stderr) = result.get("stderr").and_then(|v| v.as_str()) {
            if !stderr.is_empty() {
                return Some(stderr.to_string());
            }
        }
    }
    if let Some(s) = result.as_str() {
        if s.starts_with("[ERROR]") {
            return Some(s.to_string());
        }
    }
    None
}

fn update_pattern_stats(
    patterns: &mut HashMap<String, ErrorPattern>,
    pattern_id: &str,
    success: bool,
) {
    if let Some(pattern) = patterns.get_mut(pattern_id) {
        pattern.occurrences += 1;
        let new_rate = if success { 1.0 } else { 0.0 };
        pattern.success_rate = (pattern.success_rate * (pattern.occurrences - 1) as f64 + new_rate)
            / pattern.occurrences as f64;
    }
}

/// Tool definitions
pub fn get_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "smart_exec",
            "description": "Auto-routing command execution. Analyzes command and routes to a direct shell run (simple), a persistent session (needs env/cwd), or PowerShell (PS syntax). Returns which route was used.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Command to execute" },
                    "cwd": { "type": "string", "description": "Working directory (triggers session mode)" },
                    "needs_env": { "type": "boolean", "default": false, "description": "If true, uses persistent session" }
                },
                "required": ["command"]
            }
        }),
        json!({
            "name": "smart_read",
            "description": "Auto-routing file read. Routes to a plain read (default), transform_grep (pattern search), transform_extract_lines (specific lines), or transform_diff_files (comparison).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read" },
                    "find": { "type": "string", "description": "Search for pattern (uses grep)" },
                    "lines": { "type": "string", "description": "Line range like '50:100'" },
                    "compare_to": { "type": "string", "description": "Compare with another file (returns diff)" }
                },
                "required": ["path"]
            }
        }),
    ]
}

/// Execute smart tool
pub fn execute(name: &str, args: &Value) -> Value {
    match name {
        "smart_exec" => smart_exec(args),
        "smart_read" => smart_read(args),
        _ => json!({"error": format!("Unknown smart tool: {}", name)}),
    }
}

/// Direct one-shot PowerShell invocation (raw_run / powershell route).
fn run_powershell(command: &str) -> Value {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", command]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            json!({
                "success": out.status.success(),
                "output": stdout,
                "stderr": stderr,
                "exit_code": out.status.code().unwrap_or(-1)
            })
        }
        Err(e) => json!({"error": format!("[ERROR] {}", e)}),
    }
}

fn execute_route(route: &str, command: &str, cwd: Option<&str>) -> Value {
    match route {
        "session_run" => {
            if let Some(dir) = cwd {
                let _ =
                    sessions::execute("session_create", &json!({"name": "default", "cwd": dir}));
            } else {
                let _ = sessions::execute("session_create", &json!({"name": "default"}));
            }
            sessions::execute(
                "session_run",
                &json!({"session": "default", "command": command}),
            )
        }
        _ => run_powershell(command),
    }
}

fn smart_exec(args: &Value) -> Value {
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let cwd = args.get("cwd").and_then(|v| v.as_str());
    let needs_env = args
        .get("needs_env")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Detect PowerShell syntax
    let needs_powershell = command.contains('$')
        || command.contains("Get-")
        || command.contains("Set-")
        || command.contains("New-Item")
        || command.contains("Remove-Item")
        || command.contains("Where-Object")
        || command.contains("-ErrorAction")
        || command.contains("Select-Object")
        || command.contains("Format-Table");

    // Detect session needs
    let needs_session = needs_env
        || cwd.is_some()
        || command.contains("cargo ")
        || command.contains("npm ")
        || command.contains("pip ")
        || command.starts_with("cd ")
        || command.contains(" && cd ")
        || command.starts_with("set ")
        || command.starts_with("export ");

    // Select initial route
    let route = if needs_session {
        "session_run"
    } else if needs_powershell {
        "powershell"
    } else {
        "raw_run"
    };

    // First attempt
    let result = execute_route(route, command, cwd);

    // Check for error and try fallback
    if let Some(error_msg) = is_error_result(&result) {
        let mut patterns = load_fallbacks();

        if let Some((pattern_id, fallback_route)) = find_fallback(route, &error_msg, &patterns) {
            log_error_attempt(route, &error_msg, Some(&fallback_route), None);

            let fallback_result = execute_route(&fallback_route, command, cwd);
            let fallback_success = is_error_result(&fallback_result).is_none();

            update_pattern_stats(&mut patterns, &pattern_id, fallback_success);
            save_fallbacks(&patterns);

            log_error_attempt(
                &fallback_route,
                &is_error_result(&fallback_result).unwrap_or_default(),
                None,
                Some(fallback_success),
            );

            return json!({
                "routed_to": route,
                "fallback_used": fallback_route,
                "fallback_reason": error_msg,
                "result": fallback_result
            });
        }

        log_error_attempt(route, &error_msg, None, None);
    }

    json!({
        "routed_to": route,
        "result": result
    })
}

fn smart_read(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let find = args.get("find").and_then(|v| v.as_str());
    let lines = args.get("lines").and_then(|v| v.as_str());
    let compare_to = args.get("compare_to").and_then(|v| v.as_str());

    let route: &str;
    let result: Value;

    if let Some(pattern) = find {
        route = "transform_grep";
        result = xforms::execute(
            "transform_grep",
            &json!({
                "path": path,
                "pattern": pattern,
                "context": 2
            }),
        );
    } else if let Some(range) = lines {
        route = "transform_extract_lines";
        let parts: Vec<&str> = range.split(':').collect();
        if parts.len() == 2 {
            let start: i64 = parts[0].parse().unwrap_or(1);
            let end: i64 = parts[1].parse().unwrap_or(-1);
            result = xforms::execute(
                "transform_extract_lines",
                &json!({
                    "path": path,
                    "start": start,
                    "end": end
                }),
            );
        } else {
            return json!({"error": "lines format: 'start:end' e.g. '50:100'"});
        }
    } else if let Some(other) = compare_to {
        route = "transform_diff_files";
        result = xforms::execute(
            "transform_diff_files",
            &json!({
                "file_a": path,
                "file_b": other
            }),
        );
    } else {
        route = "read_file";
        result = super::files::execute(
            "read_file",
            &json!({
                "path": path,
                "max_kb": 50
            }),
        );
    }

    json!({
        "routed_to": route,
        "result": result
    })
}
