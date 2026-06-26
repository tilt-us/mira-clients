use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

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
const OAUTH_MODAL_WIDTH_RATIO: f64 = 0.75;
const OAUTH_MODAL_HEIGHT_RATIO: f64 = 0.8;
const OAUTH_MODAL_FALLBACK_WIDTH: f64 = 960.0;
const OAUTH_MODAL_FALLBACK_HEIGHT: f64 = 720.0;
const OAUTH_MODAL_MIN_WIDTH: f64 = 720.0;
const OAUTH_MODAL_MIN_HEIGHT: f64 = 560.0;
const OAUTH_BROWSER_CALLBACK_TIMEOUT: Duration = Duration::from_secs(600);

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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthWindowRequest {
    auth_url: String,
    redirect_uri: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthCallbackPayload {
    url: String,
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

#[tauri::command]
fn start_oauth_window(app: tauri::AppHandle, request: OAuthWindowRequest) -> Result<(), String> {
    let auth_url_text = request.auth_url.trim().to_string();
    let auth_url = auth_url_text
        .parse()
        .map_err(|error| format!("OAuth-URL ist ungueltig: {error}"))?;
    let redirect_uri = request.redirect_uri.trim().to_string();

    if redirect_uri.is_empty() {
        return Err("OAuth-Redirect-URI fehlt.".to_string());
    }

    if cfg!(windows) && !cfg!(debug_assertions) {
        return start_external_oauth_login(app, auth_url_text, redirect_uri);
    }

    if let Some(existing_window) = app.get_webview_window("mira-oauth") {
        existing_window
            .close()
            .map_err(|error| format!("OAuth-Fenster konnte nicht ersetzt werden: {error}"))?;
    }

    let app_for_navigation = app.clone();
    let redirect_uri_for_navigation = redirect_uri.clone();
    let back_button_script = oauth_back_button_init_script(&redirect_uri)
        .map_err(|error| format!("OAuth-Button konnte nicht vorbereitet werden: {error}"))?;

    let mut modal_width = OAUTH_MODAL_FALLBACK_WIDTH;
    let mut modal_height = OAUTH_MODAL_FALLBACK_HEIGHT;
    let mut oauth_window_builder =
        tauri::WebviewWindowBuilder::new(&app, "mira-oauth", tauri::WebviewUrl::External(auth_url))
            .title("Mira Login")
            .min_inner_size(OAUTH_MODAL_MIN_WIDTH, OAUTH_MODAL_MIN_HEIGHT)
            .resizable(false)
            .decorations(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .initialization_script(back_button_script)
            .on_navigation(move |url| {
                let target_url = url.as_str();

                if is_oauth_redirect_url(target_url, &redirect_uri_for_navigation) {
                    let _ = app_for_navigation.emit(
                        "mira-oauth-callback",
                        OAuthCallbackPayload {
                            url: target_url.to_string(),
                        },
                    );

                    if let Some(oauth_window) = app_for_navigation.get_webview_window("mira-oauth")
                    {
                        let _ = oauth_window.close();
                    }

                    return false;
                }

                true
            });

    if let Some(main_window) = app.get_webview_window("main") {
        let geometry = oauth_modal_geometry(&main_window)?;
        modal_width = geometry.width;
        modal_height = geometry.height;
        oauth_window_builder = oauth_window_builder
            .parent(&main_window)
            .map_err(|error| {
                format!("OAuth-Modal konnte nicht an das Main-Window gebunden werden: {error}")
            })?
            .position(geometry.x, geometry.y);
    } else {
        oauth_window_builder = oauth_window_builder.center();
    }

    let oauth_window = oauth_window_builder
        .inner_size(modal_width, modal_height)
        .build()
        .map_err(|error| format!("OAuth-Fenster konnte nicht geoeffnet werden: {error}"))?;

    disable_webview_hardware_acceleration(&oauth_window);

    let app_for_close = app.clone();
    oauth_window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let _ = app_for_close.emit("mira-oauth-closed", ());
        }
    });

    Ok(())
}

