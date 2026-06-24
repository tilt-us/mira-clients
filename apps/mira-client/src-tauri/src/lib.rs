use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::Mutex,
    thread,
    time::Duration,
};
use tauri::Manager;

const CONFIG_FILE_NAME: &str = "mira-client.toml";
#[cfg(debug_assertions)]
const DEFAULT_API_BASE_URL: &str = "http://localhost:8080";
#[cfg(not(debug_assertions))]
const DEFAULT_API_BASE_URL: &str = "https://api.tilt-us.com/auth";
#[cfg(debug_assertions)]
const DEFAULT_KEYCLOAK_BASE_URL: &str = "http://localhost:8081";
#[cfg(not(debug_assertions))]
const DEFAULT_KEYCLOAK_BASE_URL: &str = "https://api.tilt-us.com/keycloak";
#[cfg(debug_assertions)]
const DEFAULT_LIVE_API_BASE_URL: &str = "http://localhost:8082";
#[cfg(not(debug_assertions))]
const DEFAULT_LIVE_API_BASE_URL: &str = "https://api.tilt-us.com/live";
#[cfg(debug_assertions)]
const DEFAULT_MATCHMAKING_API_BASE_URL: &str = "http://localhost:8083";
#[cfg(not(debug_assertions))]
const DEFAULT_MATCHMAKING_API_BASE_URL: &str = "https://api.tilt-us.com/match";
const DEFAULT_KEYCLOAK_REALM: &str = "mira";
const DEFAULT_KEYCLOAK_CLIENT_ID: &str = "mira-bevy";
const DEFAULT_KEYCLOAK_PASSWORD_CLIENT_ID: &str = "mira-e2e";
const FORCE_RESTART_RECONNECT_DELAY: Duration = Duration::from_millis(8_500);

#[derive(serde::Serialize)]
struct LauncherStatus {
    game_binary: &'static str,
    connected: bool,
}

