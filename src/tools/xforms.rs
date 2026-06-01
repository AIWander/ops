//! Transform utilities — ported from local crate transforms.rs
//! Extended with base64/csv/json/minify/bulk_rename/scaffold (local) and
//! sync_dir/transform_file (Programmer-Wander).

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn get_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "transform_grep",
            "description": "Search files for pattern, return matching lines with context.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File or directory" },
                    "pattern": { "type": "string", "description": "Search pattern (regex)" },
                    "context": { "type": "integer", "description": "Lines of context (default 0)" },
                    "recursive": { "type": "boolean", "description": "Search subdirs (default false)" }
                },
                "required": ["path", "pattern"]
            }
        }),
        json!({
            "name": "transform_extract_lines",
            "description": "Extract specific line range from file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path" },
                    "start": { "type": "integer", "description": "Start line (1-indexed)" },
                    "end": { "type": "integer", "description": "End line (inclusive, -1 for EOF)" }
                },
                "required": ["path", "start"]
            }
        }),
        json!({
            "name": "transform_diff_file",
            "description": "Compare two files, return diff. Saves loading both files into chat.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_a": { "type": "string", "description": "First file path" },
                    "file_b": { "type": "string", "description": "Second file path" }
                },
                "required": ["file_a", "file_b"]
            }
        }),
        json!({
            "name": "transform_find_replace",
            "description": "Find/replace in file. Saves reading entire file into chat.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path" },
                    "find": { "type": "string", "description": "Text or regex to find" },
                    "replace": { "type": "string", "description": "Replacement text" },
                    "regex": { "type": "boolean", "description": "Use regex (default false)" }
                },
                "required": ["path", "find", "replace"]
            }
        }),
        json!({
            "name": "transform_json_format",
            "description": "Pretty-print JSON with proper indentation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "json_string": { "type": "string", "description": "JSON to format" }
                },
                "required": ["json_string"]
            }
        }),
        json!({
            "name": "transform_hash_file",
            "description": "Compute file checksum (SHA256 via PowerShell).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path" },
                    "algorithm": { "type": "string", "description": "md5 or sha256 (default sha256)" }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "transform_file_stats",
            "description": "Get file/directory stats without reading content.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to analyze" },
                    "recursive": { "type": "boolean", "description": "Include subdirs (default false)" }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "transform_json_minify",
            "description": "Minify JSON by removing whitespace.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "json_string": { "type": "string", "description": "JSON to minify" }
                },
                "required": ["json_string"]
            }
        }),
        json!({
            "name": "transform_base64_encode",
            "description": "Encode string to base64.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to encode" }
                },
                "required": ["text"]
            }
        }),
        json!({
            "name": "transform_base64_decode",
            "description": "Decode base64 to string.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "encoded": { "type": "string", "description": "Base64 to decode" }
                },
                "required": ["encoded"]
            }
        }),
        json!({
            "name": "transform_csv_to_json",
            "description": "Convert CSV to JSON array. First row = headers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "csv_string": { "type": "string", "description": "CSV data" },
                    "delimiter": { "type": "string", "description": "Delimiter (default: comma)" }
                },
                "required": ["csv_string"]
            }
        }),
        json!({
            "name": "transform_json_to_csv",
            "description": "Convert JSON array to CSV.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "json_array": { "type": "string", "description": "JSON array" },
                    "delimiter": { "type": "string", "description": "Delimiter (default: comma)" }
                },
                "required": ["json_array"]
            }
        }),
        json!({
            "name": "transform_bulk_rename",
            "description": "Regex-based batch rename. Returns preview unless execute=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": { "type": "string", "description": "Directory to scan" },
                    "pattern": { "type": "string", "description": "Regex pattern to match" },
                    "replacement": { "type": "string", "description": "Replacement string ($1, $2 for groups)" },
                    "execute": { "type": "boolean", "description": "Actually rename (default: false = preview)" }
                },
                "required": ["directory", "pattern", "replacement"]
            }
        }),
        json!({
            "name": "transform_scaffold",
            "description": "Generate project scaffolding. Creates boilerplate structure.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "template": { "type": "string", "description": "Template: rust-mcp, python-mcp, nextjs, fastapi" },
                    "name": { "type": "string", "description": "Project name" },
                    "output_dir": { "type": "string", "description": "Output directory (default: current)" }
                },
                "required": ["template", "name"]
            }
        }),
        json!({
            "name": "transform_sync_dir",
            "description": "Sync directories with modes: mirror, update, backup.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "destination": { "type": "string" },
                    "mode": { "type": "string", "default": "update" },
                    "dry_run": { "type": "boolean", "default": true },
                    "exclude": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["source", "destination"]
            }
        }),
        json!({
            "name": "transform_file",
            "description": "Apply a Python transform expression to matching files. Requires python on PATH.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "directory": { "type": "string", "default": "." },
                    "pattern": { "type": "string", "description": "Glob-like filename pattern, e.g. *.txt" },
                    "transform_code": { "type": "string", "description": "Python expr over `content`" },
                    "dry_run": { "type": "boolean", "default": true }
                },
                "required": ["pattern", "transform_code"]
            }
        }),
    ]
}