fn start_external_oauth_login(
    app: tauri::AppHandle,
    auth_url: String,
    redirect_uri: String,
) -> Result<(), String> {
    let listeners = bind_oauth_redirect_listeners(&redirect_uri)?;
    let callback_origin = oauth_redirect_origin(&redirect_uri)?;
    let completed = Arc::new(AtomicBool::new(false));

    for listener in listeners {
        let app_for_callback = app.clone();
        let callback_origin = callback_origin.clone();
        let completed = Arc::clone(&completed);

        thread::spawn(move || {
            accept_oauth_redirect(listener, app_for_callback, callback_origin, completed);
        });
    }

    tauri_plugin_opener::open_url(&auth_url, None::<&str>)
        .map_err(|error| format!("OAuth-Browser konnte nicht geoeffnet werden: {error}"))
}

fn bind_oauth_redirect_listeners(redirect_uri: &str) -> Result<Vec<TcpListener>, String> {
    let redirect_url: tauri::Url = redirect_uri
        .parse()
        .map_err(|error| format!("OAuth-Redirect-URI ist ungueltig: {error}"))?;
    let port = redirect_url
        .port_or_known_default()
        .ok_or_else(|| "OAuth-Redirect-URI hat keinen Port.".to_string())?;

    let mut listeners = Vec::new();
    let mut errors = Vec::new();

    for address in [format!("127.0.0.1:{port}"), format!("[::1]:{port}")] {
        match TcpListener::bind(&address) {
            Ok(listener) => {
                listener.set_nonblocking(true).map_err(|error| {
                    format!("OAuth-Redirect-Port {port} konnte nicht vorbereitet werden: {error}")
                })?;
                listeners.push(listener);
            }
            Err(error) => {
                errors.push(format!("{address}: {error}"));
            }
        }
    }

    if listeners.is_empty() {
        return Err(format!(
            "OAuth-Redirect-Port {port} konnte nicht geoeffnet werden: {}",
            errors.join(", ")
        ));
    }

    Ok(listeners)
}

fn oauth_redirect_origin(redirect_uri: &str) -> Result<String, String> {
    let redirect_url: tauri::Url = redirect_uri
        .parse()
        .map_err(|error| format!("OAuth-Redirect-URI ist ungueltig: {error}"))?;
    let host = redirect_url
        .host_str()
        .ok_or_else(|| "OAuth-Redirect-URI hat keinen Host.".to_string())?;
    let port = redirect_url
        .port_or_known_default()
        .ok_or_else(|| "OAuth-Redirect-URI hat keinen Port.".to_string())?;

    Ok(format!("{}://{}:{}", redirect_url.scheme(), host, port))
}

fn accept_oauth_redirect(
    listener: TcpListener,
    app: tauri::AppHandle,
    callback_origin: String,
    completed: Arc<AtomicBool>,
) {
    let started_at = Instant::now();

    while !completed.load(Ordering::Relaxed)
        && started_at.elapsed() < OAUTH_BROWSER_CALLBACK_TIMEOUT
    {
        match listener.accept() {
            Ok((stream, _)) => {
                if completed.swap(true, Ordering::Relaxed) {
                    return;
                }

                handle_oauth_redirect_stream(stream, app, callback_origin);
                return;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return,
        }
    }
}

fn handle_oauth_redirect_stream(
    mut stream: TcpStream,
    app: tauri::AppHandle,
    callback_origin: String,
) {
    let callback_url = read_oauth_callback_url(&stream, &callback_origin);
    let _ = write_oauth_browser_response(&mut stream, callback_url.is_some());

    if let Some(url) = callback_url {
        let _ = app.emit("mira-oauth-callback", OAuthCallbackPayload { url });
    } else {
        let _ = app.emit("mira-oauth-closed", ());
    }
}

fn read_oauth_callback_url(stream: &TcpStream, callback_origin: &str) -> Option<String> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;

    let target = request_line.split_whitespace().nth(1)?;
    if target.starts_with("http://") || target.starts_with("https://") {
        return Some(target.to_string());
    }

    Some(format!("{callback_origin}{target}"))
}

