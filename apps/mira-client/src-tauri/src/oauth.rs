use std::{
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(target_os = "linux")]
use std::process::Command;
use tauri::{Emitter, Manager};
#[cfg(not(target_os = "linux"))]
use tauri_plugin_opener::OpenerExt;

const KEYCLOAK_NATIVE_REDIRECT_URI: &str = "http://127.0.0.1";

static OAUTH_ATTEMPTS: LazyLock<OAuthAttemptRegistry> = LazyLock::new(OAuthAttemptRegistry::new);

struct NativeOAuthAttempt {
    attempt_id: u64,
    redirect_uri: String,
    listener: Option<TcpListener>,
}

struct OAuthAttemptRegistry {
    next_attempt_id: AtomicU64,
    active_attempt: Mutex<Option<NativeOAuthAttempt>>,
}

impl OAuthAttemptRegistry {
    const fn new() -> Self {
        Self {
            next_attempt_id: AtomicU64::new(1),
            active_attempt: Mutex::new(None),
        }
    }

    fn prepare(&self, provider: &str) -> Result<OAuthPreparation, String> {
        let mut active_attempt = self
            .active_attempt
            .lock()
            .map_err(|_| "OAuth attempt registry could not be locked.".to_string())?;

        if let Some(attempt) = active_attempt.as_ref() {
            return Err(format!(
                "OAuth login is already active (attempt={}). Finish or cancel it before starting another login.",
                attempt.attempt_id
            ));
        }

        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("OAuth callback listener could not be allocated: {error}"))?;
        let address = listener.local_addr().map_err(|error| {
            format!("OAuth callback listener address could not be read: {error}")
        })?;
        let redirect_uri = native_redirect_uri(address)?;
        let attempt_id = self.next_attempt_id.fetch_add(1, Ordering::Relaxed);

        eprintln!(
            "[mira-client][oauth] attempt={attempt_id} provider={provider} callbackListener=prepared redirectUri={redirect_uri}"
        );
        *active_attempt = Some(NativeOAuthAttempt {
            attempt_id,
            redirect_uri: redirect_uri.clone(),
            listener: Some(listener),
        });