pub fn execute(name: &str, args: &Value) -> Value {
    match name {
        "transform_grep" => grep(args),
        "transform_extract_lines" => extract_lines(args),
        "transform_diff_file" | "transform_diff_files" => diff_files(args),
        "transform_find_replace" => find_replace(args),
        "transform_json_format" => json_format(args),
        "transform_hash_file" => hash_file(args),
        "transform_file_stats" => file_stats(args),
        "transform_json_minify" => json_minify(args),
        "transform_base64_encode" => base64_encode(args),
        "transform_base64_decode" => base64_decode(args),
        "transform_csv_to_json" => csv_to_json(args),
        "transform_json_to_csv" => json_to_csv(args),
        "transform_bulk_rename" => bulk_rename(args),
        "transform_scaffold" => scaffold(args),
        "transform_sync_dir" => sync_dir(args),
        "transform_file" => transform_file(args),
        _ => json!({"error": format!("Unknown transform: {}", name)}),
    }
}

fn grep(args: &Value) -> Value {
    let path = match args["path"].as_str() {
        Some(s) => s,
        None => return json!({"error": "path required"}),
    };
    let pattern = match args["pattern"].as_str() {
        Some(s) => s,
        None => return json!({"error": "pattern required"}),
    };
    let context = args["context"].as_u64().unwrap_or(0) as usize;

    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => return json!({"error": format!("Invalid regex: {}", e)}),
    };

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Can't read: {}", e)}),
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut matches: Vec<Value> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        if re.is_match(line) {
            let start = i.saturating_sub(context);
            let end = (i + context + 1).min(lines.len());
            let context_lines: Vec<String> = lines[start..end]
                .iter()
                .enumerate()
                .map(|(j, l)| format!("{}: {}", start + j + 1, l))
                .collect();
            matches.push(json!({
                "line": i + 1,
                "match": line,
                "context": context_lines
            }));
        }
    }

    json!({"path": path, "pattern": pattern, "matches": matches, "count": matches.len()})
}

fn extract_lines(args: &Value) -> Value {
    let path = match args["path"].as_str() {
        Some(s) => s,
        None => return json!({"error": "path required"}),
    };
    let start = args["start"].as_i64().unwrap_or(1) as usize;
    let end = args["end"].as_i64().unwrap_or(-1);

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return json!({"error": format!("Can't open: {}", e)}),
    };

    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let line_num = i + 1;
            let in_range = line_num >= start && (end < 0 || line_num <= end as usize);
            if in_range {
                line.ok()
            } else {
                None
            }
        })
        .collect();

    json!({"path": path, "start": start, "end": if end < 0 { "EOF".to_string() } else { end.to_string() }, "lines": lines, "count": lines.len()})
}

