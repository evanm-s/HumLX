//! Remote control: the Studio PC serves its own screens to browsers on the
//! local network, so a laptop can run the same show live.
//!
//! A remote browser loads the built screens over HTTP and sends every command
//! over a WebSocket. Each command is handed to the app's own invoke handler,
//! exactly as if it came from the Studio PC's window, so remotes and the
//! console always behave the same. Only the Studio PC sends Art-Net.
//!
//! Devices pair once with a 6-digit code shown on the Studio PC and then keep
//! a key. After any change, every other screen is told which part of the show
//! changed so it can reload just that part.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State as Extract};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{broadcast, oneshot, watch};

use crate::App;

/// Port the Studio PC listens on unless changed in `remote.json`.
pub const DEFAULT_PORT: u16 = 8130;
/// How long a pairing code works for.
const CODE_LIFETIME: Duration = Duration::from_secs(120);
/// Wrong guesses before a pairing code stops working.
const CODE_ATTEMPTS: u32 = 5;
/// Header that marks an invoke as coming from a remote, with its connection.
const REMOTE_HEADER: &str = "x-humlx-remote";
/// WebSocket close code that tells a browser its key isn't (or is no longer) paired.
const CLOSE_UNPAIRED: u16 = 4001;

/// Commands that belong to the Studio PC itself and are refused from remotes.
const CONSOLE_ONLY: &[&str] = &[
    "set_output",
    "discover_nodes",
    "set_osc",
    "save_show_as",
    "open_show",
    "new_show",
    "set_surface_leds",
    "remote_status",
    "set_remote",
    "remote_pairing_code",
    "forget_remote_device",
];

pub fn console_only(cmd: &str) -> bool {
    CONSOLE_ONLY.contains(&cmd) || cmd.starts_with("plugin:")
}

/// Which parts of the show a command can change, so other screens reload them.
/// Commands not listed only read, or change things the live levels already carry.
pub fn changes(cmd: &str) -> &'static [&'static str] {
    match cmd {
        "add_fixtures" | "update_fixture" | "update_fixtures" | "remove_fixtures" | "import_profile" => {
            &["patch", "programmer", "playback", "stage"]
        }
        "select"
        | "select_group"
        | "set_intensity"
        | "set_colour"
        | "set_position"
        | "set_attribute"
        | "clear_programmer"
        | "remove_from_programmer"
        | "load_live"
        | "save_group"
        | "delete_group"
        | "record_preset"
        | "apply_preset"
        | "delete_preset"
        | "point_at" => &["programmer"],
        "set_stage_plan" | "calibrate_mark" | "forget_calibration" | "set_fixture_position" | "set_placement" => {
            &["stage"]
        }
        "record_cue" | "update_cue" => &["playback", "programmer"],
        "new_cue_list"
        | "rename_cue_list"
        | "delete_cue_list"
        | "duplicate_cue_list"
        | "select_cue_list"
        | "set_slot"
        | "set_grid_button"
        | "swap_grid_buttons"
        | "add_grid_page"
        | "rename_grid_page"
        | "delete_grid_page"
        | "edit_cue"
        | "delete_cue"
        | "set_cue_list_options"
        | "save_effect"
        | "delete_effect"
        | "set_effect_size"
        | "set_speed_bpm"
        | "tap_speed"
        | "set_master" => &["playback"],
        "set_highlight_colour" => &["settings"],
        "open_show" | "new_show" => &["show"],
        _ => &[],
    }
}

/// A change made from the Studio PC (`from: None`) or a remote connection.
#[derive(Clone, Debug)]
pub struct Change {
    pub from: Option<u64>,
    pub parts: &'static [&'static str],
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Settings {
    enabled: bool,
    port: u16,
    devices: Vec<Device>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            devices: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    id: u32,
    name: String,
    key: String,
    /// Seconds since 1970 when it paired.
    paired_at: u64,
}

struct Pairing {
    code: String,
    until: Instant,
    wrong: u32,
}

