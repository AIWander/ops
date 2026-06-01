//! File operations — ported/adapted from Programmer-Wander file.rs + search.rs
//! (and local raw.rs/transforms.rs). Sync implementations, bare tool names
//! distinct from ops's existing read_file/write_file and transform_* tools.
//! stdio MCP: never prints to stdout.

use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn get_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "copy_file",
            "description": "Copy file (creates parent dirs).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "destination": { "type": "string" }
                },
                "required": ["source", "destination"]
            }
        }),
        json!({
            "name": "move_file",
            "description": "Move or rename file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "destination": { "type": "string" }
                },
                "required": ["source", "destination"]
            }
        }),
        json!({
            "name": "create_dir",
            "description": "Create directory recursively.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "get_file_info",
            "description": "Get file metadata: size, dates, permissions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "edit_block",
            "description": "Replace text in file with string replacement.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Path to file" },
                    "old_string": { "type": "string", "description": "Text to find" },
                    "new_string": { "type": "string", "description": "Replacement text" },
                    "expected_replacements": { "type": "integer", "description": "Expected count", "default": 1 }
                },
                "required": ["file_path", "old_string", "new_string"]
            }
        }),
        json!({
            "name": "grep",
            "description": "Search files for pattern, return matching lines with context.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File or directory" },
                    "pattern": { "type": "string", "description": "Search pattern (regex)" },
                    "context": { "type": "integer", "description": "Lines of context (default: 0)" },
                    "recursive": { "type": "boolean", "description": "Search subdirs (default: false)" }
                },
                "required": ["path", "pattern"]
            }
        }),
        json!({
            "name": "diff_file",
            "description": "Create unified diff between two files.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path1": { "type": "string" },
                    "path2": { "type": "string" },
                    "context_lines": { "type": "integer", "default": 3 }
                },
                "required": ["path1", "path2"]
            }
        }),
        json!({
            "name": "file_stats",
            "description": "Get file/directory stats without reading content.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to analyze" },
                    "recursive": { "type": "boolean", "description": "Include subdirs (default: false)" }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "extract_lines",
            "description": "Extract specific line range from file. Saves reading entire file.",
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
            "name": "search_start",
            "description": "Search for files by name or content (recursive). Returns matches.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "pattern": { "type": "string" },
                    "search_type": { "type": "string", "default": "files", "description": "files or content" },
                    "file_pattern": { "type": "string", "description": "Filter like '*.rs|*.toml'" },
                    "ignore_case": { "type": "boolean", "default": true },
                    "max_results": { "type": "integer", "default": 100 }
                },
                "required": ["path", "pattern"]
            }
        }),
    ]
}

pub fn execute(name: &str, args: &Value) -> Value {
    match name {
        "copy_file" => copy_file(args),
        "move_file" => move_file(args),
        "create_dir" => create_dir(args),
        "get_file_info" => get_file_info(args),
        "edit_block" => edit_block(args),
        "grep" => grep(args),
        "diff_file" => diff_file(args),
        "file_stats" => file_stats(args),
        "extract_lines" => extract_lines(args),
        "search_start" => search_start(args),
        _ => json!({"error": format!("Unknown file tool: {}", name)}),
    }
}

fn copy_file(args: &Value) -> Value {
    let src = args.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let dst = args
        .get("destination")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if src.is_empty() || dst.is_empty() {
        return json!({"error": "source and destination are required"});
    }
    if let Some(parent) = Path::new(dst).parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::copy(src, dst) {
        Ok(bytes) => json!({"success": true, "source": src, "destination": dst, "bytes": bytes}),
        Err(e) => json!({"error": format!("Copy failed: {}", e)}),
    }
}

fn move_file(args: &Value) -> Value {
    let src = args.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let dst = args
        .get("destination")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if src.is_empty() || dst.is_empty() {
        return json!({"error": "source and destination are required"});
    }
    if let Some(parent) = Path::new(dst).parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::rename(src, dst) {
        Ok(_) => json!({"success": true, "source": src, "destination": dst}),
        Err(e) => json!({"error": format!("Move failed: {}", e)}),
    }
}

fn create_dir(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "path is required"});
    }
    match fs::create_dir_all(path) {
        Ok(_) => json!({"success": true, "path": path}),
        Err(e) => json!({"error": format!("Create dir failed: {}", e)}),
    }
}

