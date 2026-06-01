//! Persistent Shell Sessions — ported from local crate psession.rs
//! Supports PowerShell and WSL (bash) backends - sync version.
//! stdio MCP: never prints to stdout.

use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static PSESSIONS: Lazy<Arc<Mutex<HashMap<String, PersistentSession>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

struct PersistentSession {
    name: String,
    shell_type: String,
    child: Child,
    output_buffer: Arc<Mutex<Vec<String>>>,
    history: Vec<String>,
    created_at: String,
}

fn start_reader(stream: impl std::io::Read + Send + 'static, buffer: Arc<Mutex<Vec<String>>>) {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            buffer.lock().unwrap().push(line);
        }
    });
}

pub fn get_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "psession_create",
            "description": "Create a persistent PowerShell or WSL session that survives across tool calls",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "default": "default", "description": "Session name" },
                    "shell": { "type": "string", "default": "powershell", "description": "Shell type: powershell or wsl" },
                    "cwd": { "type": "string", "description": "Working directory" }
                }
            }
        }),
        json!({
            "name": "psession_run",
            "description": "Run command in a persistent shell session. State (env vars, cd) persists between calls.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID from psession_create" },
                    "command": { "type": "string", "description": "Command to run" },
                    "timeout_secs": { "type": "integer", "default": 30, "description": "Timeout in seconds" }
                },
                "required": ["session_id", "command"]
            }
        }),
        json!({
            "name": "psession_destroy",
            "description": "Kill a persistent shell session",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID to destroy" }
                },
                "required": ["session_id"]
            }
        }),
        json!({
            "name": "psession_list",
            "description": "List all active persistent shell sessions",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "psession_read",
            "description": "Read output buffer from a persistent session",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID" },
                    "tail": { "type": "integer", "default": 20, "description": "Number of lines from end" }
                },
                "required": ["session_id"]
            }
        }),
        json!({
            "name": "psession_history",
            "description": "Get command history for a persistent session",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string", "description": "Session ID" }
                },
                "required": ["session_id"]
            }
        }),
    ]
}

pub fn execute(name: &str, args: &Value) -> Value {
    match name {
        "psession_create" => psession_create(args),
        "psession_run" => psession_run(args),
        "psession_destroy" => psession_destroy(args),
        "psession_list" => psession_list(args),
        "psession_read" => psession_read(args),
        "psession_history" => psession_history(args),
        _ => json!({"error": format!("Unknown psession tool: {}", name)}),
    }
}

fn psession_create(args: &Value) -> Value {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("powershell");
    let default_cwd = std::env::var("WORKSPACE_PATH").unwrap_or_else(|_| "C:\\".to_string());
    let cwd = args
        .get("cwd")
        .and_then(|v| v.as_str())
        .unwrap_or(&default_cwd);

    let mut cmd = match shell {
        "wsl" => {
            let mut c = Command::new("wsl");
            c.args(["-d", "Ubuntu-24.04", "--", "bash"]);
            c
        }
        _ => {
            let mut c = Command::new("powershell");
            c.args(["-NoLogo", "-NoProfile", "-Command", "-"]);
            c
        }
    };

    cmd.current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Failed to spawn {}: {}", shell, e)}),
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return json!({"error": "Failed to take stdout"}),
    };

    let buffer = Arc::new(Mutex::new(Vec::new()));
    start_reader(stdout, buffer.clone());

    if let Some(stderr) = child.stderr.take() {
        start_reader(stderr, buffer.clone());
    }

    thread::sleep(std::time::Duration::from_millis(200));

    let session_id = format!("{}_{}", shell, name);
    let created = chrono::Local::now().to_rfc3339();

    let mut sessions = PSESSIONS.lock().unwrap();
    sessions.insert(
        session_id.clone(),
        PersistentSession {
            name: name.to_string(),
            shell_type: shell.to_string(),
            child,
            output_buffer: buffer,
            history: Vec::new(),
            created_at: created.clone(),
        },
    );

    json!({
        "session_id": session_id,
        "shell": shell,
        "name": name,
        "created_at": created
    })
}