        Ok(OAuthPreparation {
            attempt_id,
            redirect_uri,
        })
    }

    fn take_listener(&self, attempt_id: u64, redirect_uri: &str) -> Result<TcpListener, String> {
        let mut active_attempt = self
            .active_attempt
            .lock()
            .map_err(|_| "OAuth attempt registry could not be locked.".to_string())?;
        let attempt = active_attempt
            .as_mut()
            .ok_or_else(|| "OAuth callback was not prepared for this login attempt.".to_string())?;

        if attempt.attempt_id != attempt_id || attempt.redirect_uri != redirect_uri {
            return Err("OAuth callback does not belong to the active login attempt.".to_string());
        }

        attempt
            .listener
            .take()
            .ok_or_else(|| "OAuth window was already opened for this login attempt.".to_string())
    }

    fn complete(&self, attempt_id: u64) -> bool {
        if let Ok(mut active_attempt) = self.active_attempt.lock()
            && active_attempt
                .as_ref()
                .is_some_and(|attempt| attempt.attempt_id == attempt_id)
        {
            *active_attempt = None;
            return true;
        }

        false
    }

    fn cancel(&self, attempt_id: u64) -> bool {
        self.complete(attempt_id)
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthPreparationRequest {
    provider: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthPreparation {
    attempt_id: u64,
    redirect_uri: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthAttemptIdentity {
    attempt_id: u64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthWindowRequest {
    attempt_id: u64,
    auth_url: String,
    redirect_uri: String,
    #[serde(default)]
    password_reset: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthWindowResponse {
    modal: bool,
    redirect_uri: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SystemBrowserRequest {
    url: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthCallbackPayload {
    url: String,
}

/// Allocates one OS-selected IPv4 loopback port for a native OAuth attempt.
/// Keycloak accepts this as `http://127.0.0.1:<port>` with no trailing slash.
#[tauri::command]
pub(crate) fn prepare_oauth_redirect_uri(
    request: OAuthPreparationRequest,
) -> Result<OAuthPreparation, String> {
    OAUTH_ATTEMPTS.prepare(request.provider.trim())
}

#[tauri::command]
pub(crate) fn cancel_oauth_attempt(request: OAuthAttemptIdentity) {
    OAUTH_ATTEMPTS.cancel(request.attempt_id);
}

/// Opens a prepared native OAuth attempt in an isolated desktop webview.
/// It consumes the listener allocated by `prepare_oauth_redirect_uri`; it never
/// allocates another port or normalizes the redirect URI.
#[tauri::command]
pub(crate) fn start_oauth_window(
    app: tauri::AppHandle,
    request: OAuthWindowRequest,
) -> Result<OAuthWindowResponse, String> {
    let auth_url = request
        .auth_url
        .trim()
        .parse::<tauri::Url>()
        .map_err(|error| format!("OAuth URL is invalid: {error}"))?;
    let redirect_uri = request.redirect_uri.trim();

    if redirect_uri.ends_with('/') {
        return Err("Native OAuth redirect URI must not have a trailing slash.".to_string());
    }
    if authorization_redirect_uri(&auth_url).as_deref() != Some(redirect_uri) {
        return Err(
            "OAuth authorization redirect URI does not match the active attempt.".to_string(),
        );
    }

    // Browser profiles are persistent per provider. This preserves a user's
    // trusted provider session while keeping Google and GitHub Keycloak
    // cookies separate when two desktop clients are open at the same time.
    drop(OAUTH_ATTEMPTS.take_listener(request.attempt_id, redirect_uri)?);
    start_isolated_oauth_window(app, auth_url, request)
}

fn start_isolated_oauth_window(
    app: tauri::AppHandle,
    auth_url: tauri::Url,
    request: OAuthWindowRequest,
) -> Result<OAuthWindowResponse, String> {
    let profile = oauth_profile_name(&auth_url);
    let window_label = format!("mira-oauth-{}", request.attempt_id);
    if let Some(window) = app.get_webview_window(&window_label) {
        let _ = window.close();
    }

    let app_for_navigation = app.clone();
    let redirect_uri_for_navigation = request.redirect_uri.clone();
    let window_label_for_navigation = window_label.clone();
    let attempt_id = request.attempt_id;
    let password_reset = request.password_reset;
    let auth_url_for_loading_screen = auth_url.clone();

    // WebView2 can leave a newly-created window white when its first
    // navigation is an external identity-provider URL. Bootstrap with the
    // bundled page instead; it performs the external navigation after the
    // webview has rendered. This also keeps the OAuth window usable when the
    // provider takes a moment to respond.
    let builder = tauri::WebviewWindowBuilder::new(
        &app,
        window_label,
        tauri::WebviewUrl::App(oauth_loading_screen_path()),
    )
    .title("Mira Login")
    .inner_size(960.0, 680.0)
    .min_inner_size(720.0, 520.0)
    .resizable(true);

    #[cfg(not(target_os = "macos"))]
    let builder = builder.data_directory(oauth_profile_directory(&app, profile)?);

    #[cfg(target_os = "macos")]
    let builder = builder.data_store_identifier(oauth_data_store_identifier(profile));

    let window = builder
        .on_page_load(move |window, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished
                && is_oauth_loading_screen(payload.url())
            {
                eprintln!("[mira-client][oauth] attempt={attempt_id} providerNavigation=start");
                if let Err(error) = window.navigate(auth_url_for_loading_screen.clone()) {
                    eprintln!(
                        "[mira-client][oauth] attempt={attempt_id} providerNavigation=failed error={error}"
                    );
                }
            }
        })
        .on_navigation(move |url| {
            let Some(callback_url) = oauth_callback_url_from_navigation(
                &redirect_uri_for_navigation,
                url.as_str(),
                password_reset,
            ) else {
                return true;
            };

            eprintln!("[mira-client][oauth] attempt={attempt_id} callbackReceived");
            OAUTH_ATTEMPTS.complete(attempt_id);
            let _ = app_for_navigation.emit(
                "mira-oauth-callback",
                OAuthCallbackPayload { url: callback_url },
            );
            if let Some(window) =
                app_for_navigation.get_webview_window(&window_label_for_navigation)
            {
                let _ = window.close();
            }
            focus_main_window(&app_for_navigation);
            false
        })
        .build()
        .map_err(|error| format!("OAuth window could not be created: {error}"))?;

    let app_for_close = app.clone();
    let redirect_uri_for_close = request.redirect_uri.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::CloseRequested { .. })
            && OAUTH_ATTEMPTS.cancel(request.attempt_id)
        {
            let _ = app_for_close.emit(
                "mira-oauth-callback",
                OAuthCallbackPayload {
                    url: oauth_error_callback_url(&redirect_uri_for_close, "oauth_window_closed"),
                },
            );
        }
    });

    Ok(OAuthWindowResponse {
        modal: true,
        redirect_uri: request.redirect_uri,
    })
}

fn oauth_profile_name(auth_url: &tauri::Url) -> &'static str {
    match auth_url
        .query_pairs()
        .find_map(|(key, value)| (key == "kc_idp_hint").then_some(value))
        .as_deref()
    {
        Some("google") => "google",
        Some("github") => "github",
        Some("discord") => "discord",
        _ => "keycloak",
    }
}

/// `oauth-loading.html` is served from Vite's public directory in development
/// and from the bundled frontend in packaged builds. The Tauri host performs
/// the provider navigation after that page has loaded.
fn oauth_loading_screen_path() -> PathBuf {
    PathBuf::from("oauth-loading.html")
}

fn is_oauth_loading_screen(url: &tauri::Url) -> bool {
    url.host_str() == Some("localhost") && url.path() == "/oauth-loading.html"
}

#[cfg(not(target_os = "macos"))]
fn oauth_profile_directory(app: &tauri::AppHandle, profile: &str) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let data_directory = app.path().app_local_data_dir();
    #[cfg(not(target_os = "windows"))]
    let data_directory = app.path().app_data_dir();

    // WebView2 keeps locks, cache, and browser databases in its user-data
    // directory. On Windows this must be local data rather than Roaming
    // AppData, which may be redirected or synchronized by a domain profile.
    let directory = data_directory
        .map_err(|error| format!("OAuth profile directory could not be resolved: {error}"))?
        .join("oauth-profiles")
        .join(profile);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("OAuth profile directory could not be created: {error}"))?;
    Ok(directory)
}