fn diff_files(args: &Value) -> Value {
    let file_a = match args["file_a"].as_str() {
        Some(s) => s,
        None => return json!({"error": "file_a required"}),
    };
    let file_b = match args["file_b"].as_str() {
        Some(s) => s,
        None => return json!({"error": "file_b required"}),
    };

    let content_a = match std::fs::read_to_string(file_a) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Can't read {}: {}", file_a, e)}),
    };
    let content_b = match std::fs::read_to_string(file_b) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Can't read {}: {}", file_b, e)}),
    };

    let lines_a: Vec<&str> = content_a.lines().collect();
    let lines_b: Vec<&str> = content_b.lines().collect();

    let mut diff_lines: Vec<String> = Vec::new();
    let max_len = lines_a.len().max(lines_b.len());
    let mut changes = 0;

    for i in 0..max_len {
        let a = lines_a.get(i);
        let b = lines_b.get(i);
        match (a, b) {
            (Some(la), Some(lb)) if la != lb => {
                diff_lines.push(format!("{}:- {}", i + 1, la));
                diff_lines.push(format!("{}:+ {}", i + 1, lb));
                changes += 1;
            }
            (Some(la), None) => {
                diff_lines.push(format!("{}:- {}", i + 1, la));
                changes += 1;
            }
            (None, Some(lb)) => {
                diff_lines.push(format!("{}:+ {}", i + 1, lb));
                changes += 1;
            }
            _ => {}
        }
    }

    json!({"diff": diff_lines.join("\n"), "changes": changes, "lines_a": lines_a.len(), "lines_b": lines_b.len()})
}

fn find_replace(args: &Value) -> Value {
    let path = match args["path"].as_str() {
        Some(s) => s,
        None => return json!({"error": "path required"}),
    };
    let find = match args["find"].as_str() {
        Some(s) => s,
        None => return json!({"error": "find required"}),
    };
    let replace = match args["replace"].as_str() {
        Some(s) => s,
        None => return json!({"error": "replace required"}),
    };
    let use_regex = args["regex"].as_bool().unwrap_or(false);

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Can't read: {}", e)}),
    };

    let (new_content, count) = if use_regex {
        match regex::Regex::new(find) {
            Ok(re) => {
                let matches = re.find_iter(&content).count();
                (re.replace_all(&content, replace).to_string(), matches)
            }
            Err(e) => return json!({"error": format!("Invalid regex: {}", e)}),
        }
    } else {
        let count = content.matches(find).count();
        (content.replace(find, replace), count)
    };

    if count > 0 {
        if let Err(e) = std::fs::write(path, &new_content) {
            return json!({"error": format!("Can't write: {}", e)});
        }
    }

    json!({"path": path, "replacements": count})
}

fn json_format(args: &Value) -> Value {
    let json_string = match args["json_string"].as_str() {
        Some(s) => s,
        None => return json!({"error": "json_string required"}),
    };
    match serde_json::from_str::<Value>(json_string) {
        Ok(parsed) => json!({"formatted": serde_json::to_string_pretty(&parsed).unwrap()}),
        Err(e) => json!({"error": format!("Invalid JSON: {}", e)}),
    }
}

fn hash_file(args: &Value) -> Value {
    let path = match args["path"].as_str() {
        Some(s) => s,
        None => return json!({"error": "path required"}),
    };
    let algorithm = args["algorithm"].as_str().unwrap_or("sha256");
    let algo_upper = algorithm.to_uppercase();

    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let result = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-FileHash -Path '{}' -Algorithm {}).Hash",
                path, algo_upper
            ),
        ])
        .output();

    match result {
        Ok(output) => {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            json!({"path": path, "algorithm": algorithm, "hash": hash, "size": size})
        }
        Err(e) => json!({"error": format!("Hash failed: {}", e)}),
    }
}