fn psession_run(args: &Value) -> Value {
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    if session_id.is_empty() || command.is_empty() {
        return json!({"error": "session_id and command are required"});
    }

    let mut sessions = PSESSIONS.lock().unwrap();
    let session = match sessions.get_mut(session_id) {
        Some(s) => s,
        None => return json!({"error": format!("Session not found: {}", session_id)}),
    };

    let start_pos = session.output_buffer.lock().unwrap().len();

    let marker = format!(
        "__DONE_{}__",
        uuid::Uuid::new_v4()
            .to_string()
            .get(..8)
            .unwrap_or("00000000")
    );
    let stdin = match session.child.stdin.as_mut() {
        Some(s) => s,
        None => return json!({"error": "stdin not available"}),
    };

    let full_cmd = if session.shell_type == "wsl" {
        format!("{}\necho {}\n", command, marker)
    } else {
        format!("{}\nWrite-Output '{}'\n", command, marker)
    };

    if let Err(e) = stdin.write_all(full_cmd.as_bytes()) {
        return json!({"error": format!("Write failed: {}", e)});
    }
    if let Err(e) = stdin.flush() {
        return json!({"error": format!("Flush failed: {}", e)});
    }

    session.history.push(command.to_string());

    let buffer = session.output_buffer.clone();
    drop(sessions);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let mut output_lines = Vec::new();
    let mut found_marker = false;

    loop {
        if std::time::Instant::now() > deadline {
            break;
        }

        {
            let buf = buffer.lock().unwrap();
            let current_len = buf.len();
            if current_len > start_pos {
                for i in start_pos..current_len {
                    if buf[i].contains(&marker) {
                        found_marker = true;
                        output_lines = buf[start_pos..i].to_vec();
                        break;
                    }
                }
                if found_marker {
                    break;
                }
            }
        }

        thread::sleep(std::time::Duration::from_millis(50));
    }

    if !found_marker {
        let buf = buffer.lock().unwrap();
        if buf.len() > start_pos {
            output_lines = buf[start_pos..].to_vec();
        }
    }

    json!({
        "session_id": session_id,
        "output": output_lines.join("\n"),
        "lines": output_lines.len(),
        "completed": found_marker,
        "timed_out": !found_marker
    })
}

fn psession_destroy(args: &Value) -> Value {
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if session_id.is_empty() {
        return json!({"error": "session_id is required"});
    }

    let mut sessions = PSESSIONS.lock().unwrap();
    if let Some(mut session) = sessions.remove(session_id) {
        let _ = session.child.kill();
        json!({"destroyed": session_id})
    } else {
        json!({"error": format!("Session not found: {}", session_id)})
    }
}

fn psession_list(_args: &Value) -> Value {
    let sessions = PSESSIONS.lock().unwrap();
    let list: Vec<Value> = sessions
        .iter()
        .map(|(id, s)| {
            json!({
                "session_id": id,
                "name": s.name,
                "shell": s.shell_type,
                "history_count": s.history.len(),
                "buffer_lines": s.output_buffer.lock().unwrap().len(),
                "created_at": s.created_at,
            })
        })
        .collect();

    let count = list.len();
    json!({"sessions": list, "count": count})
}

fn psession_read(args: &Value) -> Value {
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let tail_n = args.get("tail").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    if session_id.is_empty() {
        return json!({"error": "session_id is required"});
    }

    let sessions = PSESSIONS.lock().unwrap();
    let session = match sessions.get(session_id) {
        Some(s) => s,
        None => return json!({"error": format!("Session not found: {}", session_id)}),
    };

    let buf = session.output_buffer.lock().unwrap();
    let total = buf.len();
    let start = total.saturating_sub(tail_n);

    json!({
        "session_id": session_id,
        "total_lines": total,
        "tail": buf[start..].join("\n"),
    })
}

fn psession_history(args: &Value) -> Value {
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if session_id.is_empty() {
        return json!({"error": "session_id is required"});
    }

    let sessions = PSESSIONS.lock().unwrap();
    let session = match sessions.get(session_id) {
        Some(s) => s,
        None => return json!({"error": format!("Session not found: {}", session_id)}),
    };

    json!({
        "session_id": session_id,
        "history": session.history,
        "count": session.history.len()
    })
}
