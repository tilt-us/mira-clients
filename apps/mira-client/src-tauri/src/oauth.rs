use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    process::Command,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "windows")]
use std::process::Stdio;
use tauri::{Emitter, Manager};
#[cfg(not(target_os = "linux"))]
use tauri_plugin_opener::OpenerExt;
#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM},
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, PostMessageW, WM_CLOSE,
    },
};
#[cfg(target_os = "windows")]
use windows_sys::core::BOOL;

const KEYCLOAK_NATIVE_REDIRECT_URI: &str = "http://127.0.0.1";
const SMART_SCREEN_BROWSER_SECURITY: &str = "smart-screen";
const SYSTEM_BROWSER_SECURITY: &str = "system-browser";

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

    fn is_active(&self, attempt_id: u64) -> bool {
        self.active_attempt
            .lock()
            .ok()
            .and_then(|active_attempt| {
                active_attempt
                    .as_ref()
                    .map(|attempt| attempt.attempt_id == attempt_id)
            })
            .unwrap_or(false)
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
    #[serde(default)]
    browser_security: String,
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

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthCallbackPayload {
    attempt_id: u64,
    url: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthBrowserConfiguration {
    default_browser_security: String,
    installed_browsers: Vec<OAuthInstalledBrowser>,
    smart_screen_available: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthInstalledBrowser {
    id: String,
    name: String,
}

#[derive(Clone)]
struct InstalledOAuthBrowser {
    id: &'static str,
    name: &'static str,
    executable: PathBuf,
}

enum OAuthBrowserSecurity {
    Installed(String),
    #[cfg(not(target_os = "windows"))]
    SmartScreen,
    SystemBrowser,
}

/// Whether the browser launch created an OAuth-only native window that Mira
/// can safely close after the loopback callback. This is deliberately false
/// for generic system-handler launches, where closing a window could affect a
/// browser tab the user was already using.
#[derive(Clone, Copy)]
struct OAuthExternalBrowserLaunch {
    close_callback_window: bool,
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

/// Lists the browser choices available on this computer. Browser IDs are
/// opaque, validated again when an OAuth flow is started, and never contain a
/// filesystem path from the frontend.
#[tauri::command]
pub(crate) fn oauth_browser_options() -> OAuthBrowserConfiguration {
    OAuthBrowserConfiguration {
        default_browser_security: default_oauth_browser_security().to_string(),
        installed_browsers: installed_oauth_browsers()
            .into_iter()
            .map(|browser| OAuthInstalledBrowser {
                id: format!("browser:{}", browser.id),
                name: browser.name.to_string(),
            })
            .collect(),
        smart_screen_available: !cfg!(target_os = "windows"),
    }
}

/// Opens a prepared native OAuth attempt using the configured browser security
/// mode. Smart Screen keeps the flow inside Mira; all external choices use a
/// loopback callback in the selected browser's normal user profile.
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

    let browser_security = parse_oauth_browser_security(&request.browser_security)?;
    #[cfg(not(target_os = "windows"))]
    if matches!(&browser_security, OAuthBrowserSecurity::SmartScreen) {
        drop(OAUTH_ATTEMPTS.take_listener(request.attempt_id, redirect_uri)?);
        return start_smart_screen_oauth_window(app, auth_url, request);
    }

    let callback_listener = OAUTH_ATTEMPTS.take_listener(request.attempt_id, redirect_uri)?;
    #[cfg(target_os = "windows")]
    let callback_page_title = oauth_callback_page_title(request.attempt_id, redirect_uri);
    #[cfg(not(target_os = "windows"))]
    let callback_page_title = "Mira Login".to_string();

    let (browser_label, open_result) = match browser_security {
        OAuthBrowserSecurity::SystemBrowser => (
            SYSTEM_BROWSER_SECURITY.to_string(),
            open_system_browser_url(&app, auth_url.as_str()),
        ),
        OAuthBrowserSecurity::Installed(browser_id) => (
            browser_id.clone(),
            open_installed_oauth_browser_url(&browser_id, auth_url.as_str()),
        ),
        #[cfg(not(target_os = "windows"))]
        OAuthBrowserSecurity::SmartScreen => unreachable!("smart screen returned above"),
    };

    eprintln!(
        "[mira-client][oauth] attempt={} browserSecurity={} open",
        request.attempt_id, browser_label
    );
    let launch = match open_result {
        Ok(launch) => launch,
        Err(error) => {
            OAUTH_ATTEMPTS.cancel(request.attempt_id);
            return Err(format!(
                "OAuth URL could not open in the selected browser: {error}"
            ));
        }
    };

    // The listener is already bound before the browser launches, so a very
    // fast redirect remains queued by the OS until this callback thread
    // starts. Starting it after a successful browser launch avoids a live
    // listener if the selected browser cannot be opened.
    start_oauth_callback_listener(
        app.clone(),
        callback_listener,
        request.attempt_id,
        request.redirect_uri.clone(),
        request.password_reset,
        callback_page_title,
        launch.close_callback_window,
    );

    Ok(OAuthWindowResponse {
        modal: true,
        redirect_uri: request.redirect_uri,
    })
}

#[cfg(not(target_os = "windows"))]
fn start_smart_screen_oauth_window(
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
    #[cfg(not(target_os = "linux"))]
    let auth_url_for_loading_screen = auth_url.clone();

    // Linux WebKitGTK reliably renders the provider URL as the first
    // navigation. The bundled bootstrap page is retained for macOS, where it
    // avoids a white child webview while the native view is constructed.
    #[cfg(target_os = "linux")]
    let initial_url = tauri::WebviewUrl::External(auth_url);
    #[cfg(not(target_os = "linux"))]
    let initial_url = tauri::WebviewUrl::App(oauth_loading_screen_path());

    let builder = tauri::WebviewWindowBuilder::new(&app, window_label, initial_url)
        .title("Mira Login")
        .inner_size(960.0, 680.0)
        .min_inner_size(720.0, 520.0)
        .resizable(true);

    #[cfg(not(target_os = "macos"))]
    let builder = builder.data_directory(oauth_profile_directory(&app, profile)?);

    #[cfg(target_os = "macos")]
    let builder = builder.data_store_identifier(oauth_data_store_identifier(profile));

    #[cfg(not(target_os = "linux"))]
    let builder = builder.on_page_load(move |window, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished
                && is_oauth_loading_screen(payload.url())
            {
                eprintln!("[mira-client][oauth] attempt={attempt_id} smartScreen=navigate");
                if let Err(error) = window.navigate(auth_url_for_loading_screen.clone()) {
                    eprintln!(
                        "[mira-client][oauth] attempt={attempt_id} smartScreen=navigationFailed error={error}"
                    );
                }
            }
        });

    let window = builder
        .on_navigation(move |url| {
            let Some(callback_url) = oauth_callback_url_from_navigation(
                &redirect_uri_for_navigation,
                url.as_str(),
                password_reset,
            ) else {
                return true;
            };

            eprintln!(
                "[mira-client][oauth] attempt={attempt_id} callbackReceived source=smartScreen"
            );
            OAUTH_ATTEMPTS.complete(attempt_id);
            let _ = app_for_navigation.emit(
                "mira-oauth-callback",
                OAuthCallbackPayload {
                    attempt_id,
                    url: callback_url,
                },
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
        .map_err(|error| format!("OAuth Smart Screen could not be created: {error}"))?;

    let app_for_close = app.clone();
    let redirect_uri_for_close = request.redirect_uri.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::CloseRequested { .. })
            && OAUTH_ATTEMPTS.cancel(request.attempt_id)
        {
            let _ = app_for_close.emit(
                "mira-oauth-callback",
                OAuthCallbackPayload {
                    attempt_id: request.attempt_id,
                    url: oauth_error_callback_url(&redirect_uri_for_close, "oauth_window_closed"),
                },
            );
            focus_main_window(&app_for_close);
        }
    });

    Ok(OAuthWindowResponse {
        modal: true,
        redirect_uri: request.redirect_uri,
    })
}

#[cfg(not(target_os = "windows"))]
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
#[cfg(target_os = "macos")]
fn oauth_loading_screen_path() -> PathBuf {
    PathBuf::from("oauth-loading.html")
}

#[cfg(target_os = "macos")]
fn is_oauth_loading_screen(url: &tauri::Url) -> bool {
    url.host_str() == Some("localhost") && url.path() == "/oauth-loading.html"
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn oauth_profile_directory(app: &tauri::AppHandle, profile: &str) -> Result<PathBuf, String> {
    let data_directory = app.path().app_data_dir();

    // Smart Screen is intentionally isolated from the user's normal browser
    // profile. External browser selections never call this helper, so they
    // retain existing accounts, passkeys, and Windows Hello support.
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

fn start_oauth_callback_listener(
    app: tauri::AppHandle,
    listener: TcpListener,
    attempt_id: u64,
    redirect_uri: String,
    password_reset: bool,
    callback_page_title: String,
    close_callback_window: bool,
) {
    thread::spawn(move || {
        #[cfg(not(target_os = "windows"))]
        let _ = close_callback_window;

        if let Err(error) = listener.set_nonblocking(true) {
            eprintln!(
                "[mira-client][oauth] attempt={attempt_id} callbackListener=failed error={error}"
            );
            return;
        }

        while OAUTH_ATTEMPTS.is_active(attempt_id) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0_u8; 8192];
                    let bytes_read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let Some(target) = oauth_http_request_target(&request) else {
                        let _ = write_oauth_http_response(&mut stream, false, &callback_page_title);
                        continue;
                    };
                    let callback_url = if target == "/" && password_reset {
                        Some(password_reset_sent_redirect_uri(&redirect_uri))
                    } else {
                        callback_url_from_target(&redirect_uri, target).map(|(url, _)| url)
                    };

                    let Some(callback_url) = callback_url else {
                        let _ = write_oauth_http_response(&mut stream, false, &callback_page_title);
                        continue;
                    };

                    eprintln!(
                        "[mira-client][oauth] attempt={attempt_id} callbackReceived source=http"
                    );
                    OAUTH_ATTEMPTS.complete(attempt_id);
                    let _ = write_oauth_http_response(&mut stream, true, &callback_page_title);
                    let _ = app.emit(
                        "mira-oauth-callback",
                        OAuthCallbackPayload {
                            attempt_id,
                            url: callback_url,
                        },
                    );
                    #[cfg(target_os = "windows")]
                    if close_callback_window {
                        close_windows_oauth_callback_window(
                            app.clone(),
                            callback_page_title.clone(),
                        );
                    }
                    focus_main_window(&app);
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    eprintln!(
                        "[mira-client][oauth] attempt={attempt_id} callbackListener=failed error={error}"
                    );
                    return;
                }
            }
        }
    });
}

