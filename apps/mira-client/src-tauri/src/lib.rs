use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

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
const OAUTH_MODAL_MARGIN: f64 = 84.0;
const OAUTH_MODAL_WIDTH_RATIO: f64 = 0.62;
const OAUTH_MODAL_HEIGHT_RATIO: f64 = 0.66;
const OAUTH_MODAL_FALLBACK_WIDTH: f64 = 960.0;
const OAUTH_MODAL_FALLBACK_HEIGHT: f64 = 640.0;
const OAUTH_MODAL_MAX_WIDTH: f64 = 1040.0;
const OAUTH_MODAL_MAX_HEIGHT: f64 = 680.0;
const OAUTH_MODAL_MIN_WIDTH: f64 = 720.0;
const OAUTH_MODAL_MIN_HEIGHT: f64 = 520.0;
static OAUTH_WINDOW_COUNTER: AtomicU64 = AtomicU64::new(1);

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
    no_shared_auth: bool,
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
    #[serde(default)]
    clear_session_before_login: bool,
    id_token_hint: Option<String>,
    #[serde(default = "default_oauth_window_visible")]
    visible: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthWindowResponse {
    modal: bool,
    redirect_uri: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthCallbackPayload {
    url: String,
}

#[derive(Clone)]
struct OAuthTheme {
    accent_color: Option<String>,
    font_color: Option<String>,
}

fn default_oauth_window_visible() -> bool {
    true
}

fn oauth_window_label(visible: bool) -> String {
    if visible {
        return "mira-oauth".to_string();
    }

    let id = OAUTH_WINDOW_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("mira-oauth-logout-{id}")
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
    let launch_stage = launch_stage_for_base_url(&request.matchmaking_api_base_url);
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
        .arg(&request.server_control_base_url)
        .arg("--stage")
        .arg(launch_stage);

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
fn start_oauth_window(
    app: tauri::AppHandle,
    request: OAuthWindowRequest,
) -> Result<OAuthWindowResponse, String> {
    let auth_url_text = request.auth_url.trim().to_string();
    let auth_url = auth_url_text
        .parse()
        .map_err(|error| format!("OAuth-URL ist ungueltig: {error}"))?;
    let redirect_uri = request.redirect_uri.trim().to_string();

    if redirect_uri.is_empty() {
        return Err("OAuth-Redirect-URI fehlt.".to_string());
    }

    if cfg!(windows) && request.visible {
        let redirect_uri = start_windows_browser_oauth(app, auth_url, &request)?;
        return Ok(OAuthWindowResponse {
            modal: false,
            redirect_uri: Some(redirect_uri),
        });
    }

    if cfg!(windows) && !request.visible {
        let redirect_uri = start_windows_browser_logout(app, auth_url)?;
        return Ok(OAuthWindowResponse {
            modal: false,
            redirect_uri: Some(redirect_uri),
        });
    }

    let window_label = oauth_window_label(request.visible);

    if request.visible
        && let Some(existing_window) = app.get_webview_window(&window_label)
    {
        existing_window
            .close()
            .map_err(|error| format!("OAuth-Fenster konnte nicht ersetzt werden: {error}"))?;
    }

    let app_for_navigation = app.clone();
    let window_label_for_navigation = window_label.clone();
    let redirect_uri_for_navigation = redirect_uri.clone();
    let oauth_theme = oauth_theme_from_url(&auth_url);
    let auth_url_for_navigation = auth_url.clone();
    let oauth_window_url = if request.visible {
        let start_url = if request.clear_session_before_login {
            let client_id = auth_url
                .query_pairs()
                .find_map(|(key, value)| (key == "client_id").then(|| value.into_owned()));
            windows_browser_keycloak_logout_url(
                &auth_url,
                &oauth_start_redirect_uri(&redirect_uri),
                client_id.as_deref(),
                request.id_token_hint.as_deref(),
            )
            .unwrap_or_else(|| auth_url.clone())
        } else {
            auth_url.clone()
        };

        oauth_loading_url(start_url.as_str(), &oauth_theme)
    } else {
        tauri::WebviewUrl::External(auth_url)
    };

    let use_native_oauth_window_frame = cfg!(windows);
    let mut modal_width = OAUTH_MODAL_FALLBACK_WIDTH;
    let mut modal_height = OAUTH_MODAL_FALLBACK_HEIGHT;
    let mut oauth_window_builder =
        tauri::WebviewWindowBuilder::new(&app, window_label.clone(), oauth_window_url)
            .title("Mira Login")
            .min_inner_size(OAUTH_MODAL_MIN_WIDTH, OAUTH_MODAL_MIN_HEIGHT)
            .max_inner_size(OAUTH_MODAL_MAX_WIDTH, OAUTH_MODAL_MAX_HEIGHT)
            .closable(true)
            .resizable(false)
            .decorations(use_native_oauth_window_frame)
            .skip_taskbar(!use_native_oauth_window_frame)
            .always_on_top(!use_native_oauth_window_frame)
            .visible(false)
            .on_navigation(move |url| {
                let target_url = url.as_str();

                if is_oauth_start_url(target_url, &redirect_uri_for_navigation) {
                    if let Some(oauth_window) =
                        app_for_navigation.get_webview_window(&window_label_for_navigation)
                    {
                        let _ = oauth_window.show();
                        let _ = oauth_window.set_focus();
                        let _ = oauth_window.navigate(auth_url_for_navigation.clone());
                    }

                    return false;
                }

                if is_oauth_redirect_url(target_url, &redirect_uri_for_navigation) {
                    let _ = app_for_navigation.emit(
                        "mira-oauth-callback",
                        OAuthCallbackPayload {
                            url: target_url.to_string(),
                        },
                    );

                    if let Some(oauth_window) =
                        app_for_navigation.get_webview_window(&window_label_for_navigation)
                    {
                        let _ = oauth_window.close();
                    }

                    return false;
                }

                true
            });

    if !cfg!(windows) {
        let oauth_init_script = oauth_window_init_script(
            &redirect_uri,
            oauth_theme.clone(),
            request.clear_session_before_login,
        )
        .map_err(|error| format!("OAuth-Fenster konnte nicht vorbereitet werden: {error}"))?;
        oauth_window_builder = oauth_window_builder.initialization_script(oauth_init_script);
    }

    if let Some(main_window) = app.get_webview_window("main") {
        let geometry = oauth_modal_geometry(&main_window)?;
        modal_width = geometry.width;
        modal_height = geometry.height;

        if use_native_oauth_window_frame {
            oauth_window_builder = oauth_window_builder.position(geometry.x, geometry.y);
        } else {
            oauth_window_builder = oauth_window_builder
                .parent(&main_window)
                .map_err(|error| {
                    format!("OAuth-Modal konnte nicht an das Main-Window gebunden werden: {error}")
                })?
                .position(geometry.x, geometry.y);
        }
    } else {
        oauth_window_builder = oauth_window_builder.center();
    }

    let oauth_window = oauth_window_builder
        .inner_size(modal_width, modal_height)
        .build()
        .map_err(|error| format!("OAuth-Fenster konnte nicht geoeffnet werden: {error}"))?;

    if request.visible {
        oauth_window
            .show()
            .map_err(|error| format!("OAuth-Fenster konnte nicht angezeigt werden: {error}"))?;
        oauth_window
            .set_focus()
            .map_err(|error| format!("OAuth-Fenster konnte nicht fokussiert werden: {error}"))?;
    }

    let app_for_close = app.clone();
    oauth_window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let _ = app_for_close.emit("mira-oauth-closed", ());
        }
    });

    Ok(OAuthWindowResponse {
        modal: request.visible && !cfg!(windows),
        redirect_uri: None,
    })
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
    let available_width = (main_width - (OAUTH_MODAL_MARGIN * 2.0)).max(OAUTH_MODAL_MIN_WIDTH);
    let available_height = (main_height - (OAUTH_MODAL_MARGIN * 2.0)).max(OAUTH_MODAL_MIN_HEIGHT);
    let modal_width = (main_width * OAUTH_MODAL_WIDTH_RATIO)
        .max(OAUTH_MODAL_MIN_WIDTH)
        .min(OAUTH_MODAL_MAX_WIDTH)
        .min(available_width);
    let modal_height = (main_height * OAUTH_MODAL_HEIGHT_RATIO)
        .max(OAUTH_MODAL_MIN_HEIGHT)
        .min(OAUTH_MODAL_MAX_HEIGHT)
        .min(available_height);

    let min_x = main_x + OAUTH_MODAL_MARGIN.min(main_width / 8.0);
    let min_y = main_y + OAUTH_MODAL_MARGIN.min(main_height / 8.0);
    let max_x = main_x + main_width - modal_width - OAUTH_MODAL_MARGIN.min(main_width / 8.0);
    let max_y = main_y + main_height - modal_height - OAUTH_MODAL_MARGIN.min(main_height / 8.0);
    let centered_x = main_x + ((main_width - modal_width) / 2.0);
    let centered_y = main_y + ((main_height - modal_height) / 2.0);
    let x = centered_x.clamp(min_x, max_x.max(min_x));
    let y = centered_y.clamp(min_y, max_y.max(min_y));

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

