use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    process::{Child, Command},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;

const FORCE_RESTART_RECONNECT_DELAY: Duration = Duration::from_millis(8_500);
const GAME_SERVER_READY_TIMEOUT: Duration = Duration::from_secs(12);
const GAME_SERVER_READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const GAME_SERVER_READY_REQUEST_TIMEOUT: Duration = Duration::from_millis(500);

/// Stores Game Process State data used by the desktop game-launcher process system.
#[derive(Default)]
pub(crate) struct GameProcessState {
    child: Mutex<Option<Child>>,
}

/// Stores Launch Game Request data used by the desktop game-launcher process system.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchGameRequest {
    access_token: String,
    accent_color: String,
    champion: String,
    #[serde(default)]
    force_restart: bool,
    #[serde(default)]
    match_manifest_json: String,
    match_id: String,
    player_public_id: u64,
    server_host: String,
    server_control_base_url: String,
    port: u16,
    #[serde(default)]
    screen: String,
    team: String,
}

/// Stores Launch Game Response data used by the desktop game-launcher process system.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchGameResponse {
    game_binary: String,
    pid: u32,
}

/// Stores Game Client Status data used by the desktop game-launcher process system.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameClientStatus {
    running: bool,
    pid: Option<u32>,
}

/// Runs the launch game step for the desktop game-launcher process system.
#[tauri::command]
pub(crate) fn launch_game(
    app: tauri::AppHandle,
    process_state: tauri::State<'_, GameProcessState>,
    request: LaunchGameRequest,
) -> Result<LaunchGameResponse, String> {
    let game_binary = resolve_game_binary(&app)?;
    let game_dir = game_binary
        .parent()
        .ok_or_else(|| "Game-Client-Verzeichnis konnte nicht bestimmt werden.".to_string())?;
    let asset_root = resolve_game_asset_root(&app, game_dir)?;
    wait_for_game_server_ready(&request.server_control_base_url)?;

    let mut command = Command::new(&game_binary);
    command
        .current_dir(game_dir)
        .env("MIRA_GAME_ASSET_ROOT", &asset_root)
        .arg("--access-token")
        .arg(&request.access_token)
        .arg("--accent-color")
        .arg(&request.accent_color)
        .arg("--champion")
        .arg(&request.champion)
        .arg("--match-id")
        .arg(&request.match_id)
        .arg("--player-public-id")
        .arg(request.player_public_id.to_string())
        .arg("--server-host")
        .arg(&request.server_host)
        .arg("--port")
        .arg(request.port.to_string())
        .arg("--server-control-base-url")
        .arg(&request.server_control_base_url);

    if !request.screen.trim().is_empty() {
        command.arg("--screen").arg(&request.screen);
    }

    command.arg("--team").arg(&request.team);

    if !request.match_manifest_json.trim().is_empty() {
        command.env("MIRA_MATCH_MANIFEST_JSON", &request.match_manifest_json);
    }

    let mut active_child = process_state
        .child
        .lock()
        .map_err(|_| "Game-Client-Status konnte nicht gesperrt werden.".to_string())?;

    let mut killed_active_child = false;

    if let Some(child) = active_child.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => {
                *active_child = None;
            }
            Ok(None) => {
                if request.force_restart {
                    stop_game_child(child)?;
                    *active_child = None;
                    killed_active_child = true;
                } else {
                    return Ok(LaunchGameResponse {
                        game_binary: game_binary.to_string_lossy().into_owned(),
                        pid: child.id(),
                    });
                }
            }
            Err(error) => {
                return Err(format!(
                    "Game-Client-Status konnte nicht geprüft werden: {error}"
                ));
            }
        }
    }

    if killed_active_child {
        thread::sleep(FORCE_RESTART_RECONNECT_DELAY);
    }

    println!(
        "[mira-client] Starting game client: binary={} cwd={} assets={} match={} player={} champion={} server={}:{} control={} screen={} team={}",
        game_binary.to_string_lossy(),
        game_dir.to_string_lossy(),
        asset_root.to_string_lossy(),
        request.match_id,
        request.player_public_id,
        request.champion,
        request.server_host,
        request.port,
        request.server_control_base_url,
        empty_as_default(&request.screen, "default"),
        request.team,
    );

    let mut child = command.spawn().map_err(|error| {
        eprintln!(
            "[mira-client] Game client spawn failed: binary={} error={error}",
            game_binary.to_string_lossy(),
        );
        format!("Game-Client konnte nicht gestartet werden: {error}")
    })?;
    let pid = child.id();
    println!("[mira-client] Game client started: pid={pid}");

    thread::sleep(Duration::from_millis(800));

    if let Some(status) = child
        .try_wait()
        .map_err(|error| format!("Game-Client-Startstatus konnte nicht geprüft werden: {error}"))?
    {
        return Err(format!(
            "Game-Client wurde direkt nach dem Start beendet: pid={pid} status={status}"
        ));
    }

    *active_child = Some(child);

    Ok(LaunchGameResponse {
        game_binary: game_binary.to_string_lossy().into_owned(),
        pid,
    })
}

