use mira_downloads::Environment;

/// Stores the runtime configuration required by the React client.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientConfig {
    environment: Environment,
}

#[derive(serde::Deserialize)]
struct EnvironmentDefinitions {
    dev: WebsiteEnvironmentConfig,
    staging: WebsiteEnvironmentConfig,
    prod: WebsiteEnvironmentConfig,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebsiteEnvironmentConfig {
    website_url: String,
}

const ENVIRONMENT_DEFINITIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../mira-environments.json"
));

/// Returns the environment selected when this desktop client was built.
pub(crate) fn build_environment() -> Result<Environment, String> {
    option_env!("MIRA_ENV")
        .ok_or_else(|| {
            "MIRA_ENV wurde nicht in den Client eingebettet. Build mit MIRA_ENV=dev, MIRA_ENV=staging oder MIRA_ENV=prod erstellen."
                .to_string()
        })?
        .parse()
}

/// Returns the configured public website host for OAuth error callbacks.
pub(crate) fn website_url() -> Result<String, String> {
    let definitions = serde_json::from_str::<EnvironmentDefinitions>(ENVIRONMENT_DEFINITIONS)
        .map_err(|error| format!("Zentrale Environment-Konfiguration ist ungültig: {error}"))?;

    let config = match build_environment()? {
        Environment::Dev => definitions.dev,
        Environment::Staging => definitions.staging,
        Environment::Prod => definitions.prod,
    };

    Ok(config.website_url)
}

/// Returns the centrally selected environment to the React client.
#[tauri::command]
pub(crate) fn client_config() -> Result<ClientConfig, String> {
    Ok(ClientConfig {
        environment: build_environment()?,
    })
}