struct Server {
    /// Dropping this stops the server.
    _stop: oneshot::Sender<()>,
}

struct Inner {
    settings: Settings,
    server: Option<Server>,
    error: Option<String>,
    pairing: Option<Pairing>,
    /// Open connections: id and device id.
    connected: Vec<(u64, u32)>,
    next_connection: u64,
}

pub struct Remote {
    path: PathBuf,
    inner: Mutex<Inner>,
    changes: broadcast::Sender<Change>,
    /// Bumped when devices are removed or remote control is turned off, so
    /// open connections check they may stay.
    kicks: watch::Sender<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    id: u32,
    name: String,
    paired_at: u64,
    connected: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    enabled: bool,
    running: bool,
    port: u16,
    /// Addresses to type into a browser, such as `http://192.168.1.20:8130`.
    addresses: Vec<String>,
    error: Option<String>,
    /// The pairing code while one is showing, and its seconds left.
    code: Option<String>,
    code_seconds: u64,
    devices: Vec<DeviceView>,
}

impl Remote {
    pub fn load(path: PathBuf) -> Self {
        let settings = std::fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Self {
            path,
            inner: Mutex::new(Inner {
                settings,
                server: None,
                error: None,
                pairing: None,
                connected: Vec::new(),
                next_connection: 1,
            }),
            changes: broadcast::channel(256).0,
            kicks: watch::channel(0).0,
        }
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn save(&self, settings: &Settings) {
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    /// Starts serving if remote control was left on.
    pub fn resume(&self, handle: &AppHandle) {
        let enabled = self.inner().settings.enabled;
        if enabled {
            self.start(handle);
        }
    }

    fn start(&self, handle: &AppHandle) {
        let mut inner = self.inner();
        inner.server = None;
        let port = inner.settings.port;
        match serve(handle.clone(), port) {
            Ok(server) => {
                inner.server = Some(server);
                inner.error = None;
            }
            Err(error) => inner.error = Some(format!("Couldn't listen on port {port}: {error}")),
        }
    }

    fn stop(&self) {
        let mut inner = self.inner();
        inner.server = None;
        inner.pairing = None;
        inner.error = None;
        drop(inner);
        self.kicks.send_modify(|n| *n += 1);
    }

    fn enabled(&self) -> bool {
        self.inner().settings.enabled
    }

    pub fn set_enabled(&self, handle: &AppHandle, on: bool) {
        let settings = {
            let mut inner = self.inner();
            inner.settings.enabled = on;
            inner.settings.clone()
        };
        self.save(&settings);
        if on {
            self.start(handle);
        } else {
            self.stop();
        }
    }

    pub fn new_code(&self) {
        let code = format!("{:06}", random_u64() % 1_000_000);
        self.inner().pairing = Some(Pairing {
            code,
            until: Instant::now() + CODE_LIFETIME,
            wrong: 0,
        });
    }

    /// Checks a pairing code and, when it matches, pairs a new device and returns its key.
    fn pair(&self, code: &str, name: &str) -> Result<String, String> {
        let mut inner = self.inner();
        let now = Instant::now();
        let Some(pairing) = inner.pairing.as_mut().filter(|p| p.until > now) else {
            inner.pairing = None;
            return Err("No code is showing. Tap Pair a device in Settings on the Studio PC.".into());
        };
        if pairing.code != code.trim() {
            pairing.wrong += 1;
            if pairing.wrong >= CODE_ATTEMPTS {
                inner.pairing = None;
                return Err("Too many wrong codes. Make a new one on the Studio PC.".into());
            }
            return Err("That code doesn't match the one on the Studio PC.".into());
        }
        inner.pairing = None;
        let id = inner.settings.devices.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        let key = format!("{:016x}{:016x}", random_u64(), random_u64());
        let name = match name.trim() {
            "" => format!("Remote {id}"),
            name => name.chars().take(40).collect(),
        };
        let paired_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        inner.settings.devices.push(Device {
            id,
            name,
            key: key.clone(),
            paired_at,
        });
        let settings = inner.settings.clone();
        drop(inner);
        self.save(&settings);
        Ok(key)
    }

    pub fn forget(&self, id: u32) {
        let settings = {
            let mut inner = self.inner();
            inner.settings.devices.retain(|d| d.id != id);
            inner.settings.clone()
        };
        self.save(&settings);
        self.kicks.send_modify(|n| *n += 1);
    }

    /// The paired device with this key, while remote control is on.
    fn device(&self, key: &str) -> Option<u32> {
        let inner = self.inner();
        if !inner.settings.enabled || key.is_empty() {
            return None;
        }
        inner.settings.devices.iter().find(|d| d.key == key).map(|d| d.id)
    }

    fn connect(&self, device: u32) -> u64 {
        let mut inner = self.inner();
        let id = inner.next_connection;
        inner.next_connection += 1;
        inner.connected.push((id, device));
        id
    }

    fn disconnect(&self, connection: u64) {
        self.inner().connected.retain(|(id, _)| *id != connection);
    }

    /// Number of remotes connected now.
    pub fn connected(&self) -> usize {
        self.inner().connected.len()
    }

    /// Tells every other screen what a command changed.
    pub fn changed<R: tauri::Runtime>(&self, handle: &AppHandle<R>, from: Option<u64>, parts: &'static [&'static str]) {
        if parts.is_empty() {
            return;
        }
        // Nobody listening is fine.
        let _ = self.changes.send(Change { from, parts });
        if from.is_some() {
            let _ = handle.emit("remote-change", parts);
        }
    }

    pub fn status(&self) -> RemoteStatus {
        let inner = self.inner();
        let now = Instant::now();
        let pairing = inner.pairing.as_ref().filter(|p| p.until > now);
        let running = inner.server.is_some();
        let port = inner.settings.port;
        RemoteStatus {
            enabled: inner.settings.enabled,
            running,
            port,
            addresses: if running { addresses(port) } else { Vec::new() },
            error: inner.error.clone(),
            code: pairing.map(|p| p.code.clone()),
            code_seconds: pairing.map(|p| (p.until - now).as_secs()).unwrap_or(0),
            devices: inner
                .settings
                .devices
                .iter()
                .map(|d| DeviceView {
                    id: d.id,
                    name: d.name.clone(),
                    paired_at: d.paired_at,
                    connected: inner.connected.iter().any(|(_, device)| *device == d.id),
                })
                .collect(),
        }
    }
}

fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("the system's random numbers");
    u64::from_le_bytes(bytes)
}