/// Runs the game client status step for the desktop game-launcher process system.
#[tauri::command]
pub(crate) fn game_client_status(
    process_state: tauri::State<'_, GameProcessState>,
) -> Result<GameClientStatus, String> {
    let mut active_child = process_state
        .child
        .lock()
        .map_err(|_| "Game-Client-Status konnte nicht gesperrt werden.".to_string())?;

    if let Some(child) = active_child.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                println!(
                    "[mira-client] Game client exited before status check: pid={} status={status}",
                    child.id(),
                );
                *active_child = None;
            }
            Ok(None) => {
                return Ok(GameClientStatus {
                    running: true,
                    pid: Some(child.id()),
                });
            }
            Err(error) => {
                return Err(format!(
                    "Game-Client-Status konnte nicht geprüft werden: {error}"
                ));
            }
        }
    }

    Ok(GameClientStatus {
        running: false,
        pid: None,
    })
}

/// Runs the stop game client step for the desktop game-launcher process system.
#[tauri::command]
pub(crate) fn stop_game_client(
    process_state: tauri::State<'_, GameProcessState>,
) -> Result<(), String> {
    let mut active_child = process_state
        .child
        .lock()
        .map_err(|_| "Game-Client-Status konnte nicht gesperrt werden.".to_string())?;

    if let Some(child) = active_child.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                println!(
                    "[mira-client] Game client already exited before stop: pid={} status={status}",
                    child.id(),
                );
                *active_child = None;
                return Ok(());
            }
            Ok(None) => {
                stop_game_child(child)?;
                *active_child = None;
                return Ok(());
            }
            Err(error) => {
                return Err(format!(
                    "Game-Client-Status konnte nicht geprüft werden: {error}"
                ));
            }
        }
    }

    Ok(())
}

/// Runs the stop game child step for the desktop game-launcher process system.
#[tauri::command]
fn stop_game_child(child: &mut Child) -> Result<(), String> {
    child
        .kill()
        .map_err(|error| format!("Game-Client konnte nicht beendet werden: {error}"))?;
    child
        .wait()
        .map_err(|error| format!("Game-Client-Ende konnte nicht abgewartet werden: {error}"))?;
    Ok(())
}

/// Runs the resolve game binary step for the desktop game-launcher process system.
fn resolve_game_binary(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let binary_name = if cfg!(windows) {
        "mira-game-client.exe"
    } else {
        "mira-game-client"
    };

    let mut candidates = Vec::new();

    if let Some(binary_path) = std::env::var_os("MIRA_GAME_CLIENT_BINARY") {
        candidates.push(PathBuf::from(binary_path));
    }

    if cfg!(debug_assertions) {
        candidates.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("..")
                .join("target")
                .join("debug")
                .join(binary_name),
        );
    }

    if let Some(appimage_path) = std::env::var_os("APPIMAGE") {
        if let Some(appimage_dir) = PathBuf::from(appimage_path).parent() {
            candidates.push(appimage_dir.join(binary_name));
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join(binary_name));
        candidates.push(current_dir.join("..").join(binary_name));
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidates.push(exe_dir.join(binary_name));
            candidates.push(exe_dir.join("..").join(binary_name));
        }
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(binary_name));
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join(binary_name),
    );

    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .cloned()
        .ok_or_else(|| {
            format!(
                "mira-game-client wurde nicht gefunden. Geprüfte Pfade: {}. Baue den Game-Client mit `cargo build -p mira-game-client` oder setze MIRA_GAME_CLIENT_BINARY.",
                format_path_candidates(&candidates),
            )
        })
}

/// Runs the empty as default step for the desktop game-launcher process system.
fn empty_as_default<'a>(value: &'a str, default_value: &'a str) -> &'a str {
    if value.trim().is_empty() {
        default_value
    } else {
        value
    }
}

/// Waits until the dedicated server confirms that its UDP listener is ready.
fn wait_for_game_server_ready(control_base_url: &str) -> Result<(), String> {
    let endpoint = game_server_ready_endpoint(control_base_url)?;
    let deadline = Instant::now() + GAME_SERVER_READY_TIMEOUT;

    let last_error = loop {
        let error = match request_game_server_readiness(&endpoint) {
            Ok(true) => return Ok(()),
            Ok(false) => "the server is still starting".to_string(),
            Err(error) => error,
        };

        if Instant::now() >= deadline {
            break error;
        }

        thread::sleep(GAME_SERVER_READY_POLL_INTERVAL);
    };

    Err(format!(
        "Game-Server wurde unter {control_base_url} nicht rechtzeitig bereit: {last_error}",
    ))
}