#[cfg(target_os = "macos")]
fn oauth_data_store_identifier(profile: &str) -> [u8; 16] {
    let mut identifier = *b"mira-oauth-00000";
    for (slot, byte) in identifier[11..].iter_mut().zip(profile.bytes()) {
        *slot = byte;
    }
    identifier
}

/// Opens an explicit logout URL without creating or consuming an OAuth login
/// callback listener. Logout is intentionally separate from a new login.
#[tauri::command]
pub(crate) fn open_system_browser(
    app: tauri::AppHandle,
    request: SystemBrowserRequest,
) -> Result<(), String> {
    let url = request
        .url
        .trim()
        .parse::<tauri::Url>()
        .map_err(|error| format!("Browser URL is invalid: {error}"))?;
    open_system_browser_url(&app, url.as_str())
        .map_err(|error| format!("URL could not open in the system browser: {error}"))
}

fn open_system_browser_url(app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        return open_linux_system_browser(url);
    }

    #[cfg(not(target_os = "linux"))]
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn open_linux_system_browser(url: &str) -> Result<(), String> {
    linux_system_browser_command(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not start the Linux system browser: {error}"))
}

#[cfg(target_os = "linux")]
fn linux_system_browser_command(url: &str) -> Command {
    // Tauri's AppImage contains its own xdg-open. The generic opener library
    // finds that one first and reports a successful spawn even if the bundled
    // program cannot resolve the host browser. Use the host opener with an
    // environment that does not point into the mounted AppImage instead.
    let program = if std::path::Path::new("/usr/bin/xdg-open").is_file() {
        "/usr/bin/xdg-open"
    } else {
        "xdg-open"
    };
    let mut command = Command::new(program);
    command.arg(url);
    command.env(
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    );
    for variable in APPIMAGE_BROWSER_ENVIRONMENT {
        command.env_remove(variable);
    }
    command
}

#[cfg(target_os = "linux")]
const APPIMAGE_BROWSER_ENVIRONMENT: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "GDK_PIXBUF_MODULE_FILE",
    "GIO_EXTRA_MODULES",
    "GSETTINGS_SCHEMA_DIR",
    "GST_PLUGIN_PATH",
    "GST_PLUGIN_SCANNER",
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_IM_MODULE_FILE",
    "GTK_PATH",
    "GTK_THEME",
    "LD_LIBRARY_PATH",
    "XDG_DATA_DIRS",
];