fn file_stats(args: &Value) -> Value {
    let path = match args["path"].as_str() {
        Some(s) => s,
        None => return json!({"error": "path required"}),
    };
    let recursive = args["recursive"].as_bool().unwrap_or(false);

    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return json!({"error": format!("Can't stat: {}", e)}),
    };

    if meta.is_file() {
        json!({"type": "file", "path": path, "size": meta.len(), "size_human": format_size(meta.len())})
    } else {
        let mut total_size: u64 = 0;
        let mut file_count: u64 = 0;
        let mut dir_count: u64 = 0;
        walk_dir(
            Path::new(path),
            recursive,
            &mut total_size,
            &mut file_count,
            &mut dir_count,
        );
        json!({
            "type": "directory", "path": path,
            "files": file_count, "directories": dir_count,
            "total_size": total_size, "total_size_human": format_size(total_size),
            "recursive": recursive
        })
    }
}

fn walk_dir(p: &Path, recursive: bool, total: &mut u64, files: &mut u64, dirs: &mut u64) {
    if let Ok(entries) = std::fs::read_dir(p) {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata() {
                if m.is_file() {
                    *total += m.len();
                    *files += 1;
                } else if m.is_dir() {
                    *dirs += 1;
                    if recursive {
                        walk_dir(&entry.path(), recursive, total, files, dirs);
                    }
                }
            }
        }
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// === ADDED TRANSFORMS (ported from local transforms.rs + PW transform.rs) ===

fn json_minify(args: &Value) -> Value {
    let json_string = match args["json_string"].as_str() {
        Some(s) => s,
        None => return json!({"error": "json_string required"}),
    };
    match serde_json::from_str::<Value>(json_string) {
        Ok(parsed) => json!({"minified": serde_json::to_string(&parsed).unwrap()}),
        Err(e) => json!({"error": format!("Invalid JSON: {}", e)}),
    }
}

fn base64_encode(args: &Value) -> Value {
    let text = match args["text"].as_str() {
        Some(s) => s,
        None => return json!({"error": "text required"}),
    };
    json!({"encoded": BASE64.encode(text.as_bytes())})
}

fn base64_decode(args: &Value) -> Value {
    let encoded = match args["encoded"].as_str() {
        Some(s) => s,
        None => return json!({"error": "encoded required"}),
    };
    match BASE64.decode(encoded) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(decoded) => json!({"decoded": decoded}),
            Err(_) => json!({"error": "Not valid UTF-8"}),
        },
        Err(e) => json!({"error": format!("Invalid base64: {}", e)}),
    }
}

fn csv_to_json(args: &Value) -> Value {
    let csv = match args["csv_string"].as_str() {
        Some(s) => s,
        None => return json!({"error": "csv_string required"}),
    };
    let delim = args["delimiter"]
        .as_str()
        .unwrap_or(",")
        .chars()
        .next()
        .unwrap_or(',');

    let lines: Vec<&str> = csv.lines().collect();
    if lines.is_empty() {
        return json!({"error": "Empty CSV"});
    }

    let headers: Vec<&str> = lines[0].split(delim).map(|s| s.trim()).collect();
    let records: Vec<Value> = lines[1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let vals: Vec<&str> = line.split(delim).map(|s| s.trim()).collect();
            let mut map = serde_json::Map::new();
            for (i, h) in headers.iter().enumerate() {
                let v = vals.get(i).unwrap_or(&"");
                map.insert(h.to_string(), json!(v));
            }
            Value::Object(map)
        })
        .collect();

    json!({"records": records, "count": records.len()})
}

fn json_to_csv(args: &Value) -> Value {
    let json_str = match args["json_array"].as_str() {
        Some(s) => s,
        None => return json!({"error": "json_array required"}),
    };
    let delim = args["delimiter"].as_str().unwrap_or(",");

    let array: Vec<Value> = match serde_json::from_str(json_str) {
        Ok(a) => a,
        Err(e) => return json!({"error": format!("Invalid JSON: {}", e)}),
    };

    if array.is_empty() {
        return json!({"csv": "", "rows": 0});
    }

    let headers: Vec<String> = match &array[0] {
        Value::Object(obj) => obj.keys().cloned().collect(),
        _ => return json!({"error": "Array must contain objects"}),
    };

    let mut lines = vec![headers.join(delim)];
    for item in &array {
        if let Value::Object(obj) = item {
            let vals: Vec<String> = headers
                .iter()
                .map(|h| {
                    obj.get(h)
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => v.to_string().trim_matches('"').to_string(),
                        })
                        .unwrap_or_default()
                })
                .collect();
            lines.push(vals.join(delim));
        }
    }
    json!({"csv": lines.join("\n"), "rows": array.len()})
}