fn get_file_info(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "path is required"});
    }
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return json!({"error": format!("Can't stat: {}", e)}),
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    json!({
        "success": true,
        "path": path,
        "size": meta.len(),
        "is_file": meta.is_file(),
        "is_dir": meta.is_dir(),
        "modified_unix": modified,
        "readonly": meta.permissions().readonly()
    })
}

fn edit_block(args: &Value) -> Value {
    let path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let old_str = args
        .get("old_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new_str = args
        .get("new_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let expected = args
        .get("expected_replacements")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);

    if path.is_empty() || old_str.is_empty() {
        return json!({"error": "file_path and old_string are required"});
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Can't read: {}", e)}),
    };
    let count = content.matches(old_str).count();

    if count == 0 {
        let lines: Vec<&str> = content.lines().collect();
        let mut close_matches = Vec::new();
        let probe = &old_str[..old_str.len().min(20).max(old_str.len().min(5))];
        for (i, line) in lines.iter().enumerate() {
            if line.contains(probe) {
                close_matches.push(json!({
                    "line": i + 1,
                    "content": line.chars().take(100).collect::<String>()
                }));
                if close_matches.len() >= 3 {
                    break;
                }
            }
        }
        return json!({
            "success": false,
            "error": "String not found in file",
            "close_matches": close_matches
        });
    }

    if expected > 0 && count != expected as usize {
        return json!({
            "success": false,
            "error": format!("Expected {} replacements, found {}", expected, count)
        });
    }

    let new_content = content.replace(old_str, new_str);
    match fs::write(path, &new_content) {
        Ok(_) => json!({
            "success": true,
            "path": path,
            "replacements": count,
            "size": new_content.len()
        }),
        Err(e) => json!({"error": format!("Can't write: {}", e)}),
    }
}

fn collect_files(p: &Path, recursive: bool) -> Result<Vec<PathBuf>, String> {
    if p.is_file() {
        Ok(vec![p.to_path_buf()])
    } else if p.is_dir() {
        if recursive {
            Ok(WalkDir::new(p)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .map(|e| e.path().to_path_buf())
                .collect())
        } else {
            match fs::read_dir(p) {
                Ok(rd) => Ok(rd
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_file())
                    .collect()),
                Err(e) => Err(format!("Can't read dir: {}", e)),
            }
        }
    } else {
        Err(format!("Path not found: {}", p.display()))
    }
}

fn grep(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    let context = args.get("context").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => return json!({"error": format!("Invalid regex: {}", e)}),
    };

    let files_to_search = match collect_files(Path::new(path), recursive) {
        Ok(f) => f,
        Err(e) => return json!({"error": e}),
    };

    let mut all_matches: Vec<Value> = Vec::new();
    for file_path in files_to_search {
        let content = match fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let start_ctx = i.saturating_sub(context);
                let end_ctx = (i + context + 1).min(lines.len());
                let context_lines: Vec<String> = lines[start_ctx..end_ctx]
                    .iter()
                    .enumerate()
                    .map(|(j, l)| format!("{}: {}", start_ctx + j + 1, l))
                    .collect();
                all_matches.push(json!({
                    "file": file_path.to_string_lossy(),
                    "line": i + 1,
                    "match": line,
                    "context": context_lines
                }));
            }
        }
    }

    json!({"path": path, "pattern": pattern, "matches": all_matches, "count": all_matches.len()})
}

fn diff_file(args: &Value) -> Value {
    let path1 = args.get("path1").and_then(|v| v.as_str()).unwrap_or("");
    let path2 = args.get("path2").and_then(|v| v.as_str()).unwrap_or("");
    if path1.is_empty() || path2.is_empty() {
        return json!({"error": "path1 and path2 are required"});
    }

    let content1 = match fs::read_to_string(path1) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Can't read {}: {}", path1, e)}),
    };
    let content2 = match fs::read_to_string(path2) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Can't read {}: {}", path2, e)}),
    };

    let lines1: Vec<&str> = content1.lines().collect();
    let lines2: Vec<&str> = content2.lines().collect();

    let mut diff_output = Vec::new();
    diff_output.push(format!("--- {}", path1));
    diff_output.push(format!("+++ {}", path2));

    let max_len = std::cmp::max(lines1.len(), lines2.len());
    let mut changes = 0;
    for i in 0..max_len {
        match (lines1.get(i), lines2.get(i)) {
            (Some(a), Some(b)) if a != b => {
                diff_output.push(format!("{}:- {}", i + 1, a));
                diff_output.push(format!("{}:+ {}", i + 1, b));
                changes += 1;
            }
            (Some(a), None) => {
                diff_output.push(format!("{}:- {}", i + 1, a));
                changes += 1;
            }
            (None, Some(b)) => {
                diff_output.push(format!("{}:+ {}", i + 1, b));
                changes += 1;
            }
            _ => {}
        }
    }

    json!({
        "success": true,
        "path1": path1,
        "path2": path2,
        "lines_in_file1": lines1.len(),
        "lines_in_file2": lines2.len(),
        "changes": changes,
        "diff": diff_output.join("\n")
    })
}