/// This computer's local network addresses, as browser links.
fn addresses(port: u16) -> Vec<String> {
    let mut ips: Vec<Ipv4Addr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.ip() {
            std::net::IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() => Some(ip),
            _ => None,
        })
        .collect();
    ips.sort();
    ips.dedup();
    ips.into_iter().map(|ip| format!("http://{ip}:{port}")).collect()
}

// --- Commands for the Studio PC's Settings screen ------------------------------

#[tauri::command]
pub fn remote_status(app: State<'_, App>) -> RemoteStatus {
    app.remote.status()
}

#[tauri::command]
pub fn set_remote(app: State<'_, App>, handle: AppHandle, on: bool) -> RemoteStatus {
    app.remote.set_enabled(&handle, on);
    app.remote.status()
}

#[tauri::command]
pub fn remote_pairing_code(app: State<'_, App>) -> RemoteStatus {
    if app.remote.enabled() {
        app.remote.new_code();
    }
    app.remote.status()
}

#[tauri::command]
pub fn forget_remote_device(app: State<'_, App>, id: u32) -> RemoteStatus {
    app.remote.forget(id);
    app.remote.status()
}

// --- The server ----------------------------------------------------------------

fn serve(handle: AppHandle, port: u16) -> std::io::Result<Server> {
    let listener = std::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))?;
    listener.set_nonblocking(true)?;
    let (stop, stopped) = oneshot::channel::<()>();
    let router = Router::new()
        .route("/ws", get(socket))
        .route("/pair", post(pair))
        .fallback(get(asset))
        .with_state(handle);
    tauri::async_runtime::spawn(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            return;
        };
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await;
    });
    Ok(Server { _stop: stop })
}

