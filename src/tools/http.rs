//! HTTP and web scraping tools — ported from local crate http.rs
//! Lightweight HTTP requests and page content extraction.
//! stdio MCP: never prints to stdout.

use serde_json::{json, Value};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Tool definitions for MCP
pub fn get_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "http_request",
            "description": "Make HTTP request with full method support (GET/POST/PUT/DELETE/PATCH/HEAD). Returns status, headers, body, timing.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to request" },
                    "method": { "type": "string", "default": "GET", "description": "HTTP method (GET/POST/PUT/DELETE/PATCH/HEAD)" },
                    "headers": { "type": "object", "description": "Custom headers as key-value pairs" },
                    "body": { "type": "string", "description": "Request body (for POST/PUT/PATCH)" },
                    "timeout_secs": { "type": "integer", "default": 30, "description": "Timeout in seconds" }
                },
                "required": ["url"]
            }
        }),
        json!({
            "name": "http_fetch",
            "description": "Fetch URL content as text/html",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "method": { "type": "string", "default": "GET", "description": "HTTP method" },
                    "headers": {
                        "type": "object",
                        "description": "Custom headers as key-value pairs"
                    }
                },
                "required": ["url"]
            }
        }),
        json!({
            "name": "http_scrape",
            "description": "Scrape URL and extract text content (strips HTML)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to scrape" },
                    "selector": { "type": "string", "description": "CSS selector (optional)" }
                },
                "required": ["url"]
            }
        }),
        json!({
            "name": "http_download",
            "description": "Download file from URL",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to download" },
                    "path": { "type": "string", "description": "Local save path" }
                },
                "required": ["url", "path"]
            }
        }),
    ]
}

/// Execute HTTP tool
pub fn execute(name: &str, args: &Value) -> Value {
    match name {
        "http_request" => http_request(args),
        "http_fetch" => http_fetch(args),
        "http_scrape" => http_scrape(args),
        "http_download" => http_download(args),
        _ => json!({"error": format!("Unknown http tool: {}", name)}),
    }
}

fn http_request(args: &Value) -> Value {
    let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let method = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
    let headers: std::collections::HashMap<String, String> = args
        .get("headers")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let body = args.get("body").and_then(|v| v.as_str());
    let timeout_secs = args
        .get("timeout_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    if url.is_empty() {
        return json!({"error": "url is required"});
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
    {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Failed to build client: {}", e)}),
    };

    let start = std::time::Instant::now();

    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        "HEAD" => client.head(url),
        _ => return json!({"error": format!("Unsupported method: {}", method)}),
    };

    for (key, value) in &headers {
        request = request.header(key.as_str(), value.as_str());
    }

    if let Some(b) = body {
        request = request.body(b.to_string());
    }

    match request.send() {
        Ok(response) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let status = response.status().as_u16();
            let response_headers: std::collections::HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let body_text = response.text().unwrap_or_default();
            let body_len = body_text.len();
            // Body cap: 500KB
            const BODY_CAP: usize = 500_000;
            json!({
                "success": (200..300).contains(&status),
                "status_code": status,
                "headers": response_headers,
                "body": if body_len > BODY_CAP { &body_text[..BODY_CAP] } else { &body_text },
                "body_length": body_len,
                "truncated": body_len > BODY_CAP,
                "response_time_ms": elapsed
            })
        }
        Err(e) => json!({"error": format!("Request failed: {}", e)}),
    }
}

fn http_fetch(args: &Value) -> Value {
    let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let method = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET");

    // Build headers string
    let headers_str = match args.get("headers").and_then(|v| v.as_object()) {
        Some(headers) => headers
            .iter()
            .map(|(k, v)| format!("-H '{}: {}'", k, v.as_str().unwrap_or("")))
            .collect::<Vec<_>>()
            .join(" "),
        None => String::new(),
    };

    // Use Invoke-WebRequest for HTTP (available on Windows 10+)
    let ps_cmd = format!(
        r#"
        try {{
            $response = Invoke-WebRequest -Uri '{}' -Method {} -UseBasicParsing {}
            $response.Content
        }} catch {{
            "[ERROR] $_"
        }}
        "#,
        url,
        method,
        if headers_str.is_empty() {
            ""
        } else {
            "-Headers @{}"
        }
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &ps_cmd]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Truncate if too large (cap 500KB)
            const FETCH_CAP: usize = 500_000;
            let content = if stdout.len() > FETCH_CAP {
                format!("{}\n...[truncated to 500KB]", &stdout[..FETCH_CAP])
            } else {
                stdout.to_string()
            };
            json!(content.trim())
        }
        Err(e) => json!(format!("[ERROR] {}", e)),
    }
}

fn http_scrape(args: &Value) -> Value {
    let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let selector = args.get("selector").and_then(|v| v.as_str());

    // PowerShell script to fetch and extract text
    let ps_cmd = if let Some(sel) = selector {
        format!(
            r#"
            try {{
                $response = Invoke-WebRequest -Uri '{}' -UseBasicParsing
                $html = $response.Content
                $pattern = '<[^>]*class=[^>]*{}[^>]*>(.*?)</[^>]*>'
                $matches = [regex]::Matches($html, $pattern, 'IgnoreCase')
                $matches | ForEach-Object {{ $_.Groups[1].Value -replace '<[^>]+>', '' }}
            }} catch {{
                "[ERROR] $_"
            }}
            "#,
            url, sel
        )
    } else {
        format!(
            r#"
            try {{
                $response = Invoke-WebRequest -Uri '{}' -UseBasicParsing
                $text = $response.Content -replace '<script[^>]*>.*?</script>', '' -replace '<style[^>]*>.*?</style>', ''
                $text = $text -replace '<[^>]+>', ' '
                $text = $text -replace '\s+', ' '
                $text.Trim()
            }} catch {{
                "[ERROR] $_"
            }}
            "#,
            url
        )
    };

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &ps_cmd]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let content = if stdout.len() > 30000 {
                format!("{}\n...[truncated to 30KB]", &stdout[..30000])
            } else {
                stdout.to_string()
            };
            json!(content.trim())
        }
        Err(e) => json!(format!("[ERROR] {}", e)),
    }
}

fn http_download(args: &Value) -> Value {
    let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");

    let ps_cmd = format!(
        r#"
        try {{
            Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing
            $file = Get-Item '{}'
            "downloaded: $($file.Length) bytes to {}"
        }} catch {{
            "[ERROR] $_"
        }}
        "#,
        url, path, path, path
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &ps_cmd]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            json!(stdout.trim())
        }
        Err(e) => json!(format!("[ERROR] {}", e)),
    }
}