fn file_stats(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return json!({"error": format!("Can't stat: {}", e)}),
    };

    if meta.is_file() {
        json!({
            "type": "file",
            "path": path,
            "size": meta.len(),
            "size_human": format_size(meta.len())
        })
    } else {
        let mut total_size: u64 = 0;
        let mut file_count: u64 = 0;
        let mut dir_count: u64 = 0;

        let walker = if recursive {
            WalkDir::new(path).into_iter()
        } else {
            WalkDir::new(path).max_depth(1).into_iter()
        };

        for entry in walker.filter_map(|e| e.ok()) {
            if let Ok(m) = entry.metadata() {
                if m.is_file() {
                    total_size += m.len();
                    file_count += 1;
                } else if m.is_dir() && entry.depth() > 0 {
                    dir_count += 1;
                }
            }
        }

        json!({
            "type": "directory",
            "path": path,
            "files": file_count,
            "directories": dir_count,
            "total_size": total_size,
            "total_size_human": format_size(total_size),
            "recursive": recursive
        })
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

fn extract_lines(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let start = args.get("start").and_then(|v| v.as_i64()).unwrap_or(1) as usize;
    let end = args.get("end").and_then(|v| v.as_i64()).unwrap_or(-1);

    let file = match fs::File::open(path) {
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

    json!({
        "path": path,
        "start": start,
        "end": if end < 0 { "EOF".to_string() } else { end.to_string() },
        "lines": lines,
        "count": lines.len()
    })
}

fn search_start(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    let search_type = args
        .get("search_type")
        .and_then(|v| v.as_str())
        .unwrap_or("files");
    let file_pattern = args.get("file_pattern").and_then(|v| v.as_str());
    let ignore_case = args
        .get("ignore_case")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;

    if pattern.is_empty() {
        return json!({"error": "pattern is required"});
    }

    let regex = regex::RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
        .ok();
    let pattern_lower = pattern.to_lowercase();
    let search_content = search_type == "content";

    let mut results: Vec<Value> = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "__pycache__")
        })
        .filter_map(|e| e.ok())
    {
        if results.len() >= max_results {
            break;
        }
        if !entry.path().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();

        // file_pattern filter
        if let Some(fp) = file_pattern {
            let matches = fp.split('|').any(|p| {
                let p = p.trim();
                if let Some(suffix) = p.strip_prefix('*') {
                    name.ends_with(suffix)
                } else {
                    name.contains(p)
                }
            });
            if !matches {
                continue;
            }
        }

        if search_content {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                let mut matches: Vec<Value> = Vec::new();
                for (i, line) in content.lines().enumerate() {
                    let is_match = match &regex {
                        Some(re) => re.is_match(line),
                        None => line.to_lowercase().contains(&pattern_lower),
                    };
                    if is_match {
                        matches.push(json!({"line_number": i + 1, "line": line}));
                        if matches.len() >= 10 {
                            break;
                        }
                    }
                }
                if !matches.is_empty() {
                    results.push(json!({
                        "path": entry.path().to_string_lossy(),
                        "name": name,
                        "type": "content_match",
                        "matches": matches
                    }));
                }
            }
        } else {
            let name_lower = name.to_lowercase();
            let is_match = match &regex {
                Some(re) => re.is_match(&name_lower),
                None => name_lower.contains(&pattern_lower),
            };
            if is_match {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                results.push(json!({
                    "path": entry.path().to_string_lossy(),
                    "name": name,
                    "type": "file_match",
                    "size": size
                }));
            }
        }
    }

    let truncated = results.len() >= max_results;
    json!({
        "success": true,
        "pattern": pattern,
        "search_type": search_type,
        "results": results,
        "count": results.len(),
        "truncated": truncated
    })
}