/// The built screens, with a marker so they know to talk to this computer.
async fn asset(Extract(handle): Extract<AppHandle>, uri: Uri) -> Response {
    let path = match uri.path().trim_start_matches('/') {
        "" => "index.html",
        path => path,
    };
    let Some(asset) = handle.asset_resolver().get(path.to_string()) else {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    };
    let mut bytes = asset.bytes;
    if path == "index.html" {
        let html =
            String::from_utf8_lossy(&bytes).replacen("<head>", "<head><meta name=\"humlx-remote\" content=\"1\">", 1);
        bytes = html.into_bytes();
    }
    Response::builder()
        .header(header::CONTENT_TYPE, asset.mime_type)
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[derive(Deserialize)]
struct PairRequest {
    code: String,
    #[serde(default)]
    name: String,
}

async fn pair(Extract(handle): Extract<AppHandle>, Json(request): Json<PairRequest>) -> Response {
    let app = handle.state::<App>();
    match app.remote.pair(&request.code, &request.name) {
        Ok(key) => Json(serde_json::json!({ "key": key })).into_response(),
        Err(error) => (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": error }))).into_response(),
    }
}

#[derive(Deserialize)]
struct SocketQuery {
    #[serde(default)]
    key: String,
}

async fn socket(
    Extract(handle): Extract<AppHandle>,
    Query(query): Query<SocketQuery>,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade.on_upgrade(move |socket| run_socket(handle, query.key, socket))
}

#[derive(Deserialize)]
struct Request {
    id: u64,
    cmd: String,
    #[serde(default)]
    args: Value,
}

async fn run_socket(handle: AppHandle, key: String, mut socket: WebSocket) {
    let app = handle.state::<App>();
    let remote = &app.remote;
    let Some(device) = remote.device(&key) else {
        let _ = socket
            .send(Message::Close(Some(CloseFrame {
                code: CLOSE_UNPAIRED,
                reason: "Not paired".into(),
            })))
            .await;
        return;
    };
    let connection = remote.connect(device);
    let mut changes = remote.changes.subscribe();
    let mut kicks = remote.kicks.subscribe();
    let handle = Arc::new(handle.clone());

    loop {
        tokio::select! {
            message = socket.recv() => {
                let Some(Ok(message)) = message else { break };
                let Message::Text(text) = message else { continue };
                let Ok(request) = serde_json::from_str::<Request>(&text) else { continue };
                let id = request.id;
                let handle = handle.clone();
                let result = tokio::task::spawn_blocking(move || run(&handle, connection, request))
                    .await
                    .unwrap_or_else(|_| Err("The Studio PC hit a problem running that".into()));
                let reply = match result {
                    Ok(value) => serde_json::json!({ "id": id, "ok": value }),
                    Err(error) => serde_json::json!({ "id": id, "err": error }),
                };
                if socket.send(Message::Text(reply.to_string().into())).await.is_err() {
                    break;
                }
            }
            change = changes.recv() => {
                let parts: &[&str] = match change {
                    Ok(change) if change.from == Some(connection) => continue,
                    Ok(change) => change.parts,
                    // Missed some: reload everything.
                    Err(broadcast::error::RecvError::Lagged(_)) => &["show"],
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                let note = serde_json::json!({ "changed": parts });
                if socket.send(Message::Text(note.to_string().into())).await.is_err() {
                    break;
                }
            }
            kicked = kicks.changed() => {
                if kicked.is_err() || remote.device(&key) != Some(device) {
                    let _ = socket
                        .send(Message::Close(Some(CloseFrame { code: CLOSE_UNPAIRED, reason: "Not paired".into() })))
                        .await;
                    break;
                }
            }
        }
    }
    remote.disconnect(connection);
}

/// Runs a command from a remote through the app's own invoke handler.
fn run(handle: &AppHandle, connection: u64, request: Request) -> Result<Value, String> {
    if console_only(&request.cmd) {
        return Err("That can only be done on the Studio PC".into());
    }
    let window = handle
        .webview_windows()
        .into_values()
        .next()
        .ok_or("The HumLX window on the Studio PC is closed")?;
    let url = window.url().map_err(|e| e.to_string())?;
    let mut headers = tauri::http::HeaderMap::new();
    headers.insert(REMOTE_HEADER, connection.into());
    let args = match request.args {
        Value::Null => Value::Object(Default::default()),
        args => args,
    };
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    window.on_message(
        InvokeRequest {
            cmd: request.cmd,
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url,
            body: InvokeBody::Json(args),
            headers,
            invoke_key: handle.invoke_key().to_string(),
        },
        Box::new(move |_webview, _cmd, response, _callback, _error| {
            let _ = send.send(response);
        }),
    );
    match receive.recv_timeout(Duration::from_secs(30)) {
        Ok(InvokeResponse::Ok(InvokeResponseBody::Json(json))) => {
            serde_json::from_str(&json).map_err(|e| e.to_string())
        }
        Ok(InvokeResponse::Ok(InvokeResponseBody::Raw(bytes))) => {
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())
        }
        Ok(InvokeResponse::Err(error)) => Err(match error.0 {
            Value::String(message) => message,
            other => other.to_string(),
        }),
        Err(_) => Err("The Studio PC didn't answer".into()),
    }
}

/// The remote connection an invoke came from, if any.
pub fn connection_of(headers: &tauri::http::HeaderMap) -> Option<u64> {
    headers.get(REMOTE_HEADER)?.to_str().ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote() -> Remote {
        let path = std::env::temp_dir().join(format!("humlx-remote-test-{}.json", random_u64()));
        let remote = Remote::load(path);
        remote.inner().settings.enabled = true;
        remote
    }

    #[test]
    fn pairs_with_the_right_code_only() {
        let remote = remote();
        assert!(remote.pair("123456", "Laptop").is_err(), "no code showing yet");
        remote.new_code();
        let code = remote.status().code.unwrap();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        assert!(remote.pair(wrong, "Laptop").is_err());
        let key = remote.pair(&code, " Evan's laptop ").unwrap();
        assert_eq!(remote.device(&key), Some(1));
        assert_eq!(remote.status().devices[0].name, "Evan's laptop");
        assert!(remote.pair(&code, "Again").is_err(), "a code works once");
        std::fs::remove_file(&remote.path).ok();
    }

    #[test]
    fn too_many_wrong_codes_void_it() {
        let remote = remote();
        remote.new_code();
        let code = remote.status().code.unwrap();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        for _ in 0..CODE_ATTEMPTS {
            assert!(remote.pair(wrong, "").is_err());
        }
        assert!(remote.pair(&code, "").is_err());
        assert!(remote.status().code.is_none());
    }

    #[test]
    fn forgotten_and_disabled_devices_are_refused() {
        let remote = remote();
        remote.new_code();
        let code = remote.status().code.unwrap();
        let key = remote.pair(&code, "").unwrap();
        assert_eq!(remote.status().devices[0].name, "Remote 1");
        remote.inner().settings.enabled = false;
        assert_eq!(remote.device(&key), None);
        remote.inner().settings.enabled = true;
        remote.forget(1);
        assert_eq!(remote.device(&key), None);
        std::fs::remove_file(&remote.path).ok();
    }

    #[test]
    fn studio_pc_only_commands() {
        assert!(console_only("open_show"));
        assert!(console_only("set_remote"));
        assert!(console_only("plugin:dialog|open"));
        assert!(!console_only("point_at"));
        assert_eq!(changes("calibrate_mark"), &["stage"]);
        assert!(changes("live").is_empty());
    }
}
