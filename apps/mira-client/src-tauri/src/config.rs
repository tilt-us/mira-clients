use std::path::PathBuf;
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
#[cfg(debug_assertions)]
const DEFAULT_CHAMPION_API_BASE_URL: &str = "http://localhost:8084";
#[cfg(not(debug_assertions))]
const DEFAULT_CHAMPION_API_BASE_URL: &str = "https://api.tilt-us.com/game";
#[cfg(debug_assertions)]
const DEFAULT_CHAT_API_BASE_URL: &str = "http://localhost:8085";
#[cfg(not(debug_assertions))]
const DEFAULT_CHAT_API_BASE_URL: &str = "https://api.tilt-us.com/chat";
const DEFAULT_KEYCLOAK_REALM: &str = "mira";
const DEFAULT_KEYCLOAK_CLIENT_ID: &str = "mira-bevy";
const DEFAULT_KEYCLOAK_PASSWORD_CLIENT_ID: &str = "mira-e2e";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientConfig {
    api_base_url: String,
    keycloak_base_url: String,
    keycloak_realm: String,
    keycloak_client_id: String,
    keycloak_password_client_id: String,
    live_api_base_url: String,
    matchmaking_api_base_url: String,
    champion_api_base_url: String,
    chat_api_base_url: String,
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
    champion_api_base_url: Option<String>,
    chat_api_base_url: Option<String>,
}

#[derive(Default, serde::Deserialize)]
struct KeycloakConfigFile {
    base_url: Option<String>,
    realm: Option<String>,
    client_id: Option<String>,
    password_client_id: Option<String>,
}

#[tauri::command]
pub(crate) fn client_config(app: tauri::AppHandle) -> Result<ClientConfig, String> {
    load_client_config(&app)
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
            champion_api_base_url: normalize_base_url(
                services.champion_api_base_url.as_deref(),
                DEFAULT_CHAMPION_API_BASE_URL,
            ),
            chat_api_base_url: normalize_base_url(
                services.chat_api_base_url.as_deref(),
                DEFAULT_CHAT_API_BASE_URL,
            ),
            no_shared_auth: env_flag_enabled("MIRA_CLIENT_NO_SHARED_AUTH"),
        }
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
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