/// Builds the control API readiness endpoint from the match server base URL.
fn game_server_ready_endpoint(control_base_url: &str) -> Result<tauri::Url, String> {
    let mut endpoint = tauri::Url::parse(control_base_url)
        .map_err(|error| format!("Ungültige Game-Server-Control-Adresse: {error}"))?;
    if endpoint.scheme() != "http" {
        return Err("Game-Server-Control-Adresse muss HTTP verwenden.".to_string());
    }
    if endpoint.host_str().is_none() {
        return Err("Game-Server-Control-Adresse enthält keinen Host.".to_string());
    }

    endpoint.set_path("/ready");
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    Ok(endpoint)
}

/// Requests the dedicated server readiness endpoint once.
fn request_game_server_readiness(endpoint: &tauri::Url) -> Result<bool, String> {
    let host = endpoint
        .host_str()
        .ok_or_else(|| "Game-Server-Control-Adresse enthält keinen Host.".to_string())?;
    let port = endpoint
        .port_or_known_default()
        .ok_or_else(|| "Game-Server-Control-Adresse enthält keinen Port.".to_string())?;
    let addresses = (host, port).to_socket_addrs().map_err(|error| {
        format!("Game-Server-Control-Host konnte nicht aufgelöst werden: {error}")
    })?;
    let mut connection_errors = Vec::new();
    let mut stream = None;

    for address in addresses {
        match TcpStream::connect_timeout(&address, GAME_SERVER_READY_REQUEST_TIMEOUT) {
            Ok(connection) => {
                stream = Some(connection);
                break;
            }
            Err(error) => connection_errors.push(error),
        }
    }

    let mut stream = stream.ok_or_else(|| {
        let detail = connection_errors
            .last()
            .map(|error| error.to_string())
            .unwrap_or_else(|| "no socket address was returned".to_string());
        format!("Game-Server-Control-Verbindung fehlgeschlagen: {detail}")
    })?;
    stream
        .set_read_timeout(Some(GAME_SERVER_READY_REQUEST_TIMEOUT))
        .map_err(|error| format!("Game-Server-Control-Lesezeitlimit fehlgeschlagen: {error}"))?;
    stream
        .set_write_timeout(Some(GAME_SERVER_READY_REQUEST_TIMEOUT))
        .map_err(|error| format!("Game-Server-Control-Schreibzeitlimit fehlgeschlagen: {error}"))?;

    write!(
        stream,
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        endpoint.path(),
        host,
    )
    .map_err(|error| format!("Game-Server-Readiness-Anfrage fehlgeschlagen: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("Game-Server-Readiness-Antwort fehlgeschlagen: {error}"))?;

    Ok(is_game_server_ready_response(&response))
}

/// Returns whether a control API response confirms server readiness.
fn is_game_server_ready_response(response: &str) -> bool {
    response
        .lines()
        .next()
        .is_some_and(|status| status.contains(" 200 "))
}

/// Runs the resolve game asset root step for the desktop game-launcher process system.
fn resolve_game_asset_root(
    app: &tauri::AppHandle,
    game_dir: &std::path::Path,
) -> Result<PathBuf, String> {
    let candidates = game_asset_root_candidates(app, game_dir);

    candidates
        .iter()
        .find(|candidate| candidate.join("index.html").is_file())
        .cloned()
        .ok_or_else(|| {
            format!(
                "Game-Assets wurden nicht gefunden: assets/index.html fehlt. Geprüfte Pfade: {}.",
                format_path_candidates(&candidates),
            )
        })
}

/// Runs the format path candidates step for the desktop game-launcher process system.
fn format_path_candidates(candidates: &[PathBuf]) -> String {
    candidates
        .iter()
        .map(|candidate| candidate.to_string_lossy())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Runs the game asset root candidates step for the desktop game-launcher process system.
fn game_asset_root_candidates(app: &tauri::AppHandle, game_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    candidates.push(game_dir.join("assets"));

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("files").join("assets"));
        candidates.push(resource_dir.join("assets"));
    }

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("files").join("assets"));
        candidates.push(current_dir.join("assets"));
        candidates.push(current_dir.join("..").join("assets"));
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("files")
            .join("assets"),
    );

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("assets"),
    );

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_a_root_readiness_endpoint() {
        let endpoint = game_server_ready_endpoint("http://127.0.0.1:6000/api").unwrap();

        assert_eq!(endpoint.as_str(), "http://127.0.0.1:6000/ready");
    }

    #[test]
    fn accepts_only_successful_readiness_responses() {
        assert!(is_game_server_ready_response(
            "HTTP/1.1 200 OK\r\n\r\n{\"ready\":true}"
        ));
        assert!(!is_game_server_ready_response(
            "HTTP/1.1 503 Service Unavailable\r\n\r\n{\"ready\":false}"
        ));
    }
}