fn oauth_http_request_target(request: &str) -> Option<&str> {
    let request_line = request.lines().next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;

    (method == "GET").then_some(target)
}

fn write_oauth_http_response(
    stream: &mut impl Write,
    success: bool,
    callback_page_title: &str,
) -> std::io::Result<()> {
    let body = if success {
        format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{callback_page_title}</title><script>(function(){{function close(){{try{{window.close()}}catch(_){{}}}}close();addEventListener('load',close,{{once:true}});setTimeout(close,100);setTimeout(close,500)}})()</script></head><body><p>Login complete. This browser window should close automatically.</p><button type=\"button\" onclick=\"window.close()\">Close window</button></body></html>"
        )
    } else {
        "<!doctype html><title>Mira Login</title><body>Ungueltige OAuth-Antwort.</body>".to_string()
    };

    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store, no-cache, max-age=0\r\nPragma: no-cache\r\nReferrer-Policy: no-referrer\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

/// Creates a title that belongs only to one native OAuth attempt. Chromium's
/// app window reflects the callback document's title, so Windows can later
/// close that exact top-level window without enumerating or terminating the
/// user's normal browser processes.
#[cfg(target_os = "windows")]
fn oauth_callback_page_title(attempt_id: u64, redirect_uri: &str) -> String {
    let port = redirect_uri.rsplit(':').next().unwrap_or("callback");
    format!(
        "Mira OAuth Callback {}-{attempt_id}-{port}",
        std::process::id()
    )
}