fn write_oauth_browser_response(stream: &mut TcpStream, success: bool) -> std::io::Result<()> {
    let message = if success {
        "Mira login complete. You can close this browser tab and return to Mira."
    } else {
        "Mira login could not be completed. You can close this browser tab and return to Mira."
    };
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Mira Login</title></head><body>{message}</body></html>"
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    stream.write_all(response.as_bytes())
}

struct OAuthModalGeometry {
    height: f64,
    width: f64,
    x: f64,
    y: f64,
}

fn oauth_modal_geometry(main_window: &tauri::WebviewWindow) -> Result<OAuthModalGeometry, String> {
    let scale_factor = main_window
        .scale_factor()
        .map_err(|error| format!("Main-Window-Skalierung konnte nicht gelesen werden: {error}"))?;
    let main_position = main_window
        .outer_position()
        .map_err(|error| format!("Main-Window-Position konnte nicht gelesen werden: {error}"))?;
    let main_size = main_window
        .inner_size()
        .map_err(|error| format!("Main-Window-Groesse konnte nicht gelesen werden: {error}"))?;

    let main_x = f64::from(main_position.x) / scale_factor;
    let main_y = f64::from(main_position.y) / scale_factor;
    let main_width = f64::from(main_size.width) / scale_factor;
    let main_height = f64::from(main_size.height) / scale_factor;
    let modal_width = (main_width * OAUTH_MODAL_WIDTH_RATIO)
        .max(OAUTH_MODAL_MIN_WIDTH)
        .min((main_width - 48.0).max(OAUTH_MODAL_MIN_WIDTH));
    let modal_height = (main_height * OAUTH_MODAL_HEIGHT_RATIO)
        .max(OAUTH_MODAL_MIN_HEIGHT)
        .min((main_height - 48.0).max(OAUTH_MODAL_MIN_HEIGHT));

    let x = main_x + ((main_width - modal_width) / 2.0).max(24.0);
    let y = main_y + ((main_height - modal_height) / 2.0).max(24.0);

    Ok(OAuthModalGeometry {
        height: modal_height,
        width: modal_width,
        x,
        y,
    })
}

fn is_oauth_redirect_url(target_url: &str, redirect_uri: &str) -> bool {
    target_url == redirect_uri
        || target_url
            .strip_prefix(redirect_uri)
            .is_some_and(|rest| rest.starts_with('?') || rest.starts_with('#'))
}

