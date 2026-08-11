use mira_downloads::Environment;

/// Stores the runtime configuration required by the React client.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientConfig {
    environment: Environment,
}

/// Returns the environment selected when this desktop client was built.
pub(crate) fn build_environment() -> Result<Environment, String> {
    option_env!("MIRA_ENV")
        .ok_or_else(|| {
            "MIRA_ENV wurde nicht in den Client eingebettet. Build mit MIRA_ENV=dev, MIRA_ENV=staging oder MIRA_ENV=prod erstellen."
                .to_string()
        })?
        .parse()
}

/// Returns the centrally selected environment to the React client.
#[tauri::command]
pub(crate) fn client_config() -> Result<ClientConfig, String> {
    Ok(ClientConfig {
        environment: build_environment()?,
    })
}
