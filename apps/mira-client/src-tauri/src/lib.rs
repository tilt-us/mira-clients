use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::Mutex,
    thread,
    time::Duration,
};
use tauri::Manager;

const CONFIG_FILE_NAME: &str = "mira-client.toml";
const DEFAULT_API_BASE_URL: &str = "http://localhost:8080";
const DEFAULT_KEYCLOAK_BASE_URL: &str = "http://localhost:8081";
const DEFAULT_LIVE_API_BASE_URL: &str = "http://localhost:8082";
const DEFAULT_MATCHMAKING_API_BASE_URL: &str = "http://localhost:8083";
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
    match_id: String,
    matchmaking_api_base_url: String,
    player_public_id: u64,
    server_host: String,
    server_control_base_url: String,
    port: u16,
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
        .arg(request.access_token)
        .arg("--accent-color")
        .arg(request.accent_color)
        .arg("--champion")
        .arg(request.champion)
        .arg("--match-id")
        .arg(request.match_id)
        .arg("--matchmaking-api-base-url")
        .arg(request.matchmaking_api_base_url)
        .arg("--player-public-id")
        .arg(request.player_public_id.to_string())
        .arg("--server-host")
        .arg(request.server_host)
        .arg("--port")
        .arg(request.port.to_string())
        .arg("--server-control-base-url")
        .arg(request.server_control_base_url)
        .arg("--team")
        .arg(request.team);

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

    let child = command
        .spawn()
        .map_err(|error| format!("Game-Client konnte nicht gestartet werden: {error}"))?;
    let pid = child.id();
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
            Ok(Some(_)) => {
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
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| "files/mira-game-client wurde nicht gefunden.".to_string())
}

fn resolve_game_asset_root(app: &tauri::AppHandle, game_dir: &std::path::Path) -> Result<PathBuf, String> {
    game_asset_root_candidates(app, game_dir)
        .into_iter()
        .find(|candidate| candidate.join("index.html").is_file())
        .ok_or_else(|| "Game-Assets wurden nicht gefunden: assets/index.html fehlt.".to_string())
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
            launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
