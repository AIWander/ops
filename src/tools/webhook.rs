//! Webhook Server — ported from Programmer-Wander crate webhook.rs
//! HTTP endpoints for external triggers, built on tokio's TcpListener.
//! Async functions are driven from the shared runtime in tools::mod.
//! stdio MCP: never prints to stdout.

use anyhow::Result;
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

// Active webhook servers
static SERVERS: Lazy<Mutex<HashMap<String, WebhookServer>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

struct WebhookServer {
    port: u16,
    routes: HashMap<String, String>,
    request_count: u64,
    running: bool,
}

fn generate_id() -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("wh_{:x}", time & 0xFFFFFF)
}

/// Tool definitions for MCP (schemas mirror Programmer-Wander's registry).
pub fn get_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "webhook_start",
            "description": "Start webhook server for external triggers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "port": { "type": "integer", "default": 9000 },
                    "routes": { "type": "object" }
                },
                "required": ["port", "routes"]
            }
        }),
        json!({
            "name": "webhook_stop",
            "description": "Stop a webhook server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "server_id": { "type": "string" }
                },
                "required": ["server_id"]
            }
        }),
        json!({
            "name": "webhook_list",
            "description": "List all webhook servers.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "webhook_add_route",
            "description": "Add route to existing webhook server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "server_id": { "type": "string" },
                    "path": { "type": "string" },
                    "action": { "type": "string" }
                },
                "required": ["server_id", "path", "action"]
            }
        }),
    ]
}

/// Start a webhook server for external triggers
pub async fn start_webhook_server(args: Value) -> Result<Value> {
    let port = args["port"].as_u64().unwrap_or(9000) as u16;
    let routes_val = args["routes"].as_object();

    let mut routes = HashMap::new();
    if let Some(r) = routes_val {
        for (path, action) in r {
            routes.insert(path.clone(), action.as_str().unwrap_or("").to_string());
        }
    }

    // Check if port is available
    let addr = format!("127.0.0.1:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            return Ok(json!({
                "success": false,
                "error": format!("Cannot bind to port {}: {}", port, e)
            }));
        }
    };

    let server_id = generate_id();

    let server = WebhookServer {
        port,
        routes: routes.clone(),
        request_count: 0,
        running: true,
    };

    {
        let mut servers = SERVERS.lock().unwrap();
        servers.insert(server_id.clone(), server);
    }

    // Spawn the server task
    let id_clone = server_id.clone();
    let routes_clone = routes.clone();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    let routes = routes_clone.clone();
                    let id = id_clone.clone();

                    tokio::spawn(async move {
                        let mut buf_reader = BufReader::new(&mut socket);
                        let mut request_line = String::new();

                        if buf_reader.read_line(&mut request_line).await.is_ok() {
                            // Parse request: GET /path HTTP/1.1
                            let parts: Vec<&str> = request_line.split_whitespace().collect();
                            if parts.len() >= 2 {
                                let method = parts[0];
                                let path = parts[1];

                                let action = routes
                                    .get(path)
                                    .cloned()
                                    .unwrap_or_else(|| "unknown".to_string());

                                // Update request count
                                if let Ok(mut servers) = SERVERS.lock() {
                                    if let Some(s) = servers.get_mut(&id) {
                                        s.request_count += 1;
                                    }
                                }

                                let response_body = json!({
                                    "received": true,
                                    "method": method,
                                    "path": path,
                                    "action": action,
                                    "timestamp": SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs()
                                })
                                .to_string();

                                let response = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                    response_body.len(),
                                    response_body
                                );

                                let _ = socket.write_all(response.as_bytes()).await;
                            }
                        }
                    });
                }
                Err(_) => break,
            }

            // Check if server should stop
            let running = SERVERS
                .lock()
                .map(|s| s.get(&id_clone).map(|srv| srv.running).unwrap_or(false))
                .unwrap_or(false);

            if !running {
                break;
            }
        }
    });

    let route_list: Vec<String> = routes.keys().cloned().collect();

    Ok(json!({
        "success": true,
        "server_id": server_id,
        "port": port,
        "url": format!("http://127.0.0.1:{}", port),
        "routes": route_list
    }))
}

/// Stop a webhook server
pub async fn stop_webhook_server(args: Value) -> Result<Value> {
    let server_id = args["server_id"].as_str().unwrap_or("");

    let server = {
        let mut servers = SERVERS.lock().unwrap();
        if let Some(s) = servers.get_mut(server_id) {
            s.running = false;
        }
        servers.remove(server_id)
    };

    match server {
        Some(s) => Ok(json!({
            "success": true,
            "server_id": server_id,
            "requests_served": s.request_count
        })),
        None => Ok(json!({
            "success": false,
            "error": format!("Server {} not found", server_id)
        })),
    }
}

/// List all webhook servers
pub async fn list_webhook_servers() -> Result<Value> {
    let servers = SERVERS.lock().unwrap();

    let list: Vec<Value> = servers
        .iter()
        .map(|(id, s)| {
            json!({
                "server_id": id,
                "port": s.port,
                "url": format!("http://127.0.0.1:{}", s.port),
                "routes": s.routes.keys().collect::<Vec<_>>(),
                "request_count": s.request_count,
                "running": s.running
            })
        })
        .collect();

    Ok(json!({
        "success": true,
        "servers": list
    }))
}

/// Add route to existing webhook server
pub async fn add_webhook_route(args: Value) -> Result<Value> {
    let server_id = args["server_id"].as_str().unwrap_or("");
    let path = args["path"].as_str().unwrap_or("/");
    let action = args["action"].as_str().unwrap_or("");

    let mut servers = SERVERS.lock().unwrap();

    match servers.get_mut(server_id) {
        Some(s) => {
            s.routes.insert(path.to_string(), action.to_string());
            Ok(json!({
                "success": true,
                "server_id": server_id,
                "added_route": path,
                "action": action,
                "total_routes": s.routes.len()
            }))
        }
        None => Ok(json!({
            "success": false,
            "error": format!("Server {} not found", server_id)
        })),
    }
}