fn native_redirect_uri(address: SocketAddr) -> Result<String, String> {
    if !address.ip().is_loopback() || address.ip().to_string() != "127.0.0.1" {
        return Err("OAuth callback listener must use IPv4 loopback only.".to_string());
    }

    // Do not append '/': Keycloak 26.7.0 accepts the authority-only URI and
    // rejects the otherwise identical value with a trailing slash.
    let redirect_uri = format!("http://{address}");
    if !redirect_uri.starts_with(KEYCLOAK_NATIVE_REDIRECT_URI) {
        return Err(
            "OAuth callback redirect URI does not use the registered loopback base.".to_string(),
        );
    }

    Ok(redirect_uri)
}

fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let focus_window = window.clone();
        if let Err(error) = window.run_on_main_thread(move || {
            if let Err(error) = focus_window.show() {
                eprintln!("[mira-client][oauth] couldNotShowMainWindow={error}");
            }
            if let Err(error) = focus_window.unminimize() {
                eprintln!("[mira-client][oauth] couldNotUnminimizeMainWindow={error}");
            }
            if let Err(error) = focus_window.set_focus() {
                eprintln!("[mira-client][oauth] couldNotFocusMainWindow={error}");
            }
        }) {
            eprintln!("[mira-client][oauth] couldNotScheduleMainWindowFocus={error}");
        }
    }
}

fn authorization_redirect_uri(url: &tauri::Url) -> Option<String> {
    url.query_pairs()
        .find_map(|(key, value)| (key == "redirect_uri").then(|| value.into_owned()))
}

/// The OIDC redirect is authority-only, while the HTTP callback naturally has
/// a root request path. This reconstruction does not mutate the redirect URI
/// stored in PKCE state or sent to the token endpoint.
fn callback_url_from_target(redirect_uri: &str, target: &str) -> Option<(String, bool)> {
    let target = if target.starts_with('?') {
        format!("/{target}")
    } else {
        target.to_string()
    };
    let (path, query) = target.split_once('?')?;
    if path != "/" {
        return None;
    }

    let has_code = query
        .split('&')
        .any(|entry| entry.split_once('=').is_some_and(|(key, _)| key == "code"));
    let has_error = query.split('&').any(|entry| {
        matches!(
            entry.split_once('=').map(|(key, _)| key),
            Some("error" | "error_description")
        )
    });
    if !has_code && !has_error {
        return None;
    }

    Some((format!("{redirect_uri}{target}"), has_error))
}

fn oauth_callback_url_from_navigation(
    redirect_uri: &str,
    navigation_url: &str,
    password_reset: bool,
) -> Option<String> {
    let target = navigation_url.strip_prefix(redirect_uri)?;

    if target == "/" && password_reset {
        return Some(password_reset_sent_redirect_uri(redirect_uri));
    }

    callback_url_from_target(redirect_uri, target).map(|(url, _)| url)
}

fn password_reset_sent_redirect_uri(redirect_uri: &str) -> String {
    format!("{redirect_uri}?mira_password_reset=sent")
}

fn oauth_error_callback_url(redirect_uri: &str, error: &str) -> String {
    format!("{redirect_uri}/?error={}", encode_url_component(error))
}