#[cfg(target_os = "windows")]
fn close_windows_oauth_callback_window(app: tauri::AppHandle, callback_page_title: String) {
    thread::spawn(move || {
        // The HTTP response is already on its way to the browser, but a
        // Chromium app window updates its title asynchronously. Poll briefly
        // for the exact, one-attempt title before asking only that window to
        // close. No browser process or generic browser tab is touched.
        for _ in 0..50 {
            match request_windows_oauth_callback_window_close(&callback_page_title) {
                Ok(true) => {
                    eprintln!("[mira-client][oauth] callbackWindow=closeRequested");
                    // Let the browser process WM_CLOSE before focusing Mira,
                    // otherwise the closing app window can briefly steal the
                    // foreground again.
                    thread::sleep(Duration::from_millis(150));
                    focus_main_window(&app);
                    return;
                }
                Ok(false) => thread::sleep(Duration::from_millis(100)),
                Err(error) => {
                    eprintln!(
                        "[mira-client][oauth] callbackWindow=closeRequestFailed error={error}"
                    );
                    return;
                }
            }
        }
    });
}

#[cfg(target_os = "windows")]
fn request_windows_oauth_callback_window_close(callback_page_title: &str) -> Result<bool, String> {
    let mut search = WindowsOAuthWindowSearch {
        title: callback_page_title.encode_utf16().collect(),
        hwnd: None,
    };

    // EnumWindows invokes its callback synchronously, so the pointer remains
    // valid for the entire enumeration.
    unsafe {
        EnumWindows(
            Some(find_windows_oauth_callback_window),
            (&mut search as *mut WindowsOAuthWindowSearch) as LPARAM,
        );
    }

    let Some(hwnd) = search.hwnd else {
        return Ok(false);
    };

    // WM_CLOSE asks the browser to close this one top-level OAuth window. It
    // does not kill or inspect any browser processes and cannot close another
    // window because the handle was selected by the unique callback title.
    if unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }

    Ok(true)
}

