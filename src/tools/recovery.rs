//! Recovery store tools — ported/merged from local crate raw.rs (recovery_*)
//! and Programmer-Wander config.rs (session_* recovery variants).
//! File-backed recovery store of crashed sessions + interrupted checkpoints.
//! stdio MCP: never prints to stdout.

use serde_json::{json, Value};

const RECOVERY_FILE: &str = "C:\\temp\\mcp_recovery.json";

fn load_recovery() -> Value {
    match std::fs::read_to_string(RECOVERY_FILE) {
        Ok(content) => {
            serde_json::from_str(&content).unwrap_or(json!({"sessions": [], "checkpoints": []}))
        }
        Err(_) => json!({"sessions": [], "checkpoints": []}),
    }
}

fn save_recovery(data: &Value) {
    let _ = std::fs::create_dir_all("C:\\temp");
    let _ = std::fs::write(
        RECOVERY_FILE,
        serde_json::to_string_pretty(data).unwrap_or_default(),
    );
}

pub fn get_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "recovery_status",
            "description": "Check for recoverable sessions and pending checkpoints.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "recovery_resume",
            "description": "Resume an interrupted operation from checkpoint.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checkpoint_id": { "type": "string", "description": "Checkpoint ID to resume" }
                },
                "required": ["checkpoint_id"]
            }
        }),
        json!({
            "name": "recovery_clear",
            "description": "Clear all recovery data.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "session_recovery_status",
            "description": "Check recovery status - shows recoverable sessions and resumable operations.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "session_recover_data",
            "description": "Get recovery data for a crashed session.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" }
                },
                "required": ["session_id"]
            }
        }),
        json!({
            "name": "session_resume_op",
            "description": "Resume an interrupted long-running operation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "checkpoint_id": { "type": "string" }
                },
                "required": ["checkpoint_id"]
            }
        }),
        json!({
            "name": "session_clear_recovery",
            "description": "Clear all recovery data.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
    ]
}

pub fn execute(name: &str, args: &Value) -> Value {
    match name {
        "recovery_status" | "session_recovery_status" => recovery_status(args),
        "recovery_resume" | "session_resume_op" => resume_operation(args),
        "recovery_clear" | "session_clear_recovery" => clear_recovery(args),
        "session_recover_data" => recover_session_data(args),
        _ => json!({"error": format!("Unknown recovery tool: {}", name)}),
    }
}

fn recovery_status(_args: &Value) -> Value {
    let data = load_recovery();
    json!({
        "recoverable_sessions": data["sessions"].as_array().map(|a| a.len()).unwrap_or(0),
        "pending_checkpoints": data["checkpoints"].as_array().map(|a| a.len()).unwrap_or(0),
        "data": data
    })
}

fn resume_operation(args: &Value) -> Value {
    let checkpoint_id = args["checkpoint_id"].as_str().unwrap_or("");
    let data = load_recovery();

    if let Some(checkpoints) = data["checkpoints"].as_array() {
        for cp in checkpoints {
            if cp["checkpoint_id"].as_str() == Some(checkpoint_id) {
                return json!({"success": true, "checkpoint": cp.clone()});
            }
        }
    }
    json!({"success": false, "error": format!("Checkpoint {} not found", checkpoint_id)})
}

fn recover_session_data(args: &Value) -> Value {
    let session_id = args["session_id"].as_str().unwrap_or("");
    let data = load_recovery();

    if let Some(sessions) = data["sessions"].as_array() {
        for session in sessions {
            if session["session_id"].as_str() == Some(session_id) {
                return json!({"success": true, "session": session.clone()});
            }
        }
    }
    json!({"success": false, "error": format!("Session {} not found", session_id)})
}

fn clear_recovery(_args: &Value) -> Value {
    save_recovery(&json!({"sessions": [], "checkpoints": []}));
    json!({"success": true, "message": "Recovery data cleared"})
}