fn bulk_rename(args: &Value) -> Value {
    let dir = match args["directory"].as_str() {
        Some(s) => s,
        None => return json!({"error": "directory required"}),
    };
    let pattern = match args["pattern"].as_str() {
        Some(s) => s,
        None => return json!({"error": "pattern required"}),
    };
    let replacement = match args["replacement"].as_str() {
        Some(s) => s,
        None => return json!({"error": "replacement required"}),
    };
    // Accept either `execute=true` (local) or `dry_run=false` (PW) semantics.
    let execute = args["execute"].as_bool().unwrap_or(false)
        || !args["dry_run"].as_bool().unwrap_or(true);

    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => return json!({"error": format!("Invalid regex: {}", e)}),
    };

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return json!({"error": format!("Can't read dir: {}", e)}),
    };

    let mut renames: Vec<Value> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_file() && re.is_match(&name) {
            let new_name = re.replace_all(&name, replacement).to_string();
            if new_name != name {
                let old_path = entry.path();
                let new_path = old_path.parent().unwrap().join(&new_name);
                if execute {
                    if let Err(e) = std::fs::rename(&old_path, &new_path) {
                        errors.push(format!("{} -> {}: {}", name, new_name, e));
                    } else {
                        renames.push(json!({"from": name, "to": new_name, "done": true}));
                    }
                } else {
                    renames.push(json!({"from": name, "to": new_name, "preview": true}));
                }
            }
        }
    }

    json!({
        "renames": renames,
        "count": renames.len(),
        "executed": execute,
        "errors": errors
    })
}

fn sync_dir(args: &Value) -> Value {
    let source = args["source"].as_str().unwrap_or("");
    let destination = args["destination"].as_str().unwrap_or("");
    let mode = args["mode"].as_str().unwrap_or("update");
    let dry_run = args["dry_run"].as_bool().unwrap_or(true);
    let exclude: HashSet<String> = args["exclude"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if source.is_empty() || destination.is_empty() {
        return json!({"success": false, "error": "Source and destination required"});
    }

    if let Err(e) = std::fs::create_dir_all(destination) {
        return json!({"success": false, "error": format!("Can't create destination: {}", e)});
    }

    let mut copied: Vec<String> = Vec::new();
    let mut deleted: Vec<String> = Vec::new();
    let mut skipped = 0u64;

    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(source).unwrap_or(src_path);
        let rel_str = rel_path.to_string_lossy().to_string();

        if !rel_str.is_empty() && exclude.iter().any(|ex| rel_str.contains(ex)) {
            skipped += 1;
            continue;
        }

        let dst_path = Path::new(destination).join(rel_path);

        if src_path.is_dir() {
            if !dry_run {
                let _ = std::fs::create_dir_all(&dst_path);
            }
        } else if src_path.is_file() {
            let should_copy = match mode {
                "mirror" | "backup" => true,
                "update" => {
                    if !dst_path.exists() {
                        true
                    } else {
                        let src_m = std::fs::metadata(src_path).and_then(|m| m.modified());
                        let dst_m = std::fs::metadata(&dst_path).and_then(|m| m.modified());
                        match (src_m, dst_m) {
                            (Ok(s), Ok(d)) => s > d,
                            _ => true,
                        }
                    }
                }
                _ => true,
            };

            if should_copy {
                copied.push(rel_str.clone());
                if !dry_run {
                    if let Some(parent) = dst_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::copy(src_path, &dst_path);
                }
            }
        }
    }

    if mode == "mirror" {
        for entry in WalkDir::new(destination).into_iter().filter_map(|e| e.ok()) {
            let dst_path = entry.path();
            let rel_path = dst_path.strip_prefix(destination).unwrap_or(dst_path);
            let src_path = Path::new(source).join(rel_path);
            if !src_path.exists() && dst_path.is_file() {
                deleted.push(rel_path.to_string_lossy().to_string());
                if !dry_run {
                    let _ = std::fs::remove_file(dst_path);
                }
            }
        }
    }

    json!({
        "success": true,
        "source": source,
        "destination": destination,
        "mode": mode,
        "dry_run": dry_run,
        "files_copied": copied.len(),
        "files_deleted": deleted.len(),
        "files_skipped": skipped,
        "copied": copied,
        "deleted": deleted
    })
}

