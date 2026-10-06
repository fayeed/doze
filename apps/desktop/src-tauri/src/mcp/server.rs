//! MCP uses stdio. The private authenticated loopback bridge only submits tool calls
//! to the running desktop engine; it has no power manager or OS action API.
use crate::state::Request;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
const LIMIT: u64 = 64 * 1024;
const PROTOCOL: &str = "2025-11-25";
#[derive(Deserialize, Serialize)]
struct Endpoint {
    address: SocketAddr,
    token: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    token: String,
    call: super::tools::Call,
}
fn read_line(reader: &mut impl BufRead) -> Result<String, String> {
    read_line_limited(reader, LIMIT)
}
fn read_line_limited(reader: &mut impl BufRead, limit: u64) -> Result<String, String> {
    let mut line = String::new();
    reader
        .take(limit)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if !line.ends_with('\n') {
        return Err("Disconnected or oversized MCP message.".into());
    }
    Ok(line)
}
fn write_line(writer: &mut impl Write, value: &Value) -> Result<(), String> {
    writeln!(writer, "{value}")
        .and_then(|_| writer.flush())
        .map_err(|e| e.to_string())
}
static ADDRESS: std::sync::OnceLock<SocketAddr> = std::sync::OnceLock::new();

/// The private bridge's loopback address, shown in Settings. Its port changes every launch
/// and every call needs the token from the endpoint file.
pub fn address() -> Option<SocketAddr> {
    ADDRESS.get().copied()
}

pub fn start(path: PathBuf, sender: Sender<Request>) -> Result<(), String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    let _ = ADDRESS.set(address);
    let endpoint = Endpoint {
        address,
        token: format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ),
    };
    std::fs::create_dir_all(path.parent().ok_or("Invalid endpoint path.")?)
        .map_err(|e| e.to_string())?;
    let temporary = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    file.write_all(&serde_json::to_vec(&endpoint).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(temporary, &path).map_err(|e| e.to_string())?;
    let active = Arc::new(AtomicUsize::new(0));
    std::thread::Builder::new()
        .name("doze-mcp-listener".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if active.load(Ordering::Relaxed) >= 16 {
                    continue;
                }
                let active = active.clone();
                let sender = sender.clone();
                let token = endpoint.token.clone();
                active.fetch_add(1, Ordering::Relaxed);
                let counter = active.clone();
                if std::thread::Builder::new()
                    .name("doze-mcp-request".into())
                    .spawn(move || {
                        let _ = serve(stream, &token, &sender);
                        active.fetch_sub(1, Ordering::Relaxed);
                    })
                    .is_err()
                {
                    counter.fetch_sub(1, Ordering::Relaxed);
                }
            }
            let _ = std::fs::remove_file(path);
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn serve(mut stream: TcpStream, token: &str, sender: &Sender<Request>) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let envelope: Envelope = serde_json::from_str(&read_line(&mut BufReader::new(&mut stream))?)
        .map_err(|e| e.to_string())?;
    if !super::auth::secure_equal(&envelope.token, token) {
        return Err("Invalid bridge token.".into());
    }
    let (reply, response) = mpsc::channel();
    sender
        .send(Request::Mcp(envelope.call, reply))
        .map_err(|_| "Doze stopped.")?;
    let result = response
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| "Doze did not respond.")?;
    write_line(&mut stream, &json!(result))
}
pub(crate) fn forward(path: &Path, call: super::tools::Call) -> Result<Value, String> {
    let endpoint: Endpoint = serde_json::from_slice(
        &std::fs::read(path).map_err(|_| "Open the Doze desktop app before connecting MCP.")?,
    )
    .map_err(|e| e.to_string())?;
    if !endpoint.address.ip().is_loopback() {
        return Err("Refusing non-local bridge endpoint.".into());
    }
    let mut stream = TcpStream::connect_timeout(&endpoint.address, Duration::from_secs(3))
        .map_err(|_| "Doze is unavailable. Reopen the app.")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    write_line(
        &mut stream,
        &json!(Envelope {
            token: endpoint.token,
            call
        }),
    )?;
    let result: Result<Value, String> = serde_json::from_str(&read_line_limited(
        &mut BufReader::new(stream),
        1024 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    result
}
pub fn bridge() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let index = args
        .iter()
        .position(|a| a == "--endpoint")
        .ok_or("--endpoint is required. Use connection settings from Doze.")?;
    let path = PathBuf::from(args.get(index + 1).ok_or("Endpoint path is missing.")?);
    let key = std::env::var("DOZE_MCP_KEY")
        .map_err(|_| "DOZE_MCP_KEY is missing. Copy connection settings from Doze.")?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut initialized = false;
    let mut ready = false;
    let sessions = Sessions::default();
    keep_alive(path.clone(), key.clone(), sessions.clone());
    while let Ok(line) = read_line(&mut input) {
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                write_line(&mut output, &rpc_error(Value::Null, -32700, "Parse error"))?;
                continue;
            }
        };
        if value["method"] == "notifications/initialized"
            && value["jsonrpc"] == "2.0"
            && value.get("id").is_none()
            && initialized
        {
            ready = true;
            continue;
        }
        if value.get("id").is_none() {
            // Notifications have no response; malformed messages still receive invalid-request.
            if value["jsonrpc"] == "2.0" && value["method"].is_string() {
                continue;
            }
        }
        let response = dispatch(&value, &mut initialized, ready, |name, arguments| {
            let result = forward(
                &path,
                super::tools::Call {
                    key: key.clone(),
                    name,
                    arguments,
                },
            );
            if let Ok(session) = &result {
                sessions.observe(session);
            }
            result
        });
        write_line(&mut output, &response)?;
    }
    // EOF is deliberately NOT finish_session. Keep-alive stops with this process, so the
    // core lease eventually becomes uncertain.
    Ok(())
}