fn encode_url_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    use std::ffi::OsStr;

    #[test]
    fn keycloak_registers_the_authority_only_loopback_uri() {
        assert_eq!(KEYCLOAK_NATIVE_REDIRECT_URI, "http://127.0.0.1");
        assert!(!KEYCLOAK_NATIVE_REDIRECT_URI.ends_with('/'));
    }

    #[test]
    fn native_redirect_uri_uses_an_ephemeral_port_without_a_trailing_slash() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let redirect_uri = native_redirect_uri(address).unwrap();

        assert_eq!(redirect_uri, format!("http://{address}"));
        assert_ne!(address.port(), 0);
        assert!(!redirect_uri.ends_with('/'));
        assert_ne!(redirect_uri, KEYCLOAK_NATIVE_REDIRECT_URI);
    }

    #[test]
    fn registry_allows_only_one_active_native_oauth_attempt() {
        let registry = OAuthAttemptRegistry::new();
        let first = registry.prepare("google").unwrap();
        assert!(registry.prepare("discord").is_err());
        registry.cancel(first.attempt_id);
        assert!(registry.prepare("github").is_ok());
    }

    #[test]
    fn authorization_request_uses_the_exact_ephemeral_redirect_uri() {
        let redirect_uri = "http://127.0.0.1:52743";
        let auth_url = tauri::Url::parse(&format!(
            "https://issuer.example/auth?redirect_uri={}&client_id=mira-bevy",
            encode_url_component(redirect_uri),
        ))
        .unwrap();

        assert_eq!(
            authorization_redirect_uri(&auth_url).as_deref(),
            Some(redirect_uri)
        );
        assert!(
            !authorization_redirect_uri(&auth_url)
                .unwrap()
                .ends_with('/')
        );
    }

    #[test]
    fn callback_get_requests_reconstruct_without_mutating_the_oidc_redirect_uri() {
        let redirect_uri = "http://127.0.0.1:52743";
        assert_eq!(
            callback_url_from_target(
                redirect_uri,
                "/?code=authorization-code&state=request-state"
            ),
            Some((
                "http://127.0.0.1:52743/?code=authorization-code&state=request-state".to_string(),
                false,
            ))
        );
        assert_eq!(
            callback_url_from_target(redirect_uri, "?error=access_denied&state=request-state"),
            Some((
                "http://127.0.0.1:52743/?error=access_denied&state=request-state".to_string(),
                true,
            ))
        );
        assert!(!redirect_uri.ends_with('/'));
    }

    #[test]
    fn isolated_oauth_window_intercepts_only_its_own_loopback_callback() {
        let redirect_uri = "http://127.0.0.1:52743";

        assert_eq!(
            oauth_callback_url_from_navigation(
                redirect_uri,
                "http://127.0.0.1:52743/?code=authorization-code&state=request-state",
                false,
            ),
            Some("http://127.0.0.1:52743/?code=authorization-code&state=request-state".to_string(),),
        );
        assert_eq!(
            oauth_callback_url_from_navigation(
                redirect_uri,
                "http://127.0.0.1:52744/?code=authorization-code&state=request-state",
                false,
            ),
            None,
        );
    }

    #[test]
    fn provider_logins_use_distinct_persistent_profiles() {
        let github_url = tauri::Url::parse(
            "https://issuer.example/auth?kc_idp_hint=github&redirect_uri=http%3A%2F%2F127.0.0.1%3A52743",
        )
        .unwrap();
        let google_url = tauri::Url::parse(
            "https://issuer.example/auth?kc_idp_hint=google&redirect_uri=http%3A%2F%2F127.0.0.1%3A52743",
        )
        .unwrap();

        assert_eq!(oauth_profile_name(&github_url), "github");
        assert_eq!(oauth_profile_name(&google_url), "google");
    }

    #[test]
    fn oauth_window_bootstraps_from_the_bundled_loading_page() {
        let path = oauth_loading_screen_path();
        let local_url = tauri::Url::parse("tauri://localhost/")
            .unwrap()
            .join(path.to_str().unwrap())
            .unwrap();

        assert_eq!(local_url.path(), "/oauth-loading.html");
        assert!(is_oauth_loading_screen(&local_url));
        assert!(local_url.query().is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_browser_command_uses_the_host_environment() {
        let command = linux_system_browser_command("https://dev.tilt-us.com/login");

        assert!(
            command.get_program() == OsStr::new("/usr/bin/xdg-open")
                || command.get_program() == OsStr::new("xdg-open")
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![OsStr::new("https://dev.tilt-us.com/login")]
        );

        let environment = command.get_envs().collect::<Vec<_>>();
        assert!(environment.iter().any(|(key, value)| {
            *key == OsStr::new("PATH")
                && *value
                    == Some(OsStr::new(
                        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    ))
        }));
        for variable in APPIMAGE_BROWSER_ENVIRONMENT {
            assert!(
                environment
                    .iter()
                    .any(|(key, value)| { *key == OsStr::new(variable) && value.is_none() })
            );
        }
    }
}