fn transform_file(args: &Value) -> Value {
    let directory = args["directory"].as_str().unwrap_or(".");
    let pattern = args["pattern"].as_str().unwrap_or("*");
    let transform_code = args["transform_code"].as_str().unwrap_or("");
    let dry_run = args["dry_run"].as_bool().unwrap_or(true);

    let regex_pattern = pattern
        .replace('.', "\\.")
        .replace('*', ".*")
        .replace('?', ".");
    let regex = match regex::Regex::new(&regex_pattern) {
        Ok(r) => r,
        Err(e) => return json!({"error": format!("Invalid pattern: {}", e)}),
    };

    let mut transformed: Vec<Value> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();

    for entry in WalkDir::new(directory).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !regex.is_match(filename) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(json!({"file": path.to_string_lossy(), "error": e.to_string()}));
                continue;
            }
        };

        let escaped_content = content
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");

        let py_code = format!(
            "content = \"{}\"\nresult = {}\nprint(result)\n",
            escaped_content, transform_code
        );

        let output = std::process::Command::new("python")
            .args(["-c", &py_code])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let new_content = String::from_utf8_lossy(&out.stdout).to_string();
                transformed.push(json!({
                    "file": path.to_string_lossy(),
                    "original_size": content.len(),
                    "new_size": new_content.len()
                }));
                if !dry_run {
                    let _ = std::fs::write(path, new_content.trim());
                }
            }
            Ok(out) => {
                errors.push(json!({
                    "file": path.to_string_lossy(),
                    "error": String::from_utf8_lossy(&out.stderr).to_string()
                }));
            }
            Err(e) => {
                errors.push(json!({"file": path.to_string_lossy(), "error": e.to_string()}));
            }
        }
    }

    json!({
        "success": true,
        "directory": directory,
        "pattern": pattern,
        "dry_run": dry_run,
        "transformed": transformed.len(),
        "errors": errors.len(),
        "files": transformed,
        "error_details": errors
    })
}

fn scaffold(args: &Value) -> Value {
    let template = match args["template"].as_str() {
        Some(s) => s,
        None => return json!({"error": "template required"}),
    };
    let name = match args["name"].as_str() {
        Some(s) => s,
        None => return json!({"error": "name required"}),
    };
    let output = args["output_dir"].as_str().unwrap_or(".");

    let base_path = PathBuf::from(output).join(name);
    if let Err(e) = std::fs::create_dir_all(&base_path) {
        return json!({"error": format!("Can't create dir: {}", e)});
    }

    let files_created: Vec<String> = match template {
        "rust-mcp" => scaffold_rust_mcp(&base_path, name),
        "python-mcp" => scaffold_python_mcp(&base_path, name),
        "nextjs" => scaffold_nextjs(&base_path, name),
        "fastapi" => scaffold_fastapi(&base_path, name),
        _ => {
            return json!({"error": format!("Unknown template: {}. Use: rust-mcp, python-mcp, nextjs, fastapi", template)})
        }
    };

    json!({
        "template": template,
        "name": name,
        "path": base_path.to_string_lossy(),
        "files_created": files_created
    })
}

fn write_scaffold(path: &Path, content: &str, files: &mut Vec<String>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::write(path, content).is_ok() {
        files.push(path.to_string_lossy().to_string());
    }
}

