use crate::content;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Mutex,
    thread,
    time::Duration,
};
use tauri::Manager;

const FORCE_RESTART_RECONNECT_DELAY: Duration = Duration::from_millis(8_500);

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
    server_port: u16,
    protocol: String,
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
    validate_gameplay_endpoint(&request)?;
    let game_binary = resolve_game_binary(&app)?;
    let game_dir = game_binary
        .parent()
        .ok_or_else(|| "Game-Client-Verzeichnis konnte nicht bestimmt werden.".to_string())?;
    let asset_root = resolve_game_asset_root(&app, &game_binary)?;

    let mut command = Command::new(&game_binary);
    command
        .current_dir(game_dir)
        .env("MIRA_GAME_ASSET_ROOT", &asset_root);
    append_game_launch_args(&mut command, &request);

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
        "[mira-client] Launching game client: matchId={} serverHost={} serverPort={} protocol={} binary={} cwd={} assets={} player={} champion={} screen={} team={}",
        request.match_id,
        request.server_host,
        request.server_port,
        request.protocol,
        game_binary.to_string_lossy(),
        game_dir.to_string_lossy(),
        asset_root.to_string_lossy(),
        request.player_public_id,
        request.champion,
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

fn append_game_launch_args(command: &mut Command, request: &LaunchGameRequest) {
    command
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
        .arg(request.server_port.to_string());
}

fn validate_gameplay_endpoint(request: &LaunchGameRequest) -> Result<(), String> {
    if request.server_host.trim().is_empty() {
        return Err("Game server address missing".to_string());
    }

    if request.server_port == 0 {
        return Err("Game server port missing".to_string());
    }

    if !request.protocol.eq_ignore_ascii_case("UDP") {
        return Err("Game server protocol unsupported".to_string());
    }

    Ok(())
}

/// Resolves the one asset root that contains both standalone game and UI assets.
fn resolve_game_asset_root(app: &tauri::AppHandle, game_binary: &Path) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) && std::env::var_os(mira_downloads::INSTALL_ROOT_ENV).is_none() {
        return development_game_asset_root(game_binary);
    }

    content::ready_game_asset_root(app)
}

/// Locates a checkout's assets folder relative to the development game binary.
///
/// This deliberately walks from the executable path instead of embedding a developer
/// workstation path in the launcher.
fn development_game_asset_root(game_binary: &Path) -> Result<PathBuf, String> {
    game_binary
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .map(|directory| directory.join("assets"))
        .find(|candidate| {
            content::has_required_game_content(&candidate.join("game"))
                && content::has_required_game_client_runtime_ui(candidate)
        })
        .and_then(|candidate| candidate.canonicalize().ok())
        .ok_or_else(|| {
            "Development game assets are incomplete. Expected an assets directory containing game content and required UI files."
                .to_string()
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

fn format_path_candidates(candidates: &[PathBuf]) -> String {
    candidates
        .iter()
        .map(|candidate| candidate.to_string_lossy())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Runs the resolve game asset root step for the desktop game-launcher process system.
#[cfg(test)]
mod tests {
    use super::*;

    fn launch_request() -> LaunchGameRequest {
        LaunchGameRequest {
            access_token: "token".to_string(),
            accent_color: "#123456".to_string(),
            champion: "lira".to_string(),
            force_restart: false,
            match_manifest_json: String::new(),
            match_id: "match-1".to_string(),
            player_public_id: 42,
            server_host: "217.160.25.101".to_string(),
            server_port: 7035,
            protocol: "UDP".to_string(),
            screen: "window".to_string(),
            team: "light".to_string(),
        }
    }

    #[test]
    fn passes_the_dynamic_udp_gameplay_endpoint_without_a_control_url() {
        let mut command = Command::new("mira-game-client");
        append_game_launch_args(&mut command, &launch_request());
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["--server-host", "217.160.25.101"])
        );
        assert!(arguments.windows(2).any(|pair| pair == ["--port", "7035"]));
        assert!(
            !arguments
                .iter()
                .any(|argument| argument == "--server-control-base-url")
        );
    }

    #[test]
    fn rejects_incomplete_or_unsupported_gameplay_endpoints() {
        let mut missing_host = launch_request();
        missing_host.server_host.clear();
        assert_eq!(
            validate_gameplay_endpoint(&missing_host),
            Err("Game server address missing".to_string())
        );

        let mut missing_port = launch_request();
        missing_port.server_port = 0;
        assert_eq!(
            validate_gameplay_endpoint(&missing_port),
            Err("Game server port missing".to_string())
        );

        let mut unsupported_protocol = launch_request();
        unsupported_protocol.protocol = "TCP".to_string();
        assert_eq!(
            validate_gameplay_endpoint(&unsupported_protocol),
            Err("Game server protocol unsupported".to_string())
        );
    }

    #[test]
    fn game_content_sentinel_requires_the_authoritative_game_directories() {
        let directory = tempfile::tempdir().unwrap();
        let required_asset_root = directory.path().join("assets/game");
        assert!(!content::has_required_game_content(&required_asset_root));
        for directory in ["audio", "champions", "maps", "materials"] {
            std::fs::create_dir_all(required_asset_root.join(directory)).unwrap();
        }
        assert!(content::has_required_game_content(&required_asset_root));
    }

    #[test]
    fn development_asset_root_is_found_relative_to_the_game_binary() {
        let directory = tempfile::tempdir().unwrap();
        let game_binary = directory.path().join("target/debug/mira-game-client");
        let assets = directory.path().join("assets");
        for directory in ["audio", "champions", "maps", "materials"] {
            std::fs::create_dir_all(assets.join("game").join(directory)).unwrap();
        }
        for asset in [
            "ui/wallpapers/lira-loading.jpg",
            "ui/wallpapers/ignara-loading.jpg",
            "ui/wallpapers/sophia-loading.jpg",
            "ui/wallpapers/yuna-loading.jpg",
            "ui/characters/lira.png",
            "ui/characters/ignara.png",
            "ui/characters/sophia.png",
            "ui/characters/yuna.png",
            "ui/fonts/Roboto-Bold.ttf",
        ] {
            let path = assets.join(asset);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, []).unwrap();
        }

        assert_eq!(
            development_game_asset_root(&game_binary).unwrap(),
            assets.canonicalize().unwrap()
        );
    }
}