#[derive(Default)]
struct GameProcessState {
    child: Mutex<Option<Child>>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientConfig {
    api_base_url: String,
    keycloak_base_url: String,
    keycloak_realm: String,
    keycloak_client_id: String,
    keycloak_password_client_id: String,
    live_api_base_url: String,
    matchmaking_api_base_url: String,
}

#[derive(Default, serde::Deserialize)]
struct ClientConfigFile {
    services: Option<ServiceConfigFile>,
    keycloak: Option<KeycloakConfigFile>,
}

#[derive(Default, serde::Deserialize)]
struct ServiceConfigFile {
    api_base_url: Option<String>,
    live_api_base_url: Option<String>,
    matchmaking_api_base_url: Option<String>,
}

#[derive(Default, serde::Deserialize)]
struct KeycloakConfigFile {
    base_url: Option<String>,
    realm: Option<String>,
    client_id: Option<String>,
    password_client_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchGameRequest {
    access_token: String,
    accent_color: String,
    champion: String,
    #[serde(default)]
    force_restart: bool,
    #[serde(default)]
    match_manifest_json: String,
    match_id: String,
    matchmaking_api_base_url: String,
    player_public_id: u64,
    server_host: String,
    server_control_base_url: String,
    port: u16,
    #[serde(default)]
    screen: String,
    team: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchGameResponse {
    game_binary: String,
    pid: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GameClientStatus {
    running: bool,
    pid: Option<u32>,
}

#[tauri::command]
fn launcher_status() -> LauncherStatus {
    LauncherStatus {
        game_binary: "mira-game-client",
        connected: false,
    }
}

#[tauri::command]
fn client_config(app: tauri::AppHandle) -> Result<ClientConfig, String> {
    load_client_config(&app)
}

#[tauri::command]
fn launch_game(
    app: tauri::AppHandle,
    process_state: tauri::State<'_, GameProcessState>,
    request: LaunchGameRequest,
) -> Result<LaunchGameResponse, String> {
    let game_binary = resolve_game_binary(&app)?;
    let game_dir = game_binary
        .parent()
        .ok_or_else(|| "Game-Client-Verzeichnis konnte nicht bestimmt werden.".to_string())?;
    let asset_root = resolve_game_asset_root(&app, game_dir)?;

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
        .arg("--matchmaking-api-base-url")
        .arg(&request.matchmaking_api_base_url)
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

#[tauri::command]
fn game_client_status(
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

#[tauri::command]
fn stop_game_client(process_state: tauri::State<'_, GameProcessState>) -> Result<(), String> {
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

fn stop_game_child(child: &mut Child) -> Result<(), String> {
    child
        .kill()
        .map_err(|error| format!("Game-Client konnte nicht beendet werden: {error}"))?;
    child
        .wait()
        .map_err(|error| format!("Game-Client-Ende konnte nicht abgewartet werden: {error}"))?;
    Ok(())
}

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

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("files").join(binary_name));
        candidates.push(resource_dir.join(binary_name));
    }

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("files").join(binary_name));
        candidates.push(current_dir.join("..").join("files").join(binary_name));
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("files")
            .join(binary_name),
    );

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidates.push(exe_dir.join("files").join(binary_name));
            candidates.push(exe_dir.join(binary_name));
        }
    }

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

fn empty_as_default<'a>(value: &'a str, default_value: &'a str) -> &'a str {
    if value.trim().is_empty() {
        default_value
    } else {
        value
    }
}

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

fn format_path_candidates(candidates: &[PathBuf]) -> String {
    candidates
        .iter()
        .map(|candidate| candidate.to_string_lossy())
        .collect::<Vec<_>>()
        .join(", ")
}

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

fn load_client_config(app: &tauri::AppHandle) -> Result<ClientConfig, String> {
    let config_file = find_config_file(app);
    let parsed_config = match config_file {
        Some(path) => {
            println!(
                "[mira-client] Loading client config: {}",
                path.to_string_lossy()
            );
            let contents = std::fs::read_to_string(&path).map_err(|error| {
                format!(
                    "{} konnte nicht gelesen werden: {error}",
                    path.to_string_lossy()
                )
            })?;

            toml::from_str::<ClientConfigFile>(&contents).map_err(|error| {
                format!(
                    "{} konnte nicht als TOML gelesen werden: {error}",
                    path.to_string_lossy()
                )
            })?
        }
        None => ClientConfigFile::default(),
    };

    Ok(parsed_config.into_runtime_config())
}

fn find_config_file(app: &tauri::AppHandle) -> Option<PathBuf> {
    config_file_candidates(app)
        .into_iter()
        .find(|candidate| candidate.is_file())
}

fn config_file_candidates(app: &tauri::AppHandle) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(config_path) = std::env::var_os("MIRA_CLIENT_CONFIG") {
        candidates.push(PathBuf::from(config_path));
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidates.push(exe_dir.join(CONFIG_FILE_NAME));
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join(CONFIG_FILE_NAME));
        candidates.push(current_dir.join("..").join(CONFIG_FILE_NAME));
    }

    if let Ok(app_config_dir) = app.path().app_config_dir() {
        candidates.push(app_config_dir.join(CONFIG_FILE_NAME));
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(CONFIG_FILE_NAME));
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(CONFIG_FILE_NAME),
    );

    candidates
}

impl ClientConfigFile {
    fn into_runtime_config(self) -> ClientConfig {
        let services = self.services.unwrap_or_default();
        let keycloak = self.keycloak.unwrap_or_default();

        ClientConfig {
            api_base_url: normalize_base_url(
                services.api_base_url.as_deref(),
                DEFAULT_API_BASE_URL,
            ),
            keycloak_base_url: normalize_base_url(
                keycloak.base_url.as_deref(),
                DEFAULT_KEYCLOAK_BASE_URL,
            ),
            keycloak_realm: value_or_default(keycloak.realm.as_deref(), DEFAULT_KEYCLOAK_REALM),
            keycloak_client_id: value_or_default(
                keycloak.client_id.as_deref(),
                DEFAULT_KEYCLOAK_CLIENT_ID,
            ),
            keycloak_password_client_id: value_or_default(
                keycloak.password_client_id.as_deref(),
                DEFAULT_KEYCLOAK_PASSWORD_CLIENT_ID,
            ),
            live_api_base_url: normalize_base_url(
                services.live_api_base_url.as_deref(),
                DEFAULT_LIVE_API_BASE_URL,
            ),
            matchmaking_api_base_url: normalize_base_url(
                services.matchmaking_api_base_url.as_deref(),
                DEFAULT_MATCHMAKING_API_BASE_URL,
            ),
        }
    }
}

fn normalize_base_url(value: Option<&str>, default_value: &str) -> String {
    value_or_default(value, default_value)
        .trim_end_matches('/')
        .to_string()
}

fn value_or_default(value: Option<&str>, default_value: &str) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_value)
        .to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(GameProcessState::default())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            client_config,
            game_client_status,
            launcher_status,
            launch_game,
            stop_game_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
