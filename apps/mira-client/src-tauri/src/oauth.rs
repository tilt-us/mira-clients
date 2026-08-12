use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

const KEYCLOAK_NATIVE_REDIRECT_URI: &str = "http://127.0.0.1";
const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);

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
            .ok_or_else(|| "OAuth browser was already opened for this login attempt.".to_string())
    }

    fn complete(&self, attempt_id: u64) {
        if let Ok(mut active_attempt) = self.active_attempt.lock()
            && active_attempt
                .as_ref()
                .is_some_and(|attempt| attempt.attempt_id == attempt_id)
        {
            *active_attempt = None;
        }
    }

    fn cancel(&self, attempt_id: u64) {
        self.complete(attempt_id);
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

/// Opens a prepared native OAuth attempt in the system browser exactly once.
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

    let listener = OAUTH_ATTEMPTS.take_listener(request.attempt_id, redirect_uri)?;
    eprintln!(
        "[mira-client][oauth] attempt={} openingSystemBrowser",
        request.attempt_id
    );

    // Keep the listener bound before opening the browser. A very fast callback
    // is queued by the operating system until the listener thread begins.
    if let Err(error) = app.opener().open_url(auth_url.as_str(), None::<&str>) {
        OAUTH_ATTEMPTS.complete(request.attempt_id);
        return Err(format!(
            "OAuth login could not open in the system browser: {error}"
        ));
    }

    spawn_oauth_listener(
        app,
        listener,
        request.attempt_id,
        redirect_uri.to_string(),
        request.password_reset,
    );

    Ok(OAuthWindowResponse {
        modal: false,
        redirect_uri: redirect_uri.to_string(),
    })
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
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|error| format!("URL could not open in the system browser: {error}"))
}

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

fn spawn_oauth_listener(
    app: tauri::AppHandle,
    listener: TcpListener,
    attempt_id: u64,
    redirect_uri: String,
    password_reset: bool,
) {
    thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let deadline = Instant::now() + OAUTH_CALLBACK_TIMEOUT;

        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    match read_oauth_callback(&mut stream, &redirect_uri, password_reset) {
                        LoopbackRequest::Callback { url, is_error } => {
                            eprintln!("[mira-client][oauth] attempt={attempt_id} callbackReceived");
                            let _ = write_callback_response(&mut stream, is_error);
                            focus_main_window(&app);
                            OAUTH_ATTEMPTS.complete(attempt_id);
                            let _ = app.emit("mira-oauth-callback", OAuthCallbackPayload { url });
                            return;
                        }
                        LoopbackRequest::Ignore => {
                            let _ = write_ignored_response(&mut stream);
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => {
                    eprintln!(
                        "[mira-client][oauth] attempt={attempt_id} callbackListenerError={error}"
                    );
                    break;
                }
            }
        }

        OAUTH_ATTEMPTS.complete(attempt_id);
        focus_main_window(&app);
        let _ = app.emit(
            "mira-oauth-callback",
            OAuthCallbackPayload {
                url: oauth_error_callback_url(&redirect_uri, "oauth_callback_timeout"),
            },
        );
    });
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

enum LoopbackRequest {
    Callback { url: String, is_error: bool },
    Ignore,
}

fn read_oauth_callback(
    stream: &mut TcpStream,
    redirect_uri: &str,
    allow_empty_root: bool,
) -> LoopbackRequest {
    let Some(target) = read_get_target(stream) else {
        return LoopbackRequest::Ignore;
    };

    if target == "/" && allow_empty_root {
        return LoopbackRequest::Callback {
            url: password_reset_sent_redirect_uri(redirect_uri),
            is_error: false,
        };
    }

    let Some((url, is_error)) = callback_url_from_target(redirect_uri, &target) else {
        return LoopbackRequest::Ignore;
    };

    LoopbackRequest::Callback { url, is_error }
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

fn read_get_target(stream: &mut TcpStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buffer = [0_u8; 4096];
    let bytes_read = stream.read(&mut buffer).ok()?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request.lines().next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;

    (method == "GET").then(|| target.to_string())
}

fn password_reset_sent_redirect_uri(redirect_uri: &str) -> String {
    format!("{redirect_uri}?mira_password_reset=sent")
}

fn oauth_error_callback_url(redirect_uri: &str, error: &str) -> String {
    format!("{redirect_uri}/?error={}", encode_url_component(error))
}

fn write_ignored_response(stream: &mut TcpStream) -> std::io::Result<()> {
    const BODY: &str = "Not an OAuth callback.";
    write_response(stream, "404 Not Found", "text/plain; charset=utf-8", BODY)
}

fn write_callback_response(stream: &mut TcpStream, is_error: bool) -> std::io::Result<()> {
    let body = if is_error {
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Mira Login</title></head><body><h1>Mira</h1><p>Login was not completed. You can return to Mira.</p></body></html>"
    } else {
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Mira Login</title></head><body><h1>Mira</h1><p>Login successful. Returning to Mira...</p><script>window.close();</script></body></html>"
    };
    write_response(stream, "200 OK", "text/html; charset=utf-8", body)
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
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
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn successful_callback_closes_the_browser_tab() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let writer = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            write_callback_response(&mut stream, false).unwrap();
        });

        let mut response = String::new();
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        stream.read_to_string(&mut response).unwrap();
        writer.join().unwrap();

        assert!(response.contains("window.close();"));
        assert!(response.contains("Returning to Mira..."));
    }
}