fn oauth_start_redirect_uri(redirect_uri: &str) -> String {
    format!("{}mira-oauth-start", redirect_uri)
}

fn is_oauth_start_url(target_url: &str, redirect_uri: &str) -> bool {
    let start_uri = oauth_start_redirect_uri(redirect_uri);

    target_url == start_uri
        || target_url
            .strip_prefix(&start_uri)
            .is_some_and(|rest| rest.starts_with('?') || rest.starts_with('#'))
}

fn oauth_loading_url(auth_url: &str, theme: &OAuthTheme) -> tauri::WebviewUrl {
    let mut query = format!("authUrl={}", encode_url_component(auth_url));

    if let Some(accent_color) = theme.accent_color.as_deref() {
        query.push_str("&accent=");
        query.push_str(&encode_url_component(accent_color));
    }

    if let Some(font_color) = theme.font_color.as_deref() {
        let font_color_name = if font_color == "#ffffff" {
            "white"
        } else {
            "black"
        };
        query.push_str("&fontColor=");
        query.push_str(font_color_name);
    }

    tauri::WebviewUrl::App(format!("oauth-loading.html?{query}").into())
}

fn start_windows_browser_oauth(
    app: tauri::AppHandle,
    mut auth_url: tauri::Url,
    request: &OAuthWindowRequest,
) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("OAuth-Callback konnte nicht gestartet werden: {error}"))?;
    let callback_address = listener
        .local_addr()
        .map_err(|error| format!("OAuth-Callback-Adresse konnte nicht gelesen werden: {error}"))?;
    let redirect_uri = format!("http://{callback_address}/");

    let mut query_pairs = auth_url
        .query_pairs()
        .filter(|(key, _)| key != "redirect_uri" && key != "prompt" && key != "max_age")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let is_discord_oauth = query_pairs
        .iter()
        .any(|(key, value)| key == "kc_idp_hint" && value == "discord");
    let client_id = query_pairs
        .iter()
        .find_map(|(key, value)| (key == "client_id").then(|| value.clone()));

    query_pairs.push(("redirect_uri".to_string(), redirect_uri.clone()));

    if !is_discord_oauth {
        query_pairs.push(("prompt".to_string(), "login select_account".to_string()));
        query_pairs.push(("max_age".to_string(), "0".to_string()));
    }

    auth_url.query_pairs_mut().clear().extend_pairs(query_pairs);

    let post_logout_redirect_uri = format!("{}mira-oauth-start", redirect_uri);
    let browser_start_url = if request.clear_session_before_login {
        windows_browser_keycloak_logout_url(
            &auth_url,
            &post_logout_redirect_uri,
            client_id.as_deref(),
            request.id_token_hint.as_deref(),
        )
        .unwrap_or_else(|| auth_url.clone())
    } else {
        auth_url.clone()
    };
    let auth_url_for_redirect = auth_url.to_string();
    let app_for_callback = app.clone();
    let redirect_uri_for_callback = redirect_uri.clone();

    thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let deadline = Instant::now() + Duration::from_secs(180);

        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    match read_windows_browser_oauth_request(
                        &mut stream,
                        &redirect_uri_for_callback,
                    ) {
                        WindowsBrowserOAuthRequest::StartLogin => {
                            let _ = write_windows_browser_oauth_redirect(
                                &mut stream,
                                &auth_url_for_redirect,
                            );
                        }
                        WindowsBrowserOAuthRequest::Callback(callback_url) => {
                            let _ = write_windows_browser_oauth_response(&mut stream);
                            let _ = app_for_callback.emit(
                                "mira-oauth-callback",
                                OAuthCallbackPayload { url: callback_url },
                            );
                            break;
                        }
                        WindowsBrowserOAuthRequest::Ignore => {
                            let _ = write_windows_browser_oauth_ignored_response(&mut stream);
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    });

    app.opener()
        .open_url(browser_start_url.as_str(), None::<&str>)
        .map_err(|error| {
            format!("OAuth-Login konnte nicht im Standardbrowser geoeffnet werden: {error}")
        })?;

    Ok(redirect_uri)
}