#[cfg(target_os = "windows")]
struct WindowsOAuthWindowSearch {
    title: Vec<u16>,
    hwnd: Option<HWND>,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn find_windows_oauth_callback_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // `lparam` originates from the live `WindowsOAuthWindowSearch` in
    // `request_windows_oauth_callback_window_close` above.
    let search = unsafe { &mut *(lparam as *mut WindowsOAuthWindowSearch) };
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return 1;
    }

    let mut title = vec![0_u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) };
    if copied > 0
        && copied as usize == search.title.len()
        && title[..copied as usize] == *search.title.as_slice()
    {
        search.hwnd = Some(hwnd);
        // Stop at the first exact match. The title includes process ID,
        // attempt ID, and loopback port, so there is only one candidate.
        return 0;
    }

    1
}

fn parse_oauth_browser_security(value: &str) -> Result<OAuthBrowserSecurity, String> {
    let selection = if value.trim().is_empty() {
        default_oauth_browser_security()
    } else {
        value.trim()
    };

    match selection {
        SMART_SCREEN_BROWSER_SECURITY => {
            #[cfg(target_os = "windows")]
            return Err("OAuth Smart Screen is not available on Windows.".to_string());

            #[cfg(not(target_os = "windows"))]
            Ok(OAuthBrowserSecurity::SmartScreen)
        }
        SYSTEM_BROWSER_SECURITY => Ok(OAuthBrowserSecurity::SystemBrowser),
        selection if selection.starts_with("browser:") => {
            let browser_id = selection.trim_start_matches("browser:");
            if browser_id.is_empty()
                || !browser_id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            {
                return Err("Selected OAuth browser is invalid.".to_string());
            }

            Ok(OAuthBrowserSecurity::Installed(browser_id.to_string()))
        }
        _ => Err("Selected OAuth browser is not supported.".to_string()),
    }
}

