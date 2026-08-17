use mira_downloads::Environment;

/// Stores the runtime configuration required by the React client.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientConfig {
    environment: Environment,
    no_shared_auth: bool,
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
        // Multiple launcher processes use the same WebView data directory.
        // Keep credentials scoped to an individual client window instead.
        no_shared_auth: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_runtime_config_disables_shared_auth_storage() {
        let value = serde_json::to_value(ClientConfig {
            environment: Environment::Dev,
            no_shared_auth: true,
        })
        .unwrap();

        assert_eq!(value["noSharedAuth"], true);
    }
}