fn start_windows_browser_logout(
    app: tauri::AppHandle,
    mut logout_url: tauri::Url,
) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("OAuth-Logout-Callback konnte nicht gestartet werden: {error}"))?;
    let callback_address = listener.local_addr().map_err(|error| {
        format!("OAuth-Logout-Callback-Adresse konnte nicht gelesen werden: {error}")
    })?;
    let redirect_uri = format!("http://{callback_address}/");

    let mut query_pairs = logout_url
        .query_pairs()
        .filter(|(key, _)| key != "post_logout_redirect_uri")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    query_pairs.push(("post_logout_redirect_uri".to_string(), redirect_uri.clone()));
    logout_url
        .query_pairs_mut()
        .clear()
        .extend_pairs(query_pairs);

    let app_for_callback = app.clone();
    let redirect_uri_for_callback = redirect_uri.clone();

    thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let deadline = Instant::now() + Duration::from_secs(30);

        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    if is_windows_browser_logout_callback(&mut stream) {
                        let _ = write_windows_browser_oauth_response(&mut stream);
                        let _ = app_for_callback.emit(
                            "mira-oauth-callback",
                            OAuthCallbackPayload {
                                url: redirect_uri_for_callback.clone(),
                            },
                        );
                        break;
                    }

                    let _ = write_windows_browser_oauth_ignored_response(&mut stream);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    });

    app.opener()
        .open_url(logout_url.as_str(), None::<&str>)
        .map_err(|error| {
            format!("OAuth-Logout konnte nicht im Standardbrowser geoeffnet werden: {error}")
        })?;

    Ok(redirect_uri)
}