fn oauth_back_button_init_script(redirect_uri: &str) -> Result<String, serde_json::Error> {
    let redirect_uri = serde_json::to_string(redirect_uri)?;

    Ok(format!(
        r###"
(function () {{
  var redirectUri = {redirect_uri};
  var backButtonId = "mira-oauth-back-button";
  var closeButtonId = "mira-oauth-close-button";
  var svgNamespace = "http://www.w3.org/2000/svg";

  function closeModal() {{
    window.location.href = redirectUri;
  }}

  function isKeycloakPage() {{
    return /\/realms\/[^/]+\/(protocol\/openid-connect\/auth|login-actions|broker)(\/|$)/.test(
      window.location.pathname
    );
  }}

  function applyButtonStyle(button, side, compact) {{
    var style = button.style;
    style.position = "fixed";
    style.top = compact ? "10px" : "35px";
    style[side] = compact ? "10px" : "35px";
    style.zIndex = "2147483647";
    style.width = compact ? "34px" : "42px";
    style.height = compact ? "34px" : "42px";
    style.display = "grid";
    style.placeItems = "center";
    style.border = "1px solid rgba(237, 242, 247, 0.18)";
    style.borderRadius = compact ? "999px" : "8px";
    style.background = "rgba(23, 26, 32, 0.82)";
    style.color = "rgba(255, 255, 255, 0.92)";
    style.boxShadow = "0 12px 28px rgba(0, 0, 0, 0.22)";
    style.cursor = "pointer";
    style.padding = "0";
    style.font = "inherit";
    style.pointerEvents = "auto";
  }}

  function attachHover(button) {{
    button.addEventListener("mouseenter", function () {{
      button.style.background = "rgba(32, 36, 44, 0.94)";
      button.style.borderColor = "rgba(237, 242, 247, 0.3)";
    }});
    button.addEventListener("mouseleave", function () {{
      button.style.background = "rgba(23, 26, 32, 0.82)";
      button.style.borderColor = "rgba(237, 242, 247, 0.18)";
    }});
  }}

  function createSvg(paths) {{
    var svg = document.createElementNS(svgNamespace, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("aria-hidden", "true");
    svg.style.width = "22px";
    svg.style.height = "22px";
    svg.style.fill = "none";
    svg.style.stroke = "currentColor";
    svg.style.strokeWidth = "2.4";
    svg.style.strokeLinecap = "round";
    svg.style.strokeLinejoin = "round";

    paths.forEach(function (pathValue) {{
      var path = document.createElementNS(svgNamespace, "path");
      path.setAttribute("d", pathValue);
      svg.appendChild(path);
    }});

    return svg;
  }}

  function createButton(id, label, side, paths, compact, onClick) {{
    var button = document.createElement("button");
    button.id = id;
    button.type = "button";
    button.setAttribute("aria-label", label);
    applyButtonStyle(button, side, compact);
    attachHover(button);
    button.appendChild(createSvg(paths));
    button.addEventListener("click", function (event) {{
      event.preventDefault();
      event.stopPropagation();
      onClick();
    }});
    return button;
  }}

  function getMountTarget() {{
    return document.body || document.documentElement;
  }}

  function ensureAuthControls() {{
    if (!document.documentElement) {{
      return;
    }}

    var mountTarget = getMountTarget();
    var hasThemeBackButton = Boolean(document.querySelector(".mira-auth-back, .mira-auth-nav"));
    var shouldShowBackButton = isKeycloakPage();

    if (!shouldShowBackButton || hasThemeBackButton) {{
      var existingBackButton = document.getElementById(backButtonId);
      if (existingBackButton) {{
        existingBackButton.remove();
      }}
    }} else if (!document.getElementById(backButtonId)) {{
      mountTarget.appendChild(createButton(
        backButtonId,
        "Zurueck",
        "left",
        ["M15 18l-6-6 6-6"],
        false,
        function () {{
          if (window.history.length > 1) {{
            window.history.back();
            return;
          }}

          closeModal();
        }}
      ));
    }}

    if (!document.getElementById(closeButtonId)) {{
      mountTarget.appendChild(createButton(
        closeButtonId,
        "Schliessen",
        "right",
        ["M18 6 6 18", "m6 6 12 12"],
        true,
        closeModal
      ));
    }}
  }}

  if (document.readyState === "loading") {{
    document.addEventListener("DOMContentLoaded", ensureAuthControls, {{ once: true }});
  }} else {{
    ensureAuthControls();
  }}

  window.addEventListener("pageshow", ensureAuthControls);
  window.setTimeout(ensureAuthControls, 50);
  window.setTimeout(ensureAuthControls, 250);
}})();
"###
    ))
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
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                disable_webview_hardware_acceleration(&window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            client_config,
            game_client_status,
            launcher_status,
            launch_game,
            start_oauth_window,
            stop_game_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "linux")]
fn disable_webview_hardware_acceleration(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|webview| {
        use webkit2gtk::{HardwareAccelerationPolicy, SettingsExt, WebViewExt};

        if let Some(settings) = webview.inner().settings() {
            settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Never);
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn disable_webview_hardware_acceleration(_window: &tauri::WebviewWindow) {}
