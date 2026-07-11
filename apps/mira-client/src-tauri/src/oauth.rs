use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Position, Size};
use tauri_plugin_opener::OpenerExt;

const OAUTH_MODAL_MARGIN: f64 = 84.0;
const OAUTH_MODAL_WIDTH_RATIO: f64 = 0.62;
const OAUTH_MODAL_HEIGHT_RATIO: f64 = 0.66;
const OAUTH_MODAL_FALLBACK_WIDTH: f64 = 960.0;
const OAUTH_MODAL_FALLBACK_HEIGHT: f64 = 640.0;
const OAUTH_MODAL_MAX_WIDTH: f64 = 1040.0;
const OAUTH_MODAL_MAX_HEIGHT: f64 = 680.0;
const OAUTH_MODAL_MIN_WIDTH: f64 = 720.0;
const OAUTH_MODAL_MIN_HEIGHT: f64 = 520.0;
const OAUTH_PROVIDER_FAILED_ERROR: &str = "oauth_provider_failed";
static OAUTH_WINDOW_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthWindowRequest {
    auth_url: String,
    redirect_uri: String,
    #[serde(default)]
    clear_session_before_login: bool,
    id_token_hint: Option<String>,
    #[serde(default)]
    password_reset: bool,
    #[serde(default = "default_oauth_window_visible")]
    visible: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthWindowResponse {
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
pub(crate) fn start_oauth_window(
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

    let oauth_theme = oauth_theme_from_url(&auth_url);
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
        tauri::WebviewUrl::External(auth_url.clone())
    };

    let app_for_navigation = app.clone();
    let window_label_for_navigation = window_label.clone();
    let redirect_uri_for_navigation = redirect_uri.clone();
    let auth_url_for_navigation = auth_url.clone();
    let password_reset_for_navigation = request.password_reset;
    let app_for_page_load = app.clone();
    let window_label_for_page_load = window_label.clone();
    let redirect_uri_for_page_load = redirect_uri.clone();
    let localhost_connection_refused_script =
        localhost_connection_refused_close_script(&redirect_uri).map_err(|error| {
            format!("OAuth-Fehlerseiten-Erkennung konnte nicht vorbereitet werden: {error}")
        })?;
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

                if password_reset_for_navigation && is_keycloak_password_reset_login_url(target_url)
                {
                    let _ = app_for_navigation.emit(
                        "mira-oauth-callback",
                        OAuthCallbackPayload {
                            url: password_reset_sent_redirect_uri(&redirect_uri_for_navigation),
                        },
                    );

                    if let Some(oauth_window) =
                        app_for_navigation.get_webview_window(&window_label_for_navigation)
                    {
                        let _ = oauth_window.close();
                    }

                    return false;
                }

                if let Some(callback_url) =
                    oauth_callback_url_from_terminal_url(target_url, &redirect_uri_for_navigation)
                {
                    let _ = app_for_navigation.emit(
                        "mira-oauth-callback",
                        OAuthCallbackPayload { url: callback_url },
                    );

                    if let Some(oauth_window) =
                        app_for_navigation.get_webview_window(&window_label_for_navigation)
                    {
                        let _ = oauth_window.close();
                    }

                    return false;
                }

                true
            })
            .on_page_load(move |oauth_window, payload| {
                let target_url = payload.url().as_str();

                if let Some(callback_url) =
                    oauth_callback_url_from_terminal_url(target_url, &redirect_uri_for_page_load)
                {
                    let _ = app_for_page_load.emit(
                        "mira-oauth-callback",
                        OAuthCallbackPayload { url: callback_url },
                    );

                    if let Some(oauth_window) =
                        app_for_page_load.get_webview_window(&window_label_for_page_load)
                    {
                        let _ = oauth_window.close();
                    }

                    return;
                }

                if payload.event() == tauri::webview::PageLoadEvent::Finished {
                    let _ = oauth_window.eval(localhost_connection_refused_script.clone());
                }
            });

    if !cfg!(windows) {
        let oauth_init_script = oauth_window_init_script(
            &redirect_uri,
            oauth_theme.clone(),
            request.clear_session_before_login,
            request.password_reset,
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

    #[cfg(target_os = "linux")]
    install_oauth_load_failed_handler(
        &oauth_window,
        app.clone(),
        window_label.clone(),
        redirect_uri.clone(),
    )?;

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

    if request.visible {
        if let Some(main_window) = app.get_webview_window("main") {
            let app_for_main_window_event = app.clone();
            let window_label_for_main_window_event = window_label.clone();
            main_window.on_window_event(move |event| {
                if matches!(
                    event,
                    tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
                ) {
                    let _ = sync_oauth_modal_to_main(
                        &app_for_main_window_event,
                        &window_label_for_main_window_event,
                    );
                }
            });
        }
    }

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

fn sync_oauth_modal_to_main(app: &tauri::AppHandle, window_label: &str) -> Result<(), String> {
    let Some(main_window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let Some(oauth_window) = app.get_webview_window(window_label) else {
        return Ok(());
    };

    let geometry = oauth_modal_geometry(&main_window)?;
    oauth_window
        .set_position(Position::Logical(LogicalPosition {
            x: geometry.x,
            y: geometry.y,
        }))
        .map_err(|error| format!("OAuth-Modal konnte nicht verschoben werden: {error}"))?;
    oauth_window
        .set_size(Size::Logical(LogicalSize {
            width: geometry.width,
            height: geometry.height,
        }))
        .map_err(|error| format!("OAuth-Modal konnte nicht skaliert werden: {error}"))?;

    Ok(())
}

fn is_oauth_redirect_url(target_url: &str, redirect_uri: &str) -> bool {
    target_url == redirect_uri
        || target_url
            .strip_prefix(redirect_uri)
            .is_some_and(|rest| rest.starts_with('?') || rest.starts_with('#'))
}

fn oauth_callback_url_from_terminal_url(target_url: &str, redirect_uri: &str) -> Option<String> {
    if is_oauth_redirect_url(target_url, redirect_uri) {
        return Some(target_url.to_string());
    }

    if is_mira_public_oauth_error_url(target_url) {
        return Some(oauth_error_callback_url(
            redirect_uri,
            oauth_error_from_url(target_url).as_deref(),
        ));
    }

    None
}

fn is_mira_public_oauth_error_url(target_url: &str) -> bool {
    let Ok(url) = target_url.parse::<tauri::Url>() else {
        return false;
    };

    let Some(host) = url.host_str().map(|host| host.to_ascii_lowercase()) else {
        return false;
    };

    let is_mira_public_host = matches!(
        host.as_str(),
        "tilt-us.com" | "www.tilt-us.com" | "mira.tilt-us.com"
    );

    if !is_mira_public_host {
        return false;
    }

    url.query_pairs().any(|(key, _)| {
        matches!(
            key.as_ref(),
            "error" | "error_description" | "kc_error" | "kc_error_message"
        )
    }) || url.path() == "/"
}

fn oauth_error_from_url(target_url: &str) -> Option<String> {
    let url = target_url.parse::<tauri::Url>().ok()?;

    for key in ["error_description", "error", "kc_error_message", "kc_error"] {
        if let Some(value) = url
            .query_pairs()
            .find_map(|(candidate, value)| (candidate == key).then(|| value.into_owned()))
            .filter(|value| !value.trim().is_empty())
        {
            return Some(normalize_oauth_error_value(&value));
        }
    }

    None
}

fn normalize_oauth_error_value(error: &str) -> String {
    let normalized = error.trim();
    let lower = normalized.to_ascii_lowercase();

    if lower.contains("account already exists")
        || lower.contains("already exists")
        || lower.contains("konto existiert")
        || lower.contains("existiert bereits")
        || lower.contains("same email")
        || lower.contains("same e-mail")
        || lower.contains("selben email")
        || lower.contains("selben e-mail")
    {
        return "oauth_email_provider_conflict".to_string();
    }

    if normalized == "1" || normalized.eq_ignore_ascii_case("true") {
        return OAUTH_PROVIDER_FAILED_ERROR.to_string();
    }

    normalized.to_string()
}

fn oauth_error_callback_url(redirect_uri: &str, error: Option<&str>) -> String {
    let separator = if redirect_uri.contains('?') { '&' } else { '?' };
    let error = error.unwrap_or(OAUTH_PROVIDER_FAILED_ERROR);

    format!(
        "{redirect_uri}{separator}error={}",
        encode_url_component(error)
    )
}

fn is_localhost_url(target_url: &str) -> bool {
    let Ok(url) = target_url.parse::<tauri::Url>() else {
        return false;
    };

    matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
}

fn is_connection_refused_error(error: &str) -> bool {
    error.to_ascii_lowercase().contains("connection refused")
}

#[cfg(target_os = "linux")]
fn install_oauth_load_failed_handler(
    oauth_window: &tauri::WebviewWindow,
    app: tauri::AppHandle,
    window_label: String,
    redirect_uri: String,
) -> Result<(), String> {
    oauth_window
        .with_webview(move |webview| {
            use webkit2gtk::WebViewExt;

            webview
                .inner()
                .connect_load_failed(move |_, _, failing_uri, error| {
                    if let Some(callback_url) =
                        oauth_callback_url_from_terminal_url(failing_uri, &redirect_uri)
                    {
                        let _ = app.emit(
                            "mira-oauth-callback",
                            OAuthCallbackPayload { url: callback_url },
                        );

                        if let Some(oauth_window) = app.get_webview_window(&window_label) {
                            let _ = oauth_window.close();
                        }

                        return true;
                    }

                    if is_localhost_url(failing_uri)
                        && is_connection_refused_error(&error.to_string())
                    {
                        if let Some(oauth_window) = app.get_webview_window(&window_label) {
                            let _ = oauth_window.close();
                        }

                        return true;
                    }

                    false
                });
        })
        .map_err(|error| format!("OAuth-Fehlerhandler konnte nicht installiert werden: {error}"))
}

fn is_keycloak_password_reset_login_url(target_url: &str) -> bool {
    let Ok(url) = target_url.parse::<tauri::Url>() else {
        return false;
    };
    let path = url.path();

    path.contains("/protocol/openid-connect/auth") || path.contains("/login-actions/authenticate")
}

fn password_reset_sent_redirect_uri(redirect_uri: &str) -> String {
    let separator = if redirect_uri.contains('?') { '&' } else { '?' };
    format!("{redirect_uri}{separator}mira_password_reset=sent")
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

    if !is_discord_oauth && !request.password_reset {
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
    let password_reset = request.password_reset;

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
                            let callback_url = if password_reset {
                                password_reset_sent_redirect_uri(&redirect_uri_for_callback)
                            } else {
                                callback_url
                            };
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

fn localhost_connection_refused_close_script(
    redirect_uri: &str,
) -> Result<String, serde_json::Error> {
    let redirect_uri = serde_json::to_string(redirect_uri)?;

    Ok(r###"
(function () {
  if (window.__miraLocalhostConnectionRefusedClosed || !document.body) {
    return;
  }

  var text = document.body.textContent.replace(/\s+/g, " ").trim().toLowerCase();
  var isLocalhostPage =
    window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1";
  var mentionsLocalhost = text.indexOf("localhost") !== -1 || text.indexOf("127.0.0.1") !== -1;
  var hasConnectionRefused = text.indexOf("connection refused") !== -1;

  if (!hasConnectionRefused || (!isLocalhostPage && !mentionsLocalhost)) {
    return;
  }

  window.__miraLocalhostConnectionRefusedClosed = true;
  window.location.href = __MIRA_REDIRECT_URI__;
}());
"###
    .replace("__MIRA_REDIRECT_URI__", &redirect_uri))
}

fn oauth_window_init_script(
    redirect_uri: &str,
    theme: OAuthTheme,
    auto_submit_logout: bool,
    password_reset: bool,
) -> Result<String, serde_json::Error> {
    let password_reset_redirect_uri =
        serde_json::to_string(&password_reset_sent_redirect_uri(redirect_uri))?;
    let redirect_uri = serde_json::to_string(redirect_uri)?;
    let accent_color = serde_json::to_string(&theme.accent_color)?;
    let font_color = serde_json::to_string(&theme.font_color)?;
    let auto_submit_logout = serde_json::to_string(&auto_submit_logout)?;
    let password_reset = serde_json::to_string(&password_reset)?;

    Ok(r###"
(function () {
  var redirectUri = __MIRA_REDIRECT_URI__;
  var passwordResetRedirectUri = __MIRA_PASSWORD_RESET_REDIRECT_URI__;
  var accentColor = __MIRA_ACCENT_COLOR__;
  var accentForegroundColor = __MIRA_ACCENT_FOREGROUND_COLOR__;
  var autoSubmitLogout = __MIRA_AUTO_SUBMIT_LOGOUT__;
  var passwordReset = __MIRA_PASSWORD_RESET__;
  var backButtonId = "mira-oauth-back-button";
  var closeButtonId = "mira-oauth-close-button";
  var passwordResetSubmittedKey = "mira-password-reset-submitted";
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

  function isKeycloakResetCredentialsPage() {
    return /\/realms\/[^/]+\/login-actions\/reset-credentials(\/|$)/.test(
      window.location.pathname
    );
  }

  function isPasswordResetSubmitted() {
    try {
      return sessionStorage.getItem(passwordResetSubmittedKey) === "1";
    } catch (error) {
      return Boolean(window.__miraPasswordResetSubmitted);
    }
  }

  function markPasswordResetSubmitted() {
    window.__miraPasswordResetSubmitted = true;

    try {
      sessionStorage.setItem(passwordResetSubmittedKey, "1");
    } catch (error) {
      // Ignore storage failures; the window flag still handles this document.
    }
  }

  function clearPasswordResetSubmitted() {
    window.__miraPasswordResetSubmitted = false;

    try {
      sessionStorage.removeItem(passwordResetSubmittedKey);
    } catch (error) {
      // Ignore storage failures.
    }
  }

  function redirectPasswordResetSent() {
    if (window.__miraPasswordResetCompleted) {
      return;
    }

    window.__miraPasswordResetCompleted = true;
    clearPasswordResetSubmitted();
    window.location.href = passwordResetRedirectUri;
  }

  function handlePasswordResetSubmit() {
    if (!passwordReset || !isKeycloakResetCredentialsPage()) {
      return;
    }

    markPasswordResetSubmitted();
    window.setTimeout(redirectPasswordResetSent, 1200);
  }

  function completePasswordResetRequest() {
    if (!passwordReset || window.__miraPasswordResetCompleted) {
      return;
    }

    if (isPasswordResetSubmitted() && isKeycloakPage() && !isKeycloakResetCredentialsPage()) {
      redirectPasswordResetSent();
      return;
    }

    if (!isKeycloakResetCredentialsPage()) {
      return;
    }

    var bodyText = document.body ? document.body.textContent.replace(/\s+/g, " ").trim().toLowerCase() : "";
    var hasEnglishSuccess =
      bodyText.indexOf("receive an email") !== -1 ||
      bodyText.indexOf("email shortly") !== -1 ||
      bodyText.indexOf("e-mail shortly") !== -1 ||
      bodyText.indexOf("check your email") !== -1 ||
      bodyText.indexOf("sent you an email") !== -1;
    var hasGermanSuccess =
      bodyText.indexOf("e-mail erhalten") !== -1 ||
      bodyText.indexOf("email erhalten") !== -1 ||
      bodyText.indexOf("prüfen sie") !== -1 ||
      bodyText.indexOf("pruefen sie") !== -1 ||
      bodyText.indexOf("posteingang") !== -1 ||
      bodyText.indexOf("weitere anweisungen") !== -1;

    if (!hasEnglishSuccess && !hasGermanSuccess) {
      return;
    }

    redirectPasswordResetSent();
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

  function detectOAuthErrorResponse() {
    if (window.__miraOAuthErrorRedirected) {
      return;
    }

    try {
      var currentUrl = new URL(window.location.href);
      var urlError =
        currentUrl.searchParams.get("error_description") ||
        currentUrl.searchParams.get("error") ||
        currentUrl.searchParams.get("kc_error_message") ||
        currentUrl.searchParams.get("kc_error");

      if (urlError) {
        redirectOAuthError(normalizeOAuthError(urlError));
        return;
      }
    } catch (errorCaught) {
      // URL parsing should not block body-based error detection.
    }

    if (!document.body) {
      return;
    }

    var text = document.body.textContent.replace(/\s+/g, " ").trim().toLowerCase();
    var hasCredentialsError =
      text.indexOf("invalid credentials") !== -1 ||
      text.indexOf("invalid username or password") !== -1 ||
      text.indexOf("incorrect username or password") !== -1 ||
      text.indexOf("wrong credentials") !== -1 ||
      text.indexOf("ungültige anmeldeinformationen") !== -1 ||
      text.indexOf("ungueltige anmeldeinformationen") !== -1 ||
      text.indexOf("benutzername oder passwort") !== -1 ||
      text.indexOf("zugangsdaten") !== -1;
    var hasProviderError =
      text.indexOf("unexpected error") !== -1 ||
      text.indexOf("identity provider") !== -1 ||
      text.indexOf("external identity provider") !== -1 ||
      text.indexOf("broker") !== -1 ||
      text.indexOf("unerwarteter fehler") !== -1 ||
      text.indexOf("identitätsanbieter") !== -1 ||
      text.indexOf("identitaetsanbieter") !== -1;

    if (hasCredentialsError) {
      redirectOAuthError("invalid_credentials");
      return;
    }

    if (hasProviderError && isKeycloakPage()) {
      redirectOAuthError("oauth_provider_failed");
    }
  }

  function normalizeOAuthError(error) {
    var normalized = String(error || "").trim();
    var lower = normalized.toLowerCase();

    if (
      lower.indexOf("account already exists") !== -1 ||
      lower.indexOf("already exists") !== -1 ||
      lower.indexOf("konto existiert") !== -1 ||
      lower.indexOf("existiert bereits") !== -1 ||
      lower.indexOf("same email") !== -1 ||
      lower.indexOf("same e-mail") !== -1 ||
      lower.indexOf("selben email") !== -1 ||
      lower.indexOf("selben e-mail") !== -1
    ) {
      return "oauth_email_provider_conflict";
    }

    if (normalized === "1" || lower === "true") {
      return "oauth_provider_failed";
    }

    return normalized || "oauth_provider_failed";
  }

  function closeLocalhostConnectionRefusedPage() {
    if (!document.body || window.__miraLocalhostConnectionRefusedClosed) {
      return;
    }

    var text = document.body.textContent.replace(/\s+/g, " ").trim().toLowerCase();
    var isLocalhostPage =
      window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1";
    var hasConnectionRefused =
      text.indexOf("could not connect to localhost") !== -1 &&
      text.indexOf("connection refused") !== -1;

    if (!isLocalhostPage || !hasConnectionRefused) {
      return;
    }

    window.__miraLocalhostConnectionRefusedClosed = true;
    closeModal();
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
    closeLocalhostConnectionRefusedPage();
    applyTheme();
    detectOAuthErrorResponse();
    detectAccountProviderConflict();
    completePasswordResetRequest();
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
  document.addEventListener("submit", handlePasswordResetSubmit, true);

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", ensureAuthControls, { once: true });
  } else {
    ensureAuthControls();
  }

  window.addEventListener("pageshow", ensureAuthControls);
  window.setTimeout(ensureAuthControls, 50);
  window.setTimeout(ensureAuthControls, 250);
  window.setTimeout(ensureAuthControls, 750);
  window.setInterval(detectOAuthErrorResponse, 500);
  window.setInterval(detectAccountProviderConflict, 500);
  window.setInterval(completePasswordResetRequest, 500);
})();
"###
    .replace("__MIRA_REDIRECT_URI__", &redirect_uri)
    .replace(
        "__MIRA_PASSWORD_RESET_REDIRECT_URI__",
        &password_reset_redirect_uri,
    )
    .replace("__MIRA_ACCENT_COLOR__", &accent_color)
    .replace("__MIRA_ACCENT_FOREGROUND_COLOR__", &font_color)
    .replace("__MIRA_AUTO_SUBMIT_LOGOUT__", &auto_submit_logout)
    .replace("__MIRA_PASSWORD_RESET__", &password_reset))
}