enum WindowsBrowserOAuthRequest {
    StartLogin,
    Callback(String),
    Ignore,
}

fn read_windows_browser_oauth_request(
    stream: &mut std::net::TcpStream,
    redirect_uri: &str,
) -> WindowsBrowserOAuthRequest {
    let mut buffer = [0_u8; 4096];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(bytes_read) => bytes_read,
        Err(_) => return WindowsBrowserOAuthRequest::Ignore,
    };
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let Some(request_line) = request.lines().next() else {
        return WindowsBrowserOAuthRequest::Ignore;
    };
    let mut parts = request_line.split_whitespace();
    let Some(method) = parts.next() else {
        return WindowsBrowserOAuthRequest::Ignore;
    };
    let Some(target) = parts.next() else {
        return WindowsBrowserOAuthRequest::Ignore;
    };

    if method != "GET" {
        return WindowsBrowserOAuthRequest::Ignore;
    }

    if target.starts_with("/mira-oauth-start") {
        return WindowsBrowserOAuthRequest::StartLogin;
    }

    if !is_windows_browser_oauth_response_target(target) {
        return WindowsBrowserOAuthRequest::Ignore;
    }

    WindowsBrowserOAuthRequest::Callback(format!(
        "{}{}",
        redirect_uri.trim_end_matches('/'),
        target
    ))
}