fn default_oauth_browser_security() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        return SYSTEM_BROWSER_SECURITY;
    }

    #[cfg(not(target_os = "windows"))]
    {
        SMART_SCREEN_BROWSER_SECURITY
    }
}

fn open_system_browser_url(
    app: &tauri::AppHandle,
    url: &str,
) -> Result<OAuthExternalBrowserLaunch, String> {
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        return open_linux_system_browser(url).map(|_| OAuthExternalBrowserLaunch {
            close_callback_window: false,
        });
    }

    #[cfg(target_os = "windows")]
    {
        return open_windows_system_browser_url(app, url);
    }

    #[cfg(target_os = "macos")]
    app.opener()
        .open_url(url, None::<&str>)
        .map(|_| OAuthExternalBrowserLaunch {
            close_callback_window: false,
        })
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn open_windows_system_browser_url(
    app: &tauri::AppHandle,
    url: &str,
) -> Result<OAuthExternalBrowserLaunch, String> {
    if let Some(browser) = windows_default_oauth_browser() {
        eprintln!(
            "[mira-client][oauth] systemBrowser=dedicatedWindow browser={}",
            browser.id
        );
        return windows_oauth_browser_command(&browser, url)
            .spawn()
            .map(|_| OAuthExternalBrowserLaunch {
                close_callback_window: true,
            })
            .map_err(|error| format!("could not start {}: {error}", browser.name));
    }

    // A browser outside Mira's known list can still be the Windows default.
    // Keep the normal system fallback rather than guessing command-line flags
    // for an unknown executable.
    eprintln!("[mira-client][oauth] systemBrowser=defaultHandler");
    app.opener()
        .open_url(url, None::<&str>)
        .map(|_| OAuthExternalBrowserLaunch {
            close_callback_window: false,
        })
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn windows_default_oauth_browser() -> Option<InstalledOAuthBrowser> {
    let browser_id = windows_default_browser_id()?;
    windows_oauth_browsers()
        .into_iter()
        .find(|browser| browser.id == browser_id)
}

#[cfg(target_os = "windows")]
fn windows_default_browser_id() -> Option<&'static str> {
    let output = Command::new("reg.exe")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice",
            "/v",
            "ProgId",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let output = String::from_utf8_lossy(&output.stdout);
    let prog_id = output
        .lines()
        .find_map(|line| line.split_once("REG_SZ").map(|(_, value)| value.trim()))?;
    windows_browser_id_for_progid(prog_id)
}