fn scaffold_rust_mcp(base: &Path, name: &str) -> Vec<String> {
    let mut files = Vec::new();
    let cargo = format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\ntokio = {{ version = \"1\", features = [\"full\"] }}\nserde = {{ version = \"1\", features = [\"derive\"] }}\nserde_json = \"1\"\nchrono = {{ version = \"0.4\", features = [\"serde\"] }}\nanyhow = \"1\"\n",
        name
    );
    write_scaffold(&base.join("Cargo.toml"), &cargo, &mut files);

    let main = r#"use std::io::{self, BufRead, Write};
use serde_json::{json, Value};

mod tools;

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        if let Ok(request) = serde_json::from_str::<Value>(&line) {
            let response = handle_request(&request);
            let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
            let _ = stdout.flush();
        }
    }
}

fn handle_request(request: &Value) -> Value {
    match request["method"].as_str().unwrap_or("") {
        "initialize" => json!({"protocolVersion": "2024-11-05", "capabilities": {"tools": {}}, "serverInfo": {"name": env!("CARGO_PKG_NAME"), "version": "0.1.0"}}),
        "tools/list" => json!({"tools": tools::get_definitions()}),
        "tools/call" => {
            let name = request["params"]["name"].as_str().unwrap_or("");
            let args = &request["params"]["arguments"];
            json!({"content": [{"type": "text", "text": serde_json::to_string(&tools::execute(name, args)).unwrap()}]})
        }
        _ => json!({"error": "unknown method"})
    }
}
"#;
    write_scaffold(&base.join("src/main.rs"), main, &mut files);

    let tools_mod = r#"use serde_json::{json, Value};

pub fn get_definitions() -> Vec<Value> {
    vec![
        json!({"name": "hello", "description": "Say hello", "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}}),
    ]
}

pub fn execute(name: &str, args: &Value) -> Value {
    match name {
        "hello" => json!({"message": format!("Hello, {}!", args["name"].as_str().unwrap_or("World"))}),
        _ => json!({"error": format!("Unknown tool: {}", name)})
    }
}
"#;
    write_scaffold(&base.join("src/tools/mod.rs"), tools_mod, &mut files);
    files
}

fn scaffold_python_mcp(base: &Path, name: &str) -> Vec<String> {
    let mut files = Vec::new();
    let server = format!(
        "#!/usr/bin/env python3\n\"\"\"MCP Server: {}\"\"\"\nimport asyncio\nfrom mcp.server import Server\nfrom mcp.server.stdio import stdio_server\n\nserver = Server(\"{}\")\n",
        name, name
    );
    write_scaffold(&base.join("server.py"), &server, &mut files);
    write_scaffold(&base.join("requirements.txt"), "mcp>=1.0.0\n", &mut files);
    files
}

fn scaffold_nextjs(base: &Path, name: &str) -> Vec<String> {
    let mut files = Vec::new();
    let package = format!(
        "{{\n  \"name\": \"{}\",\n  \"version\": \"0.1.0\",\n  \"scripts\": {{ \"dev\": \"next dev\", \"build\": \"next build\", \"start\": \"next start\" }},\n  \"dependencies\": {{ \"next\": \"^14.0.0\", \"react\": \"^18.2.0\", \"react-dom\": \"^18.2.0\" }}\n}}\n",
        name
    );
    write_scaffold(&base.join("package.json"), &package, &mut files);
    write_scaffold(
        &base.join("app/page.tsx"),
        "export default function Home() {\n  return <main><h1>Hello World</h1></main>\n}\n",
        &mut files,
    );
    files
}

fn scaffold_fastapi(base: &Path, name: &str) -> Vec<String> {
    let mut files = Vec::new();
    let main = format!(
        "from fastapi import FastAPI\n\napp = FastAPI(title=\"{}\")\n\n@app.get(\"/\")\ndef root():\n    return {{\"message\": \"Hello World\"}}\n",
        name
    );
    write_scaffold(&base.join("main.py"), &main, &mut files);
    write_scaffold(
        &base.join("requirements.txt"),
        "fastapi>=0.100.0\nuvicorn>=0.23.0\n",
        &mut files,
    );
    files
}