fn windows_browser_keycloak_logout_url(
    auth_url: &tauri::Url,
    post_logout_redirect_uri: &str,
    client_id: Option<&str>,
    id_token_hint: Option<&str>,
) -> Option<tauri::Url> {
    let logout_path = auth_url.path().strip_suffix("/auth")?.to_string() + "/logout";
    let mut logout_url = auth_url.clone();
    logout_url.set_path(&logout_path);
    logout_url.set_query(None);

    {
        let mut query = logout_url.query_pairs_mut();

        if let Some(client_id) = client_id {
            query.append_pair("client_id", client_id);
        }

        if let Some(id_token_hint) = id_token_hint {
            query.append_pair("id_token_hint", id_token_hint);
        }

        query.append_pair("post_logout_redirect_uri", post_logout_redirect_uri);
    }

    Some(logout_url)
}

fn is_windows_browser_oauth_response_target(target: &str) -> bool {
    target.contains("code=") || target.contains("error=") || target.contains("error_description=")
}

fn is_windows_browser_logout_callback(stream: &mut std::net::TcpStream) -> bool {
    let mut buffer = [0_u8; 4096];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(bytes_read) => bytes_read,
        Err(_) => return false,
    };
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let Some(request_line) = request.lines().next() else {
        return false;
    };
    let mut parts = request_line.split_whitespace();
    let Some(method) = parts.next() else {
        return false;
    };
    let Some(target) = parts.next() else {
        return false;
    };

    method == "GET" && (target == "/" || target.starts_with("/?"))
}

fn write_windows_browser_oauth_ignored_response(
    stream: &mut std::net::TcpStream,
) -> std::io::Result<()> {
    let body = "Not an OAuth callback.";
    let response = format!(
        "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    stream.write_all(response.as_bytes())
}

fn write_windows_browser_oauth_redirect(
    stream: &mut std::net::TcpStream,
    target_url: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        target_url
    );

    stream.write_all(response.as_bytes())
}

fn write_windows_browser_oauth_response(stream: &mut std::net::TcpStream) -> std::io::Result<()> {
    let body = r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><title>Mira Login</title><style>html,body{height:100%;margin:0;background:#101216;color:#edf2f7;font:16px system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}body{display:grid;place-items:center}.panel{display:grid;gap:12px;text-align:center}.mark{width:54px;height:54px;border-radius:10px;background:#f2c45b;color:#101216;display:grid;place-items:center;font-weight:800;font-size:28px;margin:auto}p{margin:0;color:#aeb7c5}</style></head><body><main class="panel"><div class="mark">M</div><h1>Login abgeschlossen</h1><p>Du kannst dieses Browserfenster jetzt schliessen.</p></main><script>(function(){function closeTab(){window.open("","_self");window.close()}window.setTimeout(closeTab,250);window.setTimeout(closeTab,700);window.setTimeout(function(){closeTab();document.body.innerHTML=""},1500)})();</script></body></html>"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    stream.write_all(response.as_bytes())
}

fn encode_url_component(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }

    encoded
}

fn oauth_theme_from_url(auth_url: &tauri::Url) -> OAuthTheme {
    let mut accent_color = None;
    let mut font_color = None;

    for (key, value) in auth_url.query_pairs() {
        match key.as_ref() {
            "accent" => {
                accent_color = normalize_oauth_accent_color(&value);
            }
            "fontColor" => {
                font_color = normalize_oauth_font_color(&value);
            }
            _ => {}
        }
    }

    OAuthTheme {
        accent_color,
        font_color,
    }
}