/// Sessions this bridge's agent created or used, with when each is next renewed.
#[derive(Clone, Default)]
struct Sessions(Arc<Mutex<HashMap<String, (Duration, Instant)>>>);
impl Sessions {
    /// Track sessions that are still open, and forget ended ones.
    fn observe(&self, session: &Value) {
        let Some(id) = session["session_id"].as_str() else {
            return;
        };
        let Ok(mut sessions) = self.0.lock() else {
            return;
        };
        if matches!(
            session["status"].as_str(),
            Some("awaiting_authorization" | "active" | "connection_lost")
        ) {
            // Renew three times per lease, using the lease Doze just granted.
            let lease = session["lease_expires_at"]
                .as_u64()
                .zip(session["last_heartbeat"].as_u64())
                .map_or(300, |(expires, renewed)| expires.saturating_sub(renewed));
            let interval = std::env::var("DOZE_KEEPALIVE_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or((lease / 3).clamp(10, 600));
            let interval = Duration::from_secs(interval);
            sessions.insert(id.into(), (interval, Instant::now() + interval));
        } else {
            sessions.remove(id);
        }
    }
    fn due(&self) -> Vec<String> {
        let now = Instant::now();
        self.0.lock().map_or_else(
            |_| Vec::new(),
            |sessions| {
                sessions
                    .iter()
                    .filter(|(_, (_, due))| *due <= now)
                    .map(|(id, _)| id.clone())
                    .collect()
            },
        )
    }
    fn forget(&self, id: &str) {
        if let Ok(mut sessions) = self.0.lock() {
            sessions.remove(id);
        }
    }
    fn postpone(&self, id: &str) {
        if let Ok(mut sessions) = self.0.lock() {
            if let Some((interval, due)) = sessions.get_mut(id) {
                *due = Instant::now() + *interval;
            }
        }
    }
}

/// Renews this agent's sessions while its app keeps the stdio connection open, so a long
/// step without model turns keeps the lease. It never finishes a session; Doze enforces the
/// keep-alive setting and limit.
fn keep_alive(path: PathBuf, key: String, sessions: Sessions) {
    let _ = std::thread::Builder::new()
        .name("doze-mcp-keepalive".into())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            for id in sessions.due() {
                let result = forward(
                    &path,
                    super::tools::Call {
                        key: key.clone(),
                        name: "doze.bridge_keepalive".into(),
                        arguments: json!({ "session_id": id }),
                    },
                );
                match result {
                    Ok(session) => sessions.observe(&session),
                    // Doze may be restarting; retry on the next interval.
                    Err(error)
                        if error.contains("unavailable") || error.contains("Open the Doze") =>
                    {
                        sessions.postpone(&id)
                    }
                    // Ended, refused by settings, or past the keep-alive limit.
                    Err(_) => sessions.forget(&id),
                }
            }
        });
}
fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
pub fn dispatch(
    request: &Value,
    initialized: &mut bool,
    ready: bool,
    mut call: impl FnMut(String, Value) -> Result<Value, String>,
) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    if !request.is_object()
        || request["jsonrpc"] != "2.0"
        || !(id.is_string() || id.is_number() || id.is_null())
        || !request["method"].is_string()
    {
        return rpc_error(id, -32600, "Invalid Request");
    }
    let method = request["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" if !*initialized => {
            let params = &request["params"];
            if !params["protocolVersion"].is_string()
                || !params["clientInfo"]["name"].is_string()
                || !params["clientInfo"]["version"].is_string()
                || !params["capabilities"].is_object()
            {
                return rpc_error(id, -32602, "Invalid initialize parameters");
            }
            *initialized = true;
            let requested = params["protocolVersion"].as_str().unwrap_or_default();
            let version =
                if ["2024-11-05", "2025-03-26", "2025-06-18", PROTOCOL].contains(&requested) {
                    requested
                } else {
                    PROTOCOL
                };
            json!({"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"Doze","version":env!("CARGO_PKG_VERSION")},"instructions":"Sessions require local Doze authorization. Heartbeat at least twice per lease. Finish explicitly only after all work and tests complete. Disconnect is uncertainty, never completion."})
        }
        "ping" => json!({}),
        _ if !*initialized || !ready => {
            return rpc_error(
                id,
                -32600,
                "Initialize and send notifications/initialized first",
            )
        }
        "tools/list" => super::tools::definitions(),
        "tools/call" => {
            let params = &request["params"];
            let Some(name) = params["name"].as_str() else {
                return rpc_error(id, -32602, "Tool name is required");
            };
            if !super::tools::definitions()["tools"]
                .as_array()
                .is_some_and(|tools| tools.iter().any(|tool| tool["name"] == name))
            {
                return rpc_error(id, -32602, "Unknown tool");
            }
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            if !arguments.is_object() {
                return rpc_error(id, -32602, "Arguments must be an object");
            }
            match call(name.into(), arguments) {
                Ok(value) => {
                    json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent": {"result":value},"isError":false})
                }
                Err(error) => json!({"content":[{"type":"text","text":error}],"isError":true}),
            }
        }
        _ => return rpc_error(id, -32601, "Method not found"),
    };
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
pub fn connection_configs(
    settings: &crate::core::sessions::Settings,
    settings_path: &Path,
) -> Value {
    let executable = std::env::current_exe()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let endpoint = settings_path
        .with_file_name("mcp-endpoint.json")
        .to_string_lossy()
        .into_owned();
    json!(settings.agents.clients.iter().map(|client| {
        let command = json!({"type":"stdio","command":executable,"args":["--mcp","--endpoint",endpoint],"env":{"DOZE_MCP_KEY":client.secret}});
        // JSON quoted strings are valid TOML basic strings, including Windows backslashes.
        let codex = format!("[mcp_servers.doze]\ncommand = {}\nargs = [\"--mcp\", \"--endpoint\", {}]\n[mcp_servers.doze.env]\nDOZE_MCP_KEY = {}\n", json!(executable), json!(endpoint), json!(client.secret));
        json!({"clientId":client.id,"name":client.name,"generic":json!({"mcpServers":{"doze":command}}).to_string(),"claude":command.to_string(),"codex":codex})
    }).collect::<Vec<_>>())
}