#[cfg(target_os = "windows")]
fn windows_browser_id_for_progid(prog_id: &str) -> Option<&'static str> {
    let prog_id = prog_id.to_ascii_lowercase();

    if prog_id.contains("edge") {
        Some("edge")
    } else if prog_id.contains("chrome") {
        Some("chrome")
    } else if prog_id.contains("firefox") {
        Some("firefox")
    } else if prog_id.contains("opera") {
        Some("opera")
    } else if prog_id.contains("brave") {
        Some("brave")
    } else if prog_id.contains("vivaldi") {
        Some("vivaldi")
    } else if prog_id.contains("chromium") {
        Some("chromium")
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn windows_oauth_browser_command(browser: &InstalledOAuthBrowser, url: &str) -> Command {
    let mut command = Command::new(&browser.executable);
    // Browser engines can write unrelated Windows-shell diagnostics to the
    // inherited development console (Firefox currently emits
    // `limited_access_features` failures on some installations). OAuth owns
    // neither that feature nor the browser's process, so keep those child
    // diagnostics out of Mira's log while preserving the browser's profile.
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if matches!(
        browser.id,
        "edge" | "chrome" | "opera" | "brave" | "vivaldi" | "chromium"
    ) {
        // Chromium browsers keep the normal user profile in app mode. That
        // gives OAuth access to the user's known accounts and Windows Hello,
        // while creating a dedicated window that the callback can close
        // without touching ordinary browser tabs.
        command.arg(format!("--app={url}"));
    } else {
        // Firefox does not offer Chromium's app-mode flag. A separate window
        // avoids putting the login flow into an unrelated existing tab.
        command.arg("-new-window").arg(url);
    }
    command
}

fn open_installed_oauth_browser_url(
    browser_id: &str,
    url: &str,
) -> Result<OAuthExternalBrowserLaunch, String> {
    let browser = installed_oauth_browsers()
        .into_iter()
        .find(|browser| browser.id == browser_id)
        .ok_or_else(|| format!("Selected OAuth browser '{browser_id}' is no longer installed."))?;

    #[cfg(target_os = "windows")]
    {
        return windows_oauth_browser_command(&browser, url)
            .spawn()
            .map(|_| OAuthExternalBrowserLaunch {
                close_callback_window: true,
            })
            .map_err(|error| format!("could not start {}: {error}", browser.name));
    }

    #[cfg(not(target_os = "windows"))]
    Command::new(&browser.executable)
        .arg(url)
        .spawn()
        .map(|_| OAuthExternalBrowserLaunch {
            close_callback_window: false,
        })
        .map_err(|error| format!("could not start {}: {error}", browser.name))
}

fn installed_oauth_browsers() -> Vec<InstalledOAuthBrowser> {
    #[cfg(target_os = "windows")]
    {
        return windows_oauth_browsers();
    }

    #[cfg(target_os = "macos")]
    {
        return macos_oauth_browsers();
    }

    #[cfg(target_os = "linux")]
    {
        return linux_oauth_browsers();
    }

    #[allow(unreachable_code)]
    Vec::new()
}

fn add_oauth_browser_from_candidates(
    browsers: &mut Vec<InstalledOAuthBrowser>,
    id: &'static str,
    name: &'static str,
    candidates: impl IntoIterator<Item = PathBuf>,
) {
    if browsers.iter().any(|browser| browser.id == id) {
        return;
    }

    if let Some(executable) = candidates.into_iter().find(|candidate| candidate.is_file()) {
        browsers.push(InstalledOAuthBrowser {
            id,
            name,
            executable,
        });
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn paths_under_roots(roots: &[PathBuf], suffixes: &[&str]) -> Vec<PathBuf> {
    roots
        .iter()
        .flat_map(|root| suffixes.iter().map(move |suffix| root.join(suffix)))
        .collect()
}

#[cfg(target_os = "windows")]
fn windows_oauth_browsers() -> Vec<InstalledOAuthBrowser> {
    let mut roots = Vec::new();
    for variable in [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "LOCALAPPDATA",
        "LocalAppData",
    ] {
        if let Some(root) = std::env::var_os(variable).map(PathBuf::from)
            && !roots.contains(&root)
        {
            roots.push(root);
        }
    }

    let mut browsers = Vec::new();
    add_oauth_browser_from_candidates(
        &mut browsers,
        "edge",
        "Microsoft Edge",
        paths_under_roots(&roots, &["Microsoft/Edge/Application/msedge.exe"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "chrome",
        "Google Chrome",
        paths_under_roots(&roots, &["Google/Chrome/Application/chrome.exe"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "firefox",
        "Mozilla Firefox",
        paths_under_roots(&roots, &["Mozilla Firefox/firefox.exe"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "opera",
        "Opera",
        paths_under_roots(
            &roots,
            &[
                "Programs/Opera/launcher.exe",
                "Programs/Opera/Opera.exe",
                "Opera/launcher.exe",
            ],
        ),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "brave",
        "Brave",
        paths_under_roots(
            &roots,
            &["BraveSoftware/Brave-Browser/Application/brave.exe"],
        ),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "vivaldi",
        "Vivaldi",
        paths_under_roots(&roots, &["Vivaldi/Application/vivaldi.exe"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "chromium",
        "Chromium",
        paths_under_roots(&roots, &["Chromium/Application/chrome.exe"]),
    );
    browsers
}

#[cfg(target_os = "macos")]
fn macos_oauth_browsers() -> Vec<InstalledOAuthBrowser> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join("Applications"));
    }

    let mut browsers = Vec::new();
    add_oauth_browser_from_candidates(
        &mut browsers,
        "safari",
        "Safari",
        paths_under_roots(&roots, &["Safari.app/Contents/MacOS/Safari"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "chrome",
        "Google Chrome",
        paths_under_roots(&roots, &["Google Chrome.app/Contents/MacOS/Google Chrome"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "firefox",
        "Mozilla Firefox",
        paths_under_roots(&roots, &["Firefox.app/Contents/MacOS/firefox"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "opera",
        "Opera",
        paths_under_roots(&roots, &["Opera.app/Contents/MacOS/Opera"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "brave",
        "Brave",
        paths_under_roots(&roots, &["Brave Browser.app/Contents/MacOS/Brave Browser"]),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "edge",
        "Microsoft Edge",
        paths_under_roots(
            &roots,
            &["Microsoft Edge.app/Contents/MacOS/Microsoft Edge"],
        ),
    );
    add_oauth_browser_from_candidates(
        &mut browsers,
        "vivaldi",
        "Vivaldi",
        paths_under_roots(&roots, &["Vivaldi.app/Contents/MacOS/Vivaldi"]),
    );
    browsers
}

#[cfg(target_os = "linux")]
fn linux_oauth_browsers() -> Vec<InstalledOAuthBrowser> {
    let mut browsers = Vec::new();
    add_oauth_browser_from_programs(&mut browsers, "firefox", "Mozilla Firefox", &["firefox"]);
    add_oauth_browser_from_programs(
        &mut browsers,
        "chrome",
        "Google Chrome",
        &["google-chrome", "google-chrome-stable"],
    );
    add_oauth_browser_from_programs(
        &mut browsers,
        "chromium",
        "Chromium",
        &["chromium", "chromium-browser"],
    );
    add_oauth_browser_from_programs(&mut browsers, "opera", "Opera", &["opera"]);
    add_oauth_browser_from_programs(&mut browsers, "brave", "Brave", &["brave-browser", "brave"]);
    add_oauth_browser_from_programs(
        &mut browsers,
        "edge",
        "Microsoft Edge",
        &["microsoft-edge", "microsoft-edge-stable"],
    );
    add_oauth_browser_from_programs(&mut browsers, "vivaldi", "Vivaldi", &["vivaldi"]);
    browsers
}

#[cfg(target_os = "linux")]
fn add_oauth_browser_from_programs(
    browsers: &mut Vec<InstalledOAuthBrowser>,
    id: &'static str,
    name: &'static str,
    programs: &[&str],
) {
    let paths = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut roots = paths;
    let snap_root = PathBuf::from("/snap/bin");
    if !roots.contains(&snap_root) {
        roots.push(snap_root);
    }

    add_oauth_browser_from_candidates(
        browsers,
        id,
        name,
        programs
            .iter()
            .flat_map(|program| roots.iter().map(move |root| root.join(program))),
    );
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
    let program = if Path::new("/usr/bin/xdg-open").is_file() {
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

#[cfg(not(target_os = "windows"))]
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

#[cfg(not(target_os = "windows"))]
fn oauth_error_callback_url(redirect_uri: &str, error: &str) -> String {
    format!("{redirect_uri}/?error={}", encode_url_component(error))
}

#[cfg(not(target_os = "windows"))]
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

    #[cfg(any(target_os = "linux", target_os = "windows"))]
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
    fn platform_default_matches_the_native_browser_policy() {
        #[cfg(target_os = "windows")]
        assert_eq!(default_oauth_browser_security(), SYSTEM_BROWSER_SECURITY);

        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            default_oauth_browser_security(),
            SMART_SCREEN_BROWSER_SECURITY
        );
    }

    #[test]
    fn browser_security_accepts_only_known_mode_shapes() {
        #[cfg(target_os = "windows")]
        assert!(parse_oauth_browser_security(SMART_SCREEN_BROWSER_SECURITY).is_err());

        #[cfg(not(target_os = "windows"))]
        assert!(matches!(
            parse_oauth_browser_security(SMART_SCREEN_BROWSER_SECURITY),
            Ok(OAuthBrowserSecurity::SmartScreen)
        ));
        assert!(matches!(
            parse_oauth_browser_security(SYSTEM_BROWSER_SECURITY),
            Ok(OAuthBrowserSecurity::SystemBrowser)
        ));
        assert!(matches!(
            parse_oauth_browser_security("browser:firefox"),
            Ok(OAuthBrowserSecurity::Installed(browser_id)) if browser_id == "firefox"
        ));
        assert!(parse_oauth_browser_security("browser:Firefox").is_err());
        assert!(parse_oauth_browser_security("browser:firefox_profile").is_err());
        assert!(parse_oauth_browser_security("unsupported-browser").is_err());
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
    fn oauth_http_callback_listener_reads_the_request_target() {
        assert_eq!(
            oauth_http_request_target(
                "GET /?code=authorization-code&state=request-state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"
            ),
            Some("/?code=authorization-code&state=request-state"),
        );
        assert_eq!(
            oauth_http_request_target("POST /?code=authorization-code HTTP/1.1\r\n\r\n"),
            None,
        );
    }

    #[test]
    fn successful_external_callback_asks_the_browser_to_close_its_tab() {
        let mut response = Vec::new();
        write_oauth_http_response(&mut response, true, "Mira OAuth Callback test").unwrap();
        let response = String::from_utf8(response).unwrap();

        assert!(response.contains("window.close()"));
        assert!(response.contains("Login complete"));
        assert!(response.contains("<title>Mira OAuth Callback test</title>"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_system_browser_uses_a_dedicated_profile_window_when_supported() {
        assert_eq!(windows_browser_id_for_progid("MSEdgeHTM"), Some("edge"));
        assert_eq!(windows_browser_id_for_progid("BraveHTML"), Some("brave"));
        assert_eq!(windows_browser_id_for_progid("FirefoxURL"), Some("firefox"));

        let edge = InstalledOAuthBrowser {
            id: "edge",
            name: "Microsoft Edge",
            executable: PathBuf::from(
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            ),
        };
        let edge_command = windows_oauth_browser_command(&edge, "https://issuer.example/login");
        assert_eq!(
            edge_command.get_args().collect::<Vec<_>>(),
            vec![OsStr::new("--app=https://issuer.example/login")],
        );

        let firefox = InstalledOAuthBrowser {
            id: "firefox",
            name: "Mozilla Firefox",
            executable: PathBuf::from(r"C:\Program Files\Mozilla Firefox\firefox.exe"),
        };
        let firefox_command =
            windows_oauth_browser_command(&firefox, "https://issuer.example/login");
        assert_eq!(
            firefox_command.get_args().collect::<Vec<_>>(),
            vec![
                OsStr::new("-new-window"),
                OsStr::new("https://issuer.example/login"),
            ],
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn smart_screen_intercepts_only_its_own_loopback_callback() {
        let redirect_uri = "http://127.0.0.1:52743";

        assert_eq!(
            oauth_callback_url_from_navigation(
                redirect_uri,
                "http://127.0.0.1:52743/?code=authorization-code&state=request-state",
                false,
            ),
            Some("http://127.0.0.1:52743/?code=authorization-code&state=request-state".to_string()),
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