fn normalize_oauth_accent_color(value: &str) -> Option<String> {
    let normalized = value.trim().trim_start_matches('#').to_ascii_lowercase();

    if normalized.len() == 6
        && normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Some(format!("#{normalized}"));
    }

    None
}

fn normalize_oauth_font_color(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "white" => Some("#ffffff".to_string()),
        "black" => Some("#101216".to_string()),
        _ => None,
    }
}

fn oauth_window_init_script(
    redirect_uri: &str,
    theme: OAuthTheme,
    auto_submit_logout: bool,
) -> Result<String, serde_json::Error> {
    let redirect_uri = serde_json::to_string(redirect_uri)?;
    let accent_color = serde_json::to_string(&theme.accent_color)?;
    let font_color = serde_json::to_string(&theme.font_color)?;
    let auto_submit_logout = serde_json::to_string(&auto_submit_logout)?;

    Ok(r###"
(function () {
  var redirectUri = __MIRA_REDIRECT_URI__;
  var accentColor = __MIRA_ACCENT_COLOR__;
  var accentForegroundColor = __MIRA_ACCENT_FOREGROUND_COLOR__;
  var autoSubmitLogout = __MIRA_AUTO_SUBMIT_LOGOUT__;
  var backButtonId = "mira-oauth-back-button";
  var closeButtonId = "mira-oauth-close-button";
  var themeStyleId = "mira-oauth-theme-style";
  var svgNamespace = "http://www.w3.org/2000/svg";

  function closeModal() {
    window.location.href = redirectUri;
  }

  function isKeycloakPage() {
    return /\/realms\/[^/]+\/(protocol\/openid-connect\/auth|login-actions|broker)(\/|$)/.test(
      window.location.pathname
    );
  }

  function isKeycloakLogoutPage() {
    return /\/realms\/[^/]+\/protocol\/openid-connect\/logout(\/|$)/.test(
      window.location.pathname
    );
  }

  function autoSubmitKeycloakLogout() {
    if (!autoSubmitLogout || window.__miraLogoutSubmitted || !isKeycloakLogoutPage()) {
      return;
    }

    var form = document.querySelector("form");
    var submit = document.querySelector(
      "button[type='submit'],input[type='submit'],#kc-logout,#kc-form-buttons input,.pf-c-button.pf-m-primary,.pf-v5-c-button.pf-m-primary"
    );

    if (!form && !submit) {
      return;
    }

    window.__miraLogoutSubmitted = true;

    if (submit && typeof submit.click === "function") {
      submit.click();
      return;
    }

    if (form && typeof form.submit === "function") {
      form.submit();
    }
  }

  function redirectOAuthError(error) {
    if (window.__miraOAuthErrorRedirected) {
      return;
    }

    window.__miraOAuthErrorRedirected = true;

    try {
      var callbackUrl = new URL(redirectUri);
      callbackUrl.searchParams.set("error", error);
      window.location.href = callbackUrl.toString();
    } catch (errorCaught) {
      window.location.href = redirectUri + (redirectUri.indexOf("?") === -1 ? "?" : "&") + "error=" + encodeURIComponent(error);
    }
  }

  function detectAccountProviderConflict() {
    if (!document.body || window.__miraOAuthErrorRedirected) {
      return;
    }

    var text = document.body.textContent.replace(/\s+/g, " ").trim().toLowerCase();
    var hasEnglishConflict =
      text.indexOf("account already exists") !== -1 ||
      (
        text.indexOf("already exists") !== -1 &&
        (text.indexOf("add to existing account") !== -1 || text.indexOf("review profile") !== -1)
      );
    var hasGermanConflict =
      text.indexOf("konto existiert bereits") !== -1 ||
      (
        text.indexOf("existiert bereits") !== -1 &&
        (text.indexOf("bestehenden") !== -1 || text.indexOf("profil") !== -1)
      );

    if (hasEnglishConflict || hasGermanConflict) {
      redirectOAuthError("oauth_email_provider_conflict");
    }
  }

  function applyTheme() {
    if (!accentColor || !document.documentElement) {
      return;
    }

    var foregroundColor = accentForegroundColor || "#101216";
    var rootStyle = document.documentElement.style;
    rootStyle.setProperty("--mira-auth-accent", accentColor);
    rootStyle.setProperty("--mira-auth-accent-foreground", foregroundColor);
    rootStyle.setProperty("--accent-color", accentColor);
    rootStyle.setProperty("--accent-foreground-color", foregroundColor);
    rootStyle.setProperty("--pf-global--primary-color--100", accentColor);
    rootStyle.setProperty("--pf-global--primary-color--200", accentColor);
    rootStyle.setProperty("--pf-v5-global--primary-color--100", accentColor);
    rootStyle.setProperty("--pf-v5-global--primary-color--200", accentColor);

    var style = document.getElementById(themeStyleId);
    if (!style) {
      style = document.createElement("style");
      style.id = themeStyleId;
      (document.head || document.documentElement).appendChild(style);
    }

    style.textContent = [
      ":root{--mira-auth-accent:" + accentColor + ";--mira-auth-accent-foreground:" + foregroundColor + ";}",
      "html,body{background:#101216!important;}",
      "@keyframes mira-oauth-spin{to{transform:rotate(360deg);}}",
      "#mira-oauth-loader{position:fixed;inset:0;z-index:2147483646;display:grid;place-items:center;background:#101216;}",
      "#mira-oauth-loader::before{content:'';width:46px;height:46px;border-radius:999px;border:4px solid rgba(237,242,247,.16);border-top-color:var(--mira-auth-accent);box-shadow:0 0 22px color-mix(in srgb,var(--mira-auth-accent) 45%,transparent);animation:mira-oauth-spin .8s linear infinite;}",
      ".mira-auth-logo,.mira-auth-logo-mark,.mira-auth-brand,.mira-auth-brand-mark,.brand-mark,[class*='brand-mark'],[class*='logo-mark'],[class*='auth-logo'],[class*='mira-logo'],[class*='mira-brand']{background:var(--mira-auth-accent)!important;color:var(--mira-auth-accent-foreground)!important;border-color:var(--mira-auth-accent)!important;}",
      "input[type='submit'],button[type='submit'],#kc-form-buttons input,.pf-c-button.pf-m-primary,.pf-v5-c-button.pf-m-primary{background:var(--mira-auth-accent)!important;border-color:var(--mira-auth-accent)!important;color:var(--mira-auth-accent-foreground)!important;}",
      "a,.mira-auth-link,#kc-current-locale-link{color:var(--mira-auth-accent)!important;}",
      "input:focus,textarea:focus{border-color:var(--mira-auth-accent)!important;box-shadow:0 0 0 1px var(--mira-auth-accent)!important;}"
    ].join("\n");

    Array.prototype.forEach.call(document.querySelectorAll("body *"), function (element) {
      if (element.children.length > 1 || element.textContent.trim() !== "M") {
        return;
      }

      var rect = element.getBoundingClientRect();
      var hasBadgeSize = rect.width <= 96 && rect.height <= 96;
      var className = typeof element.className === "string" ? element.className : "";
      var looksLikeBrand = /brand|logo|mark|mira/i.test(className);

      if (!hasBadgeSize && !looksLikeBrand) {
        return;
      }

      element.style.setProperty("background", accentColor, "important");
      element.style.setProperty("background-color", accentColor, "important");
      element.style.setProperty("border-color", accentColor, "important");
      element.style.setProperty("color", foregroundColor, "important");
    });
  }

  function ensureLoader() {
    if (!document.documentElement || document.getElementById("mira-oauth-loader")) {
      return;
    }

    var loader = document.createElement("div");
    loader.id = "mira-oauth-loader";
    loader.setAttribute("aria-hidden", "true");
    (document.body || document.documentElement).appendChild(loader);
  }

  function removeLoader() {
    var loader = document.getElementById("mira-oauth-loader");
    if (loader) {
      loader.remove();
    }
  }

  function applyButtonStyle(button, side, compact) {
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
  }

  function attachHover(button) {
    button.addEventListener("mouseenter", function () {
      button.style.background = "rgba(32, 36, 44, 0.94)";
      button.style.borderColor = "rgba(237, 242, 247, 0.3)";
    });
    button.addEventListener("mouseleave", function () {
      button.style.background = "rgba(23, 26, 32, 0.82)";
      button.style.borderColor = "rgba(237, 242, 247, 0.18)";
    });
  }

  function createSvg(paths) {
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

    paths.forEach(function (pathValue) {
      var path = document.createElementNS(svgNamespace, "path");
      path.setAttribute("d", pathValue);
      svg.appendChild(path);
    });

    return svg;
  }

  function createButton(id, label, side, paths, compact, onClick) {
    var button = document.createElement("button");
    button.id = id;
    button.type = "button";
    button.setAttribute("aria-label", label);
    applyButtonStyle(button, side, compact);
    attachHover(button);
    button.appendChild(createSvg(paths));
    button.addEventListener("click", function (event) {
      event.preventDefault();
      event.stopPropagation();
      onClick();
    });
    return button;
  }

  function getMountTarget() {
    return document.body || document.documentElement;
  }

  function ensureAuthControls() {
    applyTheme();
    detectAccountProviderConflict();
    autoSubmitKeycloakLogout();

    if (!document.documentElement) {
      return;
    }

    removeLoader();

    var mountTarget = getMountTarget();
    var hasThemeBackButton = Boolean(document.querySelector(".mira-auth-back, .mira-auth-nav"));
    var shouldShowBackButton = isKeycloakPage();

    if (!shouldShowBackButton || hasThemeBackButton) {
      var existingBackButton = document.getElementById(backButtonId);
      if (existingBackButton) {
        existingBackButton.remove();
      }
    } else if (!document.getElementById(backButtonId)) {
      mountTarget.appendChild(createButton(
        backButtonId,
        "Zurueck",
        "left",
        ["M15 18l-6-6 6-6"],
        false,
        function () {
          if (window.history.length > 1) {
            window.history.back();
            return;
          }

          closeModal();
        }
      ));
    }

    if (!document.getElementById(closeButtonId)) {
      mountTarget.appendChild(createButton(
        closeButtonId,
        "Schliessen",
        "right",
        ["M18 6 6 18", "m6 6 12 12"],
        true,
        closeModal
      ));
    }
  }

  applyTheme();
  ensureLoader();

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", ensureAuthControls, { once: true });
  } else {
    ensureAuthControls();
  }

  window.addEventListener("pageshow", ensureAuthControls);
  window.setTimeout(ensureAuthControls, 50);
  window.setTimeout(ensureAuthControls, 250);
  window.setTimeout(ensureAuthControls, 750);
  window.setInterval(detectAccountProviderConflict, 500);
})();
"###
    .replace("__MIRA_REDIRECT_URI__", &redirect_uri)
    .replace("__MIRA_ACCENT_COLOR__", &accent_color)
    .replace("__MIRA_ACCENT_FOREGROUND_COLOR__", &font_color)
    .replace("__MIRA_AUTO_SUBMIT_LOGOUT__", &auto_submit_logout))
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
            no_shared_auth: env_flag_enabled("MIRA_CLIENT_NO_SHARED_AUTH"),
        }
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn normalize_base_url(value: Option<&str>, default_value: &str) -> String {
    value_or_default(value, default_value)
        .trim_end_matches('/')
        .to_string()
}

fn launch_stage_for_base_url(base_url: &str) -> &'static str {
    tauri::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(localhost_matches_stage))
        .unwrap_or("Dev")
}

fn localhost_matches_stage(host: &str) -> &'static str {
    let host = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();

    if host == "localhost"
        || host == "::1"
        || host == "0:0:0:0:0:0:0:1"
        || host.starts_with("127.")
    {
        "Local"
    } else {
        "Dev"
    }
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
            start_oauth_window,
            stop_game_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
